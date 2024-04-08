use std::{
    borrow::Borrow,
    collections::{BTreeSet, HashMap, HashSet},
    error::Error,
};

use evdev::InputEvent;
use tokio::{
    sync::{broadcast, mpsc},
    task::JoinSet,
    time::Duration,
};
use zbus::{fdo, Connection};
use zbus_macros::dbus_interface;

use crate::{
    config::{
        CapabilityMap, CapabilityMapping, CompositeDeviceConfig, DeviceProfile, ProfileMapping,
    },
    input::{
        capability::{Capability, Gamepad, GamepadButton, Mouse},
        event::{
            native::NativeEvent,
            value::{InputValue, TranslationError},
            Event,
        },
        manager::SourceDeviceInfo,
        output_event::UinputOutputEvent,
        source::{self, SourceDevice},
        target::TargetCommand,
    },
    udev::{hide_device, unhide_device},
};

use super::{output_event::OutputEvent, source::SourceCommand};

/// Size of the command channel buffer for processing input events and commands.
const BUFFER_SIZE: usize = 2048;

/// The [InterceptMode] defines whether or not inputs should be routed over
/// DBus instead of to the target devices. This can be used by overlays to
/// intercept input.
#[derive(Debug, Clone)]
pub enum InterceptMode {
    /// Pass all input to the target devices
    None,
    /// Pass all inputs to the target devices except the guide button
    Pass,
    /// Intercept all input and send nothing to the target devices
    Always,
}

/// CompositeDevice commands define all the different ways to interact with [CompositeDevice]
/// over a channel. These commands are processed in an asyncronous thread and
/// dispatched as they come in.
#[derive(Debug, Clone)]
pub enum Command {
    ProcessEvent(String, Event),
    ProcessOutputEvent(OutputEvent),
    GetCapabilities(mpsc::Sender<HashSet<Capability>>),
    SetInterceptMode(InterceptMode),
    GetInterceptMode(mpsc::Sender<InterceptMode>),
    GetSourceDevicePaths(mpsc::Sender<Vec<String>>),
    GetTargetDevicePaths(mpsc::Sender<Vec<String>>),
    GetDBusDevicePaths(mpsc::Sender<Vec<String>>),
    SourceDeviceAdded(SourceDeviceInfo),
    SourceDeviceStopped(String),
    SourceDeviceRemoved(String),
    GetProfileName(mpsc::Sender<String>),
    LoadProfilePath(String, mpsc::Sender<Result<(), String>>),
    WriteEvent(NativeEvent),
    Stop,
}

/// The [DBusInterface] provides a DBus interface that can be exposed for managing
/// a [Manager]. It works by sending command messages to a channel that the
/// [Manager] is listening on.
pub struct DBusInterface {
    tx: broadcast::Sender<Command>,
}

impl DBusInterface {
    fn new(tx: broadcast::Sender<Command>) -> DBusInterface {
        DBusInterface { tx }
    }
}

#[dbus_interface(name = "org.shadowblip.Input.CompositeDevice")]
impl DBusInterface {
    /// Name of the composite device
    #[dbus_interface(property)]
    async fn name(&self) -> fdo::Result<String> {
        Ok("CompositeDevice".into())
    }

    /// Name of the currently loaded profile
    #[dbus_interface(property)]
    async fn profile_name(&self) -> fdo::Result<String> {
        let (sender, mut receiver) = mpsc::channel::<String>(1);
        self.tx
            .send(Command::GetProfileName(sender))
            .map_err(|e| fdo::Error::Failed(e.to_string()))?;
        let Some(profile_name) = receiver.recv().await else {
            return Ok("".to_string());
        };

        Ok(profile_name)
    }

    /// Load the device profile from the given path
    async fn load_profile_path(&self, path: String) -> fdo::Result<()> {
        let (sender, mut receiver) = mpsc::channel::<Result<(), String>>(1);
        self.tx
            .send(Command::LoadProfilePath(path, sender))
            .map_err(|e| fdo::Error::Failed(e.to_string()))?;

        let Some(result) = receiver.recv().await else {
            return Err(fdo::Error::Failed(
                "No response from CompositeDevice".to_string(),
            ));
        };

        if let Err(e) = result {
            return Err(fdo::Error::Failed(format!(
                "Failed to load profile: {:?}",
                e
            )));
        }

        Ok(())
    }

    /// List of capabilities that all source devices implement
    #[dbus_interface(property)]
    async fn capabilities(&self) -> fdo::Result<Vec<String>> {
        let (sender, mut receiver) = mpsc::channel::<HashSet<Capability>>(1);
        self.tx
            .send(Command::GetCapabilities(sender))
            .map_err(|e| fdo::Error::Failed(e.to_string()))?;
        let Some(capabilities) = receiver.recv().await else {
            return Ok(Vec::new());
        };

        let mut capability_strings = Vec::new();
        for cap in capabilities {
            let str = match cap {
                Capability::Gamepad(gamepad) => match gamepad {
                    Gamepad::Button(button) => format!("Gamepad:Button:{}", button),
                    Gamepad::Axis(axis) => format!("Gamepad:Axis:{}", axis),
                    Gamepad::Trigger(trigger) => format!("Gamepad:Trigger:{}", trigger),
                    Gamepad::Accelerometer => "Gamepad:Accelerometer".to_string(),
                    Gamepad::Gyro => "Gamepad:Gyro".to_string(),
                },
                Capability::Mouse(mouse) => match mouse {
                    Mouse::Motion => "Mouse:Motion".to_string(),
                    Mouse::Button(button) => format!("Mouse:Button:{}", button),
                },
                Capability::Keyboard(key) => format!("Keyboard:{}", key),
                _ => cap.to_string(),
            };
            capability_strings.push(str);
        }

        Ok(capability_strings)
    }

    /// List of source devices that this composite device is processing inputs for
    #[dbus_interface(property)]
    async fn source_device_paths(&self) -> fdo::Result<Vec<String>> {
        let (sender, mut receiver) = mpsc::channel::<Vec<String>>(1);
        self.tx
            .send(Command::GetSourceDevicePaths(sender))
            .map_err(|e| fdo::Error::Failed(e.to_string()))?;
        let Some(paths) = receiver.recv().await else {
            return Ok(Vec::new());
        };

        Ok(paths)
    }

    /// The intercept mode of the composite device.
    #[dbus_interface(property)]
    async fn intercept_mode(&self) -> fdo::Result<u32> {
        let (sender, mut receiver) = mpsc::channel::<InterceptMode>(1);
        self.tx
            .send(Command::GetInterceptMode(sender))
            .map_err(|e| fdo::Error::Failed(e.to_string()))?;
        let Some(mode) = receiver.recv().await else {
            return Ok(0);
        };

        match mode {
            InterceptMode::None => Ok(0),
            InterceptMode::Pass => Ok(1),
            InterceptMode::Always => Ok(2),
        }
    }

    #[dbus_interface(property)]
    async fn set_intercept_mode(&self, mode: u32) -> zbus::Result<()> {
        let mode = match mode {
            0 => InterceptMode::None,
            1 => InterceptMode::Pass,
            2 => InterceptMode::Always,
            _ => InterceptMode::None,
        };
        self.tx
            .send(Command::SetInterceptMode(mode))
            .map_err(|err| zbus::Error::Failure(err.to_string()))?;
        Ok(())
    }

    /// Target devices that this [CompositeDevice] is managing
    #[dbus_interface(property)]
    async fn target_devices(&self) -> fdo::Result<Vec<String>> {
        let (sender, mut receiver) = mpsc::channel::<Vec<String>>(1);
        self.tx
            .send(Command::GetTargetDevicePaths(sender))
            .map_err(|e| fdo::Error::Failed(e.to_string()))?;
        let Some(paths) = receiver.recv().await else {
            return Ok(Vec::new());
        };

        Ok(paths)
    }

    /// Target dbus devices that this [CompositeDevice] is managing
    #[dbus_interface(property)]
    async fn dbus_devices(&self) -> fdo::Result<Vec<String>> {
        let (sender, mut receiver) = mpsc::channel::<Vec<String>>(1);
        self.tx
            .send(Command::GetDBusDevicePaths(sender))
            .map_err(|e| fdo::Error::Failed(e.to_string()))?;
        let Some(paths) = receiver.recv().await else {
            return Ok(Vec::new());
        };

        Ok(paths)
    }
}

/// Defines a handle to a [CompositeDevice] for communication
#[derive(Debug)]
pub struct Handle {
    pub tx: broadcast::Sender<Command>,
    pub rx: broadcast::Receiver<Command>,
}

impl Handle {
    pub fn new(tx: broadcast::Sender<Command>, rx: broadcast::Receiver<Command>) -> Self {
        Self { tx, rx }
    }
}

/// A [CompositeDevice] represents any number source input devices that
/// can translate input to any target devices
#[derive(Debug)]
pub struct CompositeDevice {
    /// Connection to DBus
    conn: Connection,
    /// Configuration for the CompositeDevice
    config: CompositeDeviceConfig,
    /// Capabilities describe all input capabilities from all source devices
    capabilities: HashSet<Capability>,
    /// Capability mapping for the CompositeDevice
    capability_map: Option<CapabilityMap>,
    /// Name of the currently loaded [DeviceProfile] for the CompositeDevice.
    /// The [DeviceProfile] is used to translate input events.
    device_profile: Option<String>,
    /// Map of profile source events to translate to one or more profile mapping
    /// configs that define how the source event should be translated.
    device_profile_config_map: HashMap<Capability, Vec<ProfileMapping>>,
    /// List of input capabilities that can be translated by the capability map
    translatable_capabilities: Vec<Capability>,
    /// List of currently "pressed" actions used to translate multiple input
    /// sequences into a single input event.
    translatable_active_inputs: Vec<Capability>,
    /// Keep track of translated events we've emitted so we can send
    /// release events
    emitted_mappings: HashMap<String, CapabilityMapping>,
    /// The DBus path this [CompositeDevice] is listening on
    dbus_path: Option<String>,
    /// Mode defining how inputs should be routed
    intercept_mode: InterceptMode,
    /// Transmit channel for sending commands to this composite device
    tx: broadcast::Sender<Command>,
    /// Receiver channel for listening for commands
    rx: broadcast::Receiver<Command>,
    /// Map of source device id to their respective transmitter channel.
    /// E.g. {"evdev://event0": <Sender>}
    source_devices: HashMap<String, mpsc::Sender<SourceCommand>>,
    /// Source devices that this composite device will consume.
    source_devices_discovered: Vec<SourceDevice>,
    /// HashSet of source devices that are blocked from passing their input events to target
    /// events.
    source_devices_blocked: HashSet<String>,
    /// Physical device path for source devices. E.g. ["/dev/input/event0"]
    source_device_paths: Vec<String>,
    /// All currently running source device threads
    source_device_tasks: JoinSet<()>,
    /// Unique identifiers for running source devices. E.g. ["evdev://event0"]
    source_devices_used: Vec<String>,
    /// Map of DBus paths to their respective transmitter channel.
    /// E.g. {"/org/shadowblip/InputPlumber/devices/target/gamepad0": <Sender>}
    target_devices: HashMap<String, mpsc::Sender<TargetCommand>>,
    /// Map of DBusDevice DBus paths to their respective transmitter channel.
    /// E.g. {"/org/shadowblip/InputPlumber/devices/target/dbus0": <Sender>}
    target_dbus_devices: HashMap<String, mpsc::Sender<TargetCommand>>,
    /// Set of available Force Feedback effect IDs that are not in use
    ff_effect_ids: BTreeSet<i16>,
    /// Source devices use their own IDs for uploaded force feedback effects.
    /// This mapping maps the composite device effect ids to source device effect ids.
    /// E.g. {3: {"evdev://event0": 6}}
    ff_effect_id_source_map: HashMap<i16, HashMap<String, i16>>,
}

impl CompositeDevice {
    pub fn new(
        conn: Connection,
        config: CompositeDeviceConfig,
        device_info: SourceDeviceInfo,
        capability_map: Option<CapabilityMap>,
    ) -> Result<Self, Box<dyn Error>> {
        log::info!("Creating CompositeDevice with config: {}", config.name);
        let (tx, rx) = broadcast::channel(BUFFER_SIZE);
        let mut device = Self {
            conn,
            config,
            capabilities: HashSet::new(),
            capability_map,
            device_profile: None,
            device_profile_config_map: HashMap::new(),
            translatable_capabilities: Vec::new(),
            translatable_active_inputs: Vec::new(),
            emitted_mappings: HashMap::new(),
            dbus_path: None,
            intercept_mode: InterceptMode::None,
            tx,
            rx,
            source_devices: HashMap::new(),
            source_devices_discovered: Vec::new(),
            source_devices_blocked: HashSet::new(),
            source_device_paths: Vec::new(),
            source_device_tasks: JoinSet::new(),
            source_devices_used: Vec::new(),
            target_devices: HashMap::new(),
            target_dbus_devices: HashMap::new(),
            ff_effect_ids: (0..64).collect(),
            ff_effect_id_source_map: HashMap::new(),
        };

        // Load the capability map if one was defined
        if device.capability_map.is_some() {
            device.load_capability_map()?;
        }

        // Load the default profile
        let profile_path = "/usr/share/inputplumber/profiles/default.yaml";
        if let Err(error) = device.load_device_profile_from_path(profile_path.to_string()) {
            log::warn!(
                "Unable to load default profile at {}. {}",
                profile_path,
                error
            );
        };

        // If a capability map is defined, add those target capabilities to
        // the hashset of implemented capabilities.
        if let Some(map) = device.capability_map.as_ref() {
            for mapping in map.mapping.clone() {
                let cap = mapping.target_event.clone().into();
                if cap == Capability::NotImplemented {
                    continue;
                }
                device.capabilities.insert(cap);
            }
        }

        device.add_source_device(device_info)?;

        Ok(device)
    }

    /// Creates a new instance of the composite device interface on DBus.
    pub async fn listen_on_dbus(&mut self, path: String) -> Result<(), Box<dyn Error>> {
        let conn = self.conn.clone();
        let tx = self.tx.clone();
        self.dbus_path = Some(path.clone());
        tokio::spawn(async move {
            let iface = DBusInterface::new(tx);
            if let Err(e) = conn.object_server().at(path, iface).await {
                log::error!("Failed to setup DBus interface for device: {:?}", e);
            }
        });
        log::info!("Started listening on {}", self.dbus_path.as_ref().unwrap());
        Ok(())
    }

    /// Starts the [CompositeDevice] and listens for events from all source
    /// devices to translate the events and send them to the appropriate target.
    pub async fn run(
        &mut self,
        targets: HashMap<String, mpsc::Sender<TargetCommand>>,
    ) -> Result<(), Box<dyn Error>> {
        log::debug!("Starting composite device");

        // Start all source devices
        self.run_source_devices().await?;

        // Keep track of all target devices
        for target in targets.values() {
            if let Err(e) = target
                .send(TargetCommand::SetCompositeDevice(self.tx.clone()))
                .await
            {
                return Err(
                    format!("Failed to set composite device for target device: {:?}", e).into(),
                );
            }
        }
        self.target_devices = targets;

        // Loop and listen for command events
        log::debug!("CompositeDevice started");
        while let Ok(cmd) = self.rx.recv().await {
            //log::debug!("Received command: {:?}", cmd);
            match cmd {
                Command::ProcessEvent(device_id, event) => {
                    if let Err(e) = self.process_event(device_id, event).await {
                        log::error!("Failed to process event: {:?}", e);
                        // TODO: Use proper errors to check for 'SendError' and
                        // stop the composite device
                        break;
                    }
                }
                Command::ProcessOutputEvent(event) => {
                    if let Err(e) = self.process_output_event(event).await {
                        log::error!("Failed to process output event: {:?}", e);
                    }
                }
                Command::GetCapabilities(sender) => {
                    if let Err(e) = sender.send(self.capabilities.clone()).await {
                        log::error!("Failed to send capabilities: {:?}", e);
                    }
                }
                Command::SetInterceptMode(mode) => self.set_intercept_mode(mode),
                Command::GetInterceptMode(sender) => {
                    if let Err(e) = sender.send(self.intercept_mode.clone()).await {
                        log::error!("Failed to send intercept mode: {:?}", e);
                    }
                }
                Command::GetSourceDevicePaths(sender) => {
                    if let Err(e) = sender.send(self.get_source_device_paths()).await {
                        log::error!("Failed to send source device paths: {:?}", e);
                    }
                }
                Command::GetTargetDevicePaths(sender) => {
                    let paths = self.target_devices.keys().cloned().collect();
                    if let Err(e) = sender.send(paths).await {
                        log::error!("Failed to send target device paths: {:?}", e);
                    }
                }
                Command::GetDBusDevicePaths(sender) => {
                    let paths = self.target_dbus_devices.keys().cloned().collect();
                    if let Err(e) = sender.send(paths).await {
                        log::error!("Failed to send dbus device paths: {:?}", e);
                    }
                }
                Command::SourceDeviceAdded(device_info) => {
                    if let Err(e) = self.on_source_device_added(device_info).await {
                        log::error!("Failed to add source device: {:?}", e);
                    }
                }
                Command::SourceDeviceStopped(device_id) => {
                    log::debug!("Detected source device stopped: {}", device_id);
                    if let Err(e) = self.on_source_device_removed(device_id).await {
                        log::error!("Failed to remove source device: {:?}", e);
                    }
                    if self.source_devices_used.is_empty() {
                        break;
                    }
                }
                Command::SourceDeviceRemoved(device_id) => {
                    log::debug!("Detected source device removed: {}", device_id);
                    if let Err(e) = self.on_source_device_removed(device_id).await {
                        log::error!("Failed to remove source device: {:?}", e);
                    }
                    if self.source_devices_used.is_empty() {
                        break;
                    }
                }
                Command::GetProfileName(sender) => {
                    let profile_name = self.device_profile.clone().unwrap_or_default();
                    if let Err(e) = sender.send(profile_name).await {
                        log::error!("Failed to send profile name: {:?}", e);
                    }
                }
                Command::LoadProfilePath(path, sender) => {
                    log::info!("Loading profile from path: {path}");
                    let result = match self.load_device_profile_from_path(path.clone()) {
                        Ok(_) => Ok(()),
                        Err(e) => Err(e.to_string()),
                    };
                    if let Err(e) = sender.send(result).await {
                        log::error!("Failed to send load profile result: {:?}", e);
                    }
                }
                Command::WriteEvent(event) => {
                    if let Err(e) = self.write_event(event).await {
                        log::error!("Failed to write event: {:?}", e);
                    }
                }
                Command::Stop => {
                    log::debug!("Stopping CompositeDevice");
                    break;
                }
            }
        }
        log::info!(
            "CompositeDevice stopping: {}",
            self.dbus_path.as_ref().unwrap()
        );

        // Stop all target devices
        log::debug!("Stopping target devices");
        #[allow(clippy::for_kv_map)]
        for (_, target) in &self.target_devices {
            target.send(TargetCommand::Stop).await?;
        }
        #[allow(clippy::for_kv_map)]
        for (_, target) in &self.target_dbus_devices {
            target.send(TargetCommand::Stop).await?;
        }

        // Unhide all source devices
        for source_path in self.source_device_paths.clone() {
            if source_path.starts_with("/sys/bus/iio/devices") {
                log::debug!("Skipping unhiding IIO device: {source_path}");
                continue;
            }
            log::debug!("Un-hiding device: {}", source_path);
            if let Err(e) = unhide_device(source_path.clone()).await {
                log::debug!("Unable to unhide device {source_path}: {:?}", e);
            }
        }

        // Wait on all tasks
        log::debug!("Waiting for source device tasks to finish");
        while let Some(res) = self.source_device_tasks.join_next().await {
            res?;
        }

        log::info!(
            "CompositeDevice stopped: {}",
            self.dbus_path.as_ref().unwrap()
        );

        Ok(())
    }

    /// Return a [Handle] to the [CompositeDevice] to communicate with
    pub fn handle(&self) -> Handle {
        let rx = self.subscribe();
        let tx = self.transmitter();

        Handle::new(tx, rx)
    }

    /// Return a [Command] transmitter to communitcate with the device while it
    /// is running
    pub fn transmitter(&self) -> broadcast::Sender<Command> {
        self.tx.clone()
    }

    /// Return a [Command] receiver to listen for signals while the device
    /// is running
    pub fn subscribe(&self) -> broadcast::Receiver<Command> {
        self.tx.subscribe()
    }

    /// Returns an array of all source devices ids being used by this device.
    pub fn get_source_devices_used(&self) -> Vec<String> {
        self.source_devices_used.clone()
    }

    /// Sets the DBus target devices on the [CompositeDevice].
    pub fn set_dbus_devices(&mut self, devices: HashMap<String, mpsc::Sender<TargetCommand>>) {
        self.target_dbus_devices = devices;
    }

    /// Return a list of source device paths (e.g. /dev/hidraw0, /dev/input/event0)
    /// that this composite device is managing
    fn get_source_device_paths(&self) -> Vec<String> {
        self.source_device_paths.clone()
    }

    /// Start and run the source devices that this composite device will
    /// consume.
    async fn run_source_devices(&mut self) -> Result<(), Box<dyn Error>> {
        // Keep a list of all the tasks

        // Hide all source devices
        // TODO: Make this configurable
        for source_path in self.source_device_paths.clone() {
            // Skip hiding IIO devices
            if source_path.starts_with("/sys/bus/iio/devices") {
                log::debug!("Skipping hiding IIO device: {source_path}");
                continue;
            }
            log::debug!("Hiding device: {}", source_path);
            hide_device(source_path).await?;
        }

        log::debug!("Starting new source devices");
        // Start listening for events from all source devices
        let sources = self.source_devices_discovered.drain(..);
        for source in sources {
            match source {
                // If the source device is an event device (i.e. from /dev/input/eventXX),
                // then start listening for inputs from that device.
                SourceDevice::EventDevice(mut device) => {
                    let device_id = device.get_id();
                    let source_tx = device.transmitter();
                    self.source_devices.insert(device_id.clone(), source_tx);
                    let tx = self.tx.clone();
                    self.source_device_tasks.spawn(async move {
                        if let Err(e) = device.run().await {
                            log::error!("Failed running event device: {:?}", e);
                        }
                        log::debug!("Event device closed");
                        if let Err(e) = tx.send(Command::SourceDeviceStopped(device_id)) {
                            log::error!("Failed to send device stop command: {:?}", e);
                        }
                    });
                }

                // If the source device is a hidraw device (i.e. /dev/hidraw0),
                // then start listening for inputs from that device.
                SourceDevice::HIDRawDevice(device) => {
                    let device_id = device.get_id();
                    let source_tx = device.transmitter();
                    self.source_devices.insert(device_id.clone(), source_tx);
                    let tx = self.tx.clone();
                    self.source_device_tasks.spawn(async move {
                        if let Err(e) = device.run().await {
                            log::error!("Failed running hidraw device: {:?}", e);
                        }
                        log::debug!("HIDRaw device closed");
                        if let Err(e) = tx.send(Command::SourceDeviceStopped(device_id)) {
                            log::error!("Failed to send device stop command: {:?}", e);
                        }
                    });
                }

                // If the source device is an iio device (i.e. /sys/bus/iio/devices/iio:device0),
                // then start listening for inputs from that device.
                SourceDevice::IIODevice(device) => {
                    let device_id = device.get_id();
                    let source_tx = device.transmitter();
                    self.source_devices.insert(device_id.clone(), source_tx);
                    let tx = self.tx.clone();
                    self.source_device_tasks.spawn(async move {
                        if let Err(e) = device.run().await {
                            log::error!("Failed running iio device: {:?}", e);
                        }
                        log::debug!("IIO device closed");
                        if let Err(e) = tx.send(Command::SourceDeviceStopped(device_id)) {
                            log::error!("Failed to send device stop command: {:?}", e);
                        }
                    });
                }
            }
        }
        log::debug!("All source device tasks started");
        Ok(())
    }

    /// Process a single event from a source device. Events are piped through
    /// a translation layer, then dispatched to the appropriate target device(s)
    async fn process_event(
        &mut self,
        device_id: String,
        raw_event: Event,
    ) -> Result<(), Box<dyn Error>> {
        if self.source_devices_blocked.contains(&device_id) {
            log::trace!("Blocking event! {:?}", raw_event);
            return Ok(());
        }
        log::trace!("Received event: {:?} from {device_id}", raw_event);

        // Convert the event into a NativeEvent
        let event: NativeEvent = match raw_event {
            Event::Evdev(event) => event.into(),
            Event::HIDRaw => todo!(),
            Event::Native(event) => event,
            Event::DBus(_) => todo!(),
        };
        let cap = event.as_capability();
        log::trace!("Event capability: {:?}", cap);

        // Only send valid events to the target device(s)
        if cap == Capability::NotImplemented {
            log::trace!("Refusing to send 'NotImplemented' event to target devices");
            return Ok(());
        }

        // Check if the event needs to be translated based on the
        // capability map. Translated events will be re-enqueued, so this will
        // return early.
        log::trace!(
            "Translatable capabilities: {:?}",
            self.translatable_capabilities
        );
        if self.capability_map.is_some() && self.translatable_capabilities.contains(&cap) {
            log::trace!("Capability mapping found for event");
            self.translate_capability(&event).await?;
            return Ok(());
        }
        self.handle_event(event).await?;

        Ok(())
    }

    /// Process a single output event from a target device.
    async fn process_output_event(&mut self, event: OutputEvent) -> Result<(), Box<dyn Error>> {
        log::trace!("Received output event: {:?}", event);

        // Handle any output events that need to upload FF effect data
        if let OutputEvent::Uinput(uinput) = event.borrow() {
            match uinput {
                UinputOutputEvent::FFUpload(data, target_dev) => {
                    // Upload the effect data to the source devices
                    let mut source_effect_ids = HashMap::new();
                    for (source_id, source) in self.source_devices.iter() {
                        log::debug!("Uploading effect to {source_id}");
                        let (tx, rx) = std::sync::mpsc::channel();
                        source.send(SourceCommand::UploadEffect(*data, tx)).await?;

                        // Wait for the result of the upload
                        match rx.recv_timeout(Duration::from_secs(1)) {
                            Ok(upload_result) => {
                                if let Err(e) = upload_result {
                                    log::debug!(
                                        "Failed to upload FF effect to {source_id}: {:?}",
                                        e
                                    );
                                    continue;
                                }
                                let source_effect_id = upload_result.unwrap();
                                log::debug!("Successfully uploaded effect with source effect id {source_effect_id}");
                                source_effect_ids.insert(source_id.clone(), source_effect_id);
                            }
                            Err(err) => {
                                log::error!(
                                    "Failed to receive response from source device {source_id} to upload effect: {:?}",
                                    err
                                );
                            }
                        }
                    }

                    // If no source devices uploaded the effect, don't bother
                    // allocating an effect id.
                    if source_effect_ids.is_empty() {
                        log::debug!("No source device available to handle FF effect");
                        target_dev.send(None)?;
                    }

                    // If upload was successful, return an effect ID
                    let id = self.ff_effect_ids.iter().next().copied();
                    if let Some(id) = id {
                        log::debug!("Uploaded effect with effect id {id}");
                        self.ff_effect_ids.remove(&id);
                        self.ff_effect_id_source_map.insert(id, source_effect_ids);
                        target_dev.send(Some(id))?;
                    } else {
                        target_dev.send(None)?;
                    }
                }
                UinputOutputEvent::FFErase(effect_id) => {
                    let effect_id = *effect_id as i16;
                    // Erase the effect from source devices
                    if let Some(source_effect_ids) = self.ff_effect_id_source_map.get(&effect_id) {
                        for (source_id, source_effect_id) in source_effect_ids.iter() {
                            let Some(source) = self.source_devices.get(source_id) else {
                                continue;
                            };
                            log::debug!("Erasing effect from {source_id}");
                            let (tx, rx) = std::sync::mpsc::channel();
                            source
                                .send(SourceCommand::EraseEffect(*source_effect_id, tx))
                                .await?;

                            // Wait for the result of the erase
                            match rx.recv_timeout(Duration::from_secs(1)) {
                                Ok(erase_result) => {
                                    if let Err(e) = erase_result {
                                        log::debug!(
                                            "Failed to erase FF effect from {source_id}: {:?}",
                                            e
                                        );
                                        continue;
                                    }
                                }
                                Err(err) => {
                                    log::error!("Failed to receive response from source device {source_id} to erase effect: {:?}", err);
                                }
                            }
                        }
                    }

                    // Add the effect ID to list of available effect ids
                    log::debug!("Erased effect with effect id {effect_id}");
                    self.ff_effect_ids.insert(effect_id);
                    self.ff_effect_id_source_map.remove(&effect_id);
                }
            }

            log::trace!("Available effect IDs: {:?}", self.ff_effect_ids);
            log::debug!("Used effect IDs: {:?}", self.ff_effect_id_source_map);

            return Ok(());
        }

        // TODO: Only write the event to devices that are capabile of handling it
        for (source_id, source) in self.source_devices.iter() {
            // If this is a force feedback event, translate the effect id into
            // the source device's effect id.
            if let OutputEvent::Evdev(input_event) = event {
                if input_event.event_type().0 == evdev::EventType::FORCEFEEDBACK.0 {
                    // Lookup the source effect ids for the effect
                    let effect_id = input_event.code() as i16;
                    let value = input_event.value();
                    let Some(source_effect_ids) = self.ff_effect_id_source_map.get(&effect_id)
                    else {
                        log::warn!("Received FF event with unknown id: {effect_id}");
                        continue;
                    };

                    // Lookup the source effect id for this source device
                    let Some(source_effect_id) = source_effect_ids.get(source_id) else {
                        log::warn!("Unable to find source effect id for effect {effect_id} from {source_id}");
                        continue;
                    };

                    // Create a new FF event with the source device effect id.
                    let new_event = InputEvent::new_now(
                        evdev::EventType::FORCEFEEDBACK.0,
                        *source_effect_id as u16,
                        value,
                    );
                    let output_event = OutputEvent::Evdev(new_event);

                    // Write the FF event to the source device
                    let event = SourceCommand::WriteEvent(output_event);
                    source.send(event).await?;
                    continue;
                }
            }

            let event = SourceCommand::WriteEvent(event.clone());
            source.send(event.clone()).await?;
        }

        Ok(())
    }

    /// Translate and write the given event to the appropriate target devices
    async fn handle_event(&mut self, event: NativeEvent) -> Result<(), Box<dyn Error>> {
        // Check if we need to reverse the event list.
        let is_pressed = event.pressed();
        // Check if this is is a single event or multiple events.
        let mut is_chord = false;
        // Track the delay for chord events.
        let mut sleep_time = 0;

        // Translate the event using the device profile.
        let mut events = if self.device_profile.is_some() {
            self.translate_event(&event).await?
        } else {
            vec![event]
        };

        // Check if we need to reverse the event list.
        if events.len() > 1 {
            is_chord = true;
            if !is_pressed {
                events = events.into_iter().rev().collect();
                // To support on_release events, we need to sleep past the time it takes to emit
                // the down events.
                sleep_time = 80 * events.len() as u64;
            }
        }

        for event in events {
            // Process the event depending on the intercept mode
            let mode = self.intercept_mode.clone();
            let cap = event.as_capability();
            if matches!(mode, InterceptMode::Pass)
                && cap == Capability::Gamepad(Gamepad::Button(GamepadButton::Guide))
            {
                // Set the intercept mode while the button is pressed
                if event.pressed() {
                    log::debug!("Intercepted guide button press");
                    self.set_intercept_mode(InterceptMode::Always);
                }
            }

            // Add a keypress delay for event chords. This is required to
            // support steam chords as it will pass through ro miss events if they aren't properly
            // timed.
            if is_chord {
                let tx = self.tx.clone();
                tokio::spawn(async move {
                    tokio::time::sleep(Duration::from_millis(sleep_time)).await;
                    if let Err(e) = tx.send(Command::WriteEvent(event)) {
                        log::error!("Failed to send chord event command: {:?}", e);
                    }
                });
                // Increment the sleep time.
                sleep_time += 80;
            } else {
                // for single events we can emit immediatly without tokio overhead.
                self.write_event(event).await?;
            }
        }
        Ok(())
    }

    /// Writes the given event to the appropriate target device.
    async fn write_event(&self, event: NativeEvent) -> Result<(), Box<dyn Error>> {
        let cap = event.as_capability();

        // If this event implements the DBus capability, send the event to DBus devices
        if matches!(cap, Capability::DBus(_)) {
            let event = TargetCommand::WriteEvent(event);
            #[allow(clippy::for_kv_map)]
            for (_, target) in &self.target_dbus_devices {
                target.send(event.clone()).await?;
            }
            return Ok(());
        }

        // If the device is in intercept mode, only send events to DBus
        // target devices.
        if matches!(self.intercept_mode, InterceptMode::Always) {
            let event = TargetCommand::WriteEvent(event);
            #[allow(clippy::for_kv_map)]
            for (_, target) in &self.target_dbus_devices {
                target.send(event.clone()).await?;
            }
            return Ok(());
        }

        // When intercept mode is enabled, send ALL Guide button events over DBus
        if matches!(self.intercept_mode, InterceptMode::Pass)
            && cap == Capability::Gamepad(Gamepad::Button(GamepadButton::Guide))
        {
            let event = TargetCommand::WriteEvent(event);
            #[allow(clippy::for_kv_map)]
            for (_, target) in &self.target_dbus_devices {
                target.send(event.clone()).await?;
            }
            return Ok(());
        }

        // TODO: Only write the event to devices that are capabile of handling it
        let event = TargetCommand::WriteEvent(event);
        #[allow(clippy::for_kv_map)]
        for (_, target) in &self.target_devices {
            target.send(event.clone()).await?;
        }
        Ok(())
    }

    /// Loads the input capabilities to translate from the capability map
    fn load_capability_map(&mut self) -> Result<(), Box<dyn Error>> {
        let Some(map) = self.capability_map.as_ref() else {
            return Err("Cannot translate device capabilities without capability map!".into());
        };

        // Loop over each mapping and try to match source events
        for mapping in map.mapping.iter() {
            for source_event in mapping.source_events.iter() {
                let cap = source_event.clone().into();
                if cap == Capability::NotImplemented {
                    continue;
                }
                self.translatable_capabilities.push(cap);
            }
        }

        Ok(())
    }

    /// Sets the intercept mode to the given value
    fn set_intercept_mode(&mut self, mode: InterceptMode) {
        log::debug!("Setting intercept mode to: {:?}", mode);
        self.intercept_mode = mode;
    }

    /// Translates the given event into a different event based on the given
    /// [CapabilityMap].
    async fn translate_capability(&mut self, event: &NativeEvent) -> Result<(), Box<dyn Error>> {
        // Get the capability map to translate input events
        let Some(map) = self.capability_map.as_ref() else {
            return Err("Cannot translate device capability without capability map!".into());
        };

        // Add or remove the event from translatable_active_inputs.
        let event_capability = event.as_capability();
        let capability_idx = self
            .translatable_active_inputs
            .iter()
            .position(|c| c == &event_capability);
        if event.pressed() {
            if capability_idx.is_none() {
                log::trace!("Adding capability to active inputs: {:?}", event_capability);
                self.translatable_active_inputs.push(event_capability);
                log::trace!(
                    "Active translatable inputs: {:?}",
                    self.translatable_active_inputs
                );
            } else {
                return Ok(());
            }
        } else if capability_idx.is_some() {
            log::trace!(
                "Removing capability from active inputs: {:?}",
                event_capability
            );
            let idx = capability_idx.unwrap();
            self.translatable_active_inputs.remove(idx);
            log::trace!(
                "Active translatable inputs: {:?}",
                self.translatable_active_inputs
            );
        } else {
            return Ok(());
        }

        // Keep a list of events to emit
        let mut emit_queue = Vec::new();

        // Loop over each mapping and try to match source events
        for mapping in map.mapping.iter() {
            // If the event was not pressed and it exists in the emitted_mappings array,
            // then we need to check to see if ALL of its events no longer exist in
            // translatable_active_inputs.
            if !event.pressed() && self.emitted_mappings.contains_key(&mapping.name) {
                let mut has_source_event_pressed = false;

                // Loop through each source capability in the mapping
                for source_event in mapping.source_events.iter() {
                    let cap = source_event.clone().into();
                    if cap == Capability::NotImplemented {
                        continue;
                    }
                    if self.translatable_active_inputs.contains(&cap) {
                        has_source_event_pressed = true;
                        break;
                    }
                }

                // If no more inputs are being pressed, send a release event.
                if !has_source_event_pressed {
                    let cap = mapping.target_event.clone().into();
                    if cap == Capability::NotImplemented {
                        continue;
                    }
                    let event = NativeEvent::new(cap, InputValue::Bool(false));
                    log::trace!("Adding event to emit queue: {:?}", event);
                    emit_queue.push(event);
                    self.emitted_mappings.remove(&mapping.name);
                }
            }

            // If the event is pressed, check for any matches to send a 'press' event
            if event.pressed() {
                let mut is_missing_source_event = false;
                for source_event in mapping.source_events.iter() {
                    let cap = source_event.clone().into();
                    if cap == Capability::NotImplemented {
                        continue;
                    }
                    if !self.translatable_active_inputs.contains(&cap) {
                        is_missing_source_event = true;
                        break;
                    }
                }

                if !is_missing_source_event {
                    let cap = mapping.target_event.clone().into();
                    if cap == Capability::NotImplemented {
                        continue;
                    }
                    let event = NativeEvent::new(cap, InputValue::Bool(true));
                    log::trace!("Adding event to emit queue: {:?}", event);
                    emit_queue.push(event);
                    self.emitted_mappings
                        .insert(mapping.name.clone(), mapping.clone());
                }
            }
        }

        // Emit the translated events
        for event in emit_queue {
            log::trace!("Emitting event: {:?}", event);
            self.handle_event(event).await?;
        }

        Ok(())
    }

    /// Translates the given event into a Vec of events based on the currently loaded
    /// [DeviceProfile]
    async fn translate_event(
        &self,
        event: &NativeEvent,
    ) -> Result<Vec<NativeEvent>, Box<dyn Error>> {
        // Lookup the profile mapping associated with this event capability. If
        // none is found, return the original un-translated event.
        let source_cap = event.as_capability();
        if let Some(mappings) = self.device_profile_config_map.get(&source_cap) {
            // Find which mapping in the device profile matches this source event
            let matched_mapping = mappings
                .iter()
                .find(|mapping| mapping.source_matches_properties(event));

            // If a mapping was found, translate the event based on the found
            // mapping.
            if let Some(mapping) = matched_mapping {
                log::trace!(
                    "Found translation for event {:?} in profile mapping: {}",
                    source_cap,
                    mapping.name
                );

                // Translate the event into the defined target event(s)
                let mut events = Vec::new();
                for target_event in mapping.target_events.iter() {
                    // TODO: We can cache this conversion for faster translation
                    let target_cap: Capability = target_event.clone().into();
                    let result = event.get_value().translate(
                        &source_cap,
                        &mapping.source_event,
                        &target_cap,
                        target_event,
                    );
                    let value = match result {
                        Ok(v) => v,
                        Err(err) => {
                            match err {
                                TranslationError::NotImplemented => {
                                    log::warn!(
                                        "Translation not implemented for profile mapping '{}': {:?} -> {:?}",
                                        mapping.name,
                                        source_cap,
                                        target_cap,
                                    );
                                    continue;
                                }
                                TranslationError::ImpossibleTranslation(msg) => {
                                    log::warn!(
                                        "Impossible translation for profile mapping '{}': {msg}",
                                        mapping.name
                                    );
                                    continue;
                                }
                                TranslationError::InvalidSourceConfig(msg) => {
                                    log::warn!("Invalid source event config in profile mapping '{}': {msg}", mapping.name);
                                    continue;
                                }
                                TranslationError::InvalidTargetConfig(msg) => {
                                    log::warn!("Invalid target event config in profile mapping '{}': {msg}", mapping.name);
                                    continue;
                                }
                            }
                        }
                    };
                    if matches!(value, InputValue::None) {
                        continue;
                    }

                    let event = NativeEvent::new_translated(source_cap.clone(), target_cap, value);
                    events.push(event);
                }

                return Ok(events);
            }
        }

        log::trace!("No translation mapping found for event: {:?}", source_cap);
        Ok(vec![event.clone()])
    }

    /// Executed whenever a source device is added to this [CompositeDevice].
    async fn on_source_device_added(
        &mut self,
        device_info: SourceDeviceInfo,
    ) -> Result<(), Box<dyn Error>> {
        self.add_source_device(device_info)?;
        self.run_source_devices().await?;
        log::debug!(
            "Finished adding source device. All sources: {:?}",
            self.source_devices_used
        );
        Ok(())
    }

    /// Executed whenever a source device is removed from this [CompositeDevice]
    async fn on_source_device_removed(&mut self, id: String) -> Result<(), Box<dyn Error>> {
        if id.starts_with("evdev://") {
            let name = id.strip_prefix("evdev://").unwrap();
            let path = format!("/dev/input/{}", name);

            if let Some(idx) = self.source_device_paths.iter().position(|str| str == &path) {
                self.source_device_paths.remove(idx);
            };

            if let Some(idx) = self.source_devices_used.iter().position(|str| str == &id) {
                self.source_devices_used.remove(idx);
            };
            self.source_devices_blocked.remove(&id);
        }
        if id.starts_with("hidraw://") {
            let name = id.strip_prefix("hidraw://").unwrap();
            let path = format!("/dev/{}", name);

            if let Some(idx) = self.source_device_paths.iter().position(|str| str == &path) {
                self.source_device_paths.remove(idx);
            };

            if let Some(idx) = self.source_devices_used.iter().position(|str| str == &id) {
                self.source_devices_used.remove(idx);
            };
            self.source_devices_blocked.remove(&id);
        }

        log::debug!(
            "Current source device paths: {:?}",
            self.source_device_paths
        );
        log::debug!(
            "Current source devices used: {:?}",
            self.source_devices_used
        );

        Ok(())
    }

    /// Creates and adds a source device using the given [SourceDeviceInfo]
    fn add_source_device(&mut self, device_info: SourceDeviceInfo) -> Result<(), Box<dyn Error>> {
        let device_info = device_info.clone();
        match device_info.clone() {
            SourceDeviceInfo::EvdevDeviceInfo(info) => {
                // Create an instance of the device
                log::debug!("Adding source device: {:?}", info);
                let device = source::evdev::EventDevice::new(info.clone(), self.tx.clone());

                // Get the capabilities of the source device.
                // TODO: When we *remove* a source device, we also need to remove
                // capabilities
                let capabilities = device.get_capabilities()?;
                for cap in capabilities {
                    if self.translatable_capabilities.contains(&cap) {
                        continue;
                    }
                    self.capabilities.insert(cap);
                }

                // TODO: Based on the capability map in the config, translate
                // the capabilities.
                // Keep track of the source device
                let id = device.get_id();
                let device_path = device.get_device_path();
                let source_device = source::SourceDevice::EventDevice(device);
                self.source_devices_discovered.push(source_device);
                self.source_device_paths.push(device_path);
                self.source_devices_used.push(id.clone());

                // Check if this device should be blocked from sending events to target devices.
                if let Some(device_config) = self.config.get_matching_device(&device_info) {
                    if let Some(blocked) = device_config.blocked {
                        if blocked {
                            self.source_devices_blocked.insert(id);
                        }
                    }
                };
            }

            SourceDeviceInfo::HIDRawDeviceInfo(info) => {
                log::debug!("Adding source device: {:?}", info);
                let device = source::hidraw::HIDRawDevice::new(info, self.tx.clone());

                // Get the capabilities of the source device.
                let capabilities = device.get_capabilities()?;
                for cap in capabilities {
                    if self.translatable_capabilities.contains(&cap) {
                        continue;
                    }
                    self.capabilities.insert(cap);
                }

                let id = device.get_id();
                let device_path = device.get_device_path();
                let source_device = source::SourceDevice::HIDRawDevice(device);
                self.source_devices_discovered.push(source_device);
                self.source_device_paths.push(device_path);
                self.source_devices_used.push(id.clone());

                // Check if this device should be blocked from sending events to target devices.
                if let Some(device_config) = self.config.get_matching_device(&device_info) {
                    if let Some(blocked) = device_config.blocked {
                        if blocked {
                            self.source_devices_blocked.insert(id);
                        }
                    }
                };
            }
            SourceDeviceInfo::IIODeviceInfo(info) => {
                log::debug!("Adding source device: {:?}", info);
                let device = source::iio::IIODevice::new(info, self.tx.clone());

                // Get the capabilities of the source device.
                let capabilities = device.get_capabilities()?;
                for cap in capabilities {
                    if self.translatable_capabilities.contains(&cap) {
                        continue;
                    }
                    self.capabilities.insert(cap);
                }

                let id = device.get_id();
                let device_path = device.get_device_path();
                let source_device = source::SourceDevice::IIODevice(device);
                self.source_devices_discovered.push(source_device);
                self.source_device_paths.push(device_path);
                self.source_devices_used.push(id.clone());

                // Check if this device should be blocked from sending events to target devices.
                if let Some(device_config) = self.config.get_matching_device(&device_info) {
                    if let Some(blocked) = device_config.blocked {
                        if blocked {
                            self.source_devices_blocked.insert(id);
                        }
                    }
                };
            }
        }

        Ok(())
    }

    /// Load the given device profile from the given path
    pub fn load_device_profile_from_path(&mut self, path: String) -> Result<(), Box<dyn Error>> {
        log::debug!("Loading device profile from path: {path}");
        // Remove all outdated capability mappings.
        log::debug!("Clearing old device profile mappings");
        self.device_profile_config_map.clear();

        // Load and parse the device profile
        let profile = DeviceProfile::from_yaml_file(path.clone())?;
        self.device_profile = Some(profile.name.clone());

        // Loop through every mapping in the profile, extract the source and target events,
        // and map them into our profile map.
        for mapping in profile.mapping.iter() {
            log::debug!("Loading mapping from profile: {}", mapping.name);

            // Convert the source event configuration in the mapping into a
            // capability that can be easily matched on during event translation
            let source_event_cap: Capability = mapping.source_event.clone().into();

            // Convert the target events configuration into a vector of capabilities
            // that can be easily used to create translated events.
            let mut target_events_caps = Vec::new();
            for cap_config in mapping.target_events.clone() {
                let cap: Capability = cap_config.into();
                target_events_caps.push(cap);
            }

            // Insert the translation config for this event
            let config_map = self
                .device_profile_config_map
                .entry(source_event_cap)
                .or_default();
            config_map.push(mapping.clone());
        }

        log::debug!("Successfully loaded device profile: {}", profile.name);
        Ok(())
    }
}
