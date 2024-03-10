use std::{collections::HashMap, error::Error, str::FromStr};

use tokio::{
    sync::{broadcast, mpsc},
    task::JoinSet,
};
use zbus::{fdo, Connection};
use zbus_macros::dbus_interface;

use crate::{
    config::{CapabilityMap, CapabilityMapping, CompositeDeviceConfig},
    input::{event::native::NativeEvent, source},
    udev::{hide_device, unhide_device},
};

use super::{
    capability,
    event::{native::InputValue, Event},
    manager::SourceDeviceInfo,
    source::SourceDevice,
    target::TargetCommand,
};

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
    ProcessEvent(Event),
    SetInterceptMode(InterceptMode),
    GetInterceptMode(mpsc::Sender<InterceptMode>),
    GetSourceDevicePaths(mpsc::Sender<Vec<String>>),
    GetTargetDevicePaths(mpsc::Sender<Vec<String>>),
    GetDBusDevicePaths(mpsc::Sender<Vec<String>>),
    SourceDeviceAdded(SourceDeviceInfo),
    SourceDeviceStopped(String),
    SourceDeviceRemoved(String),
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
    /// Capability mapping for the CompositeDevice
    capability_map: Option<CapabilityMap>,
    /// List of input capabilities that can be translated by the capability map
    translatable_capabilities: Vec<capability::Capability>,
    /// List of currently "pressed" actions used to translate multiple input
    /// sequences into a single input event.
    translatable_active_inputs: Vec<capability::Capability>,
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
    /// Source devices that this composite device will consume.
    source_devices: Vec<SourceDevice>,
    /// Physical device path for source devices. E.g. ["/dev/input/event0"]
    source_device_paths: Vec<String>,
    /// Unique identifiers for source devices. E.g. ["evdev://event0"]
    source_device_ids: Vec<String>,
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
            capability_map,
            translatable_capabilities: Vec::new(),
            translatable_active_inputs: Vec::new(),
            emitted_mappings: HashMap::new(),
            dbus_path: None,
            intercept_mode: InterceptMode::None,
            tx,
            rx,
            source_devices: Vec::new(),
            source_device_paths: Vec::new(),
            source_device_ids: Vec::new(),
            source_device_tasks: JoinSet::new(),
            source_devices_used: Vec::new(),
            target_devices: HashMap::new(),
            target_dbus_devices: HashMap::new(),
        };
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

        // Load the capability map if one was defined
        if self.capability_map.is_some() {
            self.load_capability_map()?;
        }

        // Start all source devices
        self.run_source_devices().await?;

        // Keep track of all target devices
        self.target_devices = targets;

        // Loop and listen for command events
        log::debug!("CompositeDevice started");
        while let Ok(cmd) = self.rx.recv().await {
            //log::debug!("Received command: {:?}", cmd);
            match cmd {
                Command::ProcessEvent(event) => {
                    if let Err(e) = self.process_event(event).await {
                        log::error!("Failed to process event: {:?}", e);
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
                    log::debug!("Detected source device removal: {}", device_id);
                    let idx = self
                        .source_devices_used
                        .iter()
                        .position(|v| v.clone() == device_id);
                    if let Some(idx) = idx {
                        self.source_devices_used.remove(idx);
                    }
                    if self.source_devices_used.is_empty() {
                        break;
                    }
                }
                Command::SourceDeviceRemoved(id) => {
                    if let Err(e) = self.on_source_device_removed(id).await {
                        log::error!("Failed to remove source device: {:?}", e);
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
    pub fn get_source_device_ids(&self) -> Vec<String> {
        self.source_device_ids.clone()
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
            log::debug!("Hiding device: {}", source_path);
            hide_device(source_path).await?;
        }

        log::debug!("Starting new source devices");
        // Start listening for events from all source devices
        let sources = self.source_devices.drain(..);
        for source in sources {
            match source {
                // If the source device is an event device (i.e. from /dev/input/eventXX),
                // then start listening for inputs from that device.
                SourceDevice::EventDevice(device) => {
                    let device_id = device.get_id();
                    self.source_devices_used.push(device_id.clone());
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
                    self.source_devices_used.push(device_id.clone());
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
            }
        }
        log::debug!("All source device tasks started");
        Ok(())
    }

    /// Process a single event from a source device. Events are piped through
    /// a translation layer, then dispatched to the appropriate target device(s)
    async fn process_event(&mut self, raw_event: Event) -> Result<(), Box<dyn Error>> {
        log::trace!("Received event: {:?}", raw_event);

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
        if cap == capability::Capability::NotImplemented {
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

        // TODO: Translate the event based on the device profile.

        // Process the event depending on the intercept mode
        let mode = self.intercept_mode.clone();
        if matches!(mode, InterceptMode::Pass) {
            let capability = event.as_capability();
            if let capability::Capability::Gamepad(gamepad) = capability {
                if let capability::Gamepad::Button(btn) = gamepad {
                    if let capability::GamepadButton::Guide = btn {
                        // Set the intercept mode while the button is pressed
                        if event.pressed() {
                            log::debug!("Intercepted guide button press");
                            self.set_intercept_mode(InterceptMode::Always);
                        }
                    }
                }
            }
        }

        // Write the event
        self.write_event(event).await?;

        Ok(())
    }

    /// Writes the given event to the appropriate target device.
    async fn write_event(&self, event: NativeEvent) -> Result<(), Box<dyn Error>> {
        let event = TargetCommand::WriteEvent(event);

        // If the device is in intercept mode, only send events to DBus
        // target devices.
        if matches!(self.intercept_mode, InterceptMode::Always) {
            for (_, target) in &self.target_dbus_devices {
                target.send(event.clone()).await?;
            }
            return Ok(());
        }

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
                if let Some(keyboard) = source_event.keyboard.as_ref() {
                    let key = capability::Keyboard::from_str(keyboard.as_str());
                    if key.is_err() {
                        return Err(
                            format!("Invalid or unimplemented capability: {keyboard}").into()
                        );
                    }
                    let key = key.unwrap();
                    let cap = capability::Capability::Keyboard(key);
                    self.translatable_capabilities.push(cap)
                }
                if let Some(gamepad) = source_event.gamepad.as_ref() {
                    unimplemented!();
                }
                if let Some(mouse) = source_event.mouse.as_ref() {
                    unimplemented!();
                }
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
                let mut has_keys_pressed = false;

                // Loop through each source capability in the mapping
                for source_event in mapping.source_events.iter() {
                    if let Some(keyboard) = source_event.keyboard.as_ref() {
                        let key = capability::Keyboard::from_str(keyboard.as_str());
                        if key.is_err() {
                            log::error!(
                                "Invalid or unimplemented capability: {}",
                                keyboard.as_str()
                            );
                            continue;
                        }
                        let key = key.unwrap();
                        let cap = capability::Capability::Keyboard(key);
                        if self.translatable_active_inputs.contains(&cap) {
                            has_keys_pressed = true;
                            break;
                        }
                    }
                    if let Some(gamepad) = source_event.gamepad.as_ref() {
                        unimplemented!();
                    }
                    if let Some(mouse) = source_event.mouse.as_ref() {
                        unimplemented!();
                    }
                }

                // If no more inputs are being pressed, send a release event.
                if !has_keys_pressed {
                    if let Some(keyboard) = &mapping.target_event.keyboard {
                        let key = capability::Keyboard::from_str(keyboard.as_str());
                        if key.is_err() {
                            log::error!(
                                "Invalid or unimplemented capability: {}",
                                keyboard.as_str()
                            );
                            continue;
                        }
                        let key = key.unwrap();
                        let cap = capability::Capability::Keyboard(key);
                        let event = NativeEvent::new(cap, InputValue::Bool(false));
                        log::trace!("Adding event to emit queue: {:?}", event);
                        emit_queue.push(event);
                        self.emitted_mappings.remove(&mapping.name);
                    }
                    if let Some(gamepad) = &mapping.target_event.gamepad {
                        if let Some(button) = gamepad.button.as_ref() {
                            let btn = capability::GamepadButton::from_str(button.as_str());
                            if btn.is_err() {
                                log::error!(
                                    "Invalid or unimplemented capability: {}",
                                    button.as_str()
                                );
                                continue;
                            }
                            let btn = btn.unwrap();
                            let cap =
                                capability::Capability::Gamepad(capability::Gamepad::Button(btn));
                            let event = NativeEvent::new(cap, InputValue::Bool(false));
                            log::trace!("Adding event to emit queue: {:?}", event);
                            emit_queue.push(event);
                            self.emitted_mappings.remove(&mapping.name);
                        }
                    }
                    if let Some(mouse) = &mapping.target_event.mouse {
                        unimplemented!();
                    }
                }
            }

            // If the event is pressed, check for any matches to send a 'press' event
            if event.pressed() {
                let mut is_missing_keys = false;
                for source_event in mapping.source_events.iter() {
                    if let Some(keyboard) = source_event.keyboard.as_ref() {
                        let key = capability::Keyboard::from_str(keyboard.as_str());
                        if key.is_err() {
                            log::error!(
                                "Invalid or unimplemented capability: {}",
                                keyboard.as_str()
                            );
                            continue;
                        }
                        let key = key.unwrap();
                        let cap = capability::Capability::Keyboard(key);
                        if !self.translatable_active_inputs.contains(&cap) {
                            is_missing_keys = true;
                            break;
                        }
                    }
                    if let Some(gamepad) = source_event.gamepad.as_ref() {
                        if let Some(button) = gamepad.button.as_ref() {
                            let btn = capability::GamepadButton::from_str(button.as_str());
                            if btn.is_err() {
                                log::error!(
                                    "Invalid or unimplemented capability: {}",
                                    button.as_str()
                                );
                                continue;
                            }
                            let btn = btn.unwrap();
                            let cap =
                                capability::Capability::Gamepad(capability::Gamepad::Button(btn));
                            if !self.translatable_active_inputs.contains(&cap) {
                                is_missing_keys = true;
                                break;
                            }
                        }
                    }
                    if let Some(mouse) = source_event.mouse.as_ref() {
                        unimplemented!();
                    }
                }

                if !is_missing_keys {
                    if let Some(keyboard) = &mapping.target_event.keyboard {
                        let key = capability::Keyboard::from_str(keyboard.as_str());
                        if key.is_err() {
                            log::error!(
                                "Invalid or unimplemented capability: {}",
                                keyboard.as_str()
                            );
                            continue;
                        }
                        let key = key.unwrap();
                        let cap = capability::Capability::Keyboard(key);
                        let event = NativeEvent::new(cap, InputValue::Bool(true));
                        log::trace!("Adding event to emit queue: {:?}", event);
                        emit_queue.push(event);
                        self.emitted_mappings
                            .insert(mapping.name.clone(), mapping.clone());
                        todo!();
                    }
                    if let Some(gamepad) = &mapping.target_event.gamepad {
                        if let Some(button) = gamepad.button.as_ref() {
                            let btn = capability::GamepadButton::from_str(button.as_str());
                            if btn.is_err() {
                                log::error!(
                                    "Invalid or unimplemented capability: {}",
                                    button.as_str()
                                );
                                continue;
                            }
                            let btn = btn.unwrap();
                            let cap =
                                capability::Capability::Gamepad(capability::Gamepad::Button(btn));
                            let event = NativeEvent::new(cap, InputValue::Bool(true));
                            log::trace!("Adding event to emit queue: {:?}", event);
                            emit_queue.push(event);
                            self.emitted_mappings
                                .insert(mapping.name.clone(), mapping.clone());
                        }
                    }
                    if let Some(mouse) = &mapping.target_event.mouse {
                        unimplemented!();
                    }
                }
            }
        }

        // Emit the translated events
        for event in emit_queue {
            log::trace!("Emitting event: {:?}", event);
            self.tx.send(Command::ProcessEvent(Event::Native(event)))?;
        }

        Ok(())
    }

    async fn on_source_device_added(
        &mut self,
        device_info: SourceDeviceInfo,
    ) -> Result<(), Box<dyn Error>> {
        self.add_source_device(device_info)?;
        self.run_source_devices().await?;
        log::debug!(
            "Finished adding source device. All sources: {:?}",
            self.source_device_ids
        );
        Ok(())
    }

    async fn on_source_device_removed(&mut self, id: String) -> Result<(), Box<dyn Error>> {
        if id.starts_with("evdev://") {
            let name = id.strip_prefix("evdev://").unwrap();
            let path = format!("/dev/input/{}", name);

            if let Some(idx) = self.source_device_paths.iter().position(|str| str == &path) {
                self.source_device_paths.remove(idx);
            };

            let Some(idx) = self.source_device_ids.iter().position(|str| str == &id) else {
                return Ok(());
            };

            self.source_device_ids.remove(idx);
        }
        if id.starts_with("hidraw://") {
            let name = id.strip_prefix("hidraw://").unwrap();
            let path = format!("/dev/{}", name);

            if let Some(idx) = self.source_device_paths.iter().position(|str| str == &path) {
                self.source_device_paths.remove(idx);
            };

            let Some(idx) = self.source_device_ids.iter().position(|str| str == &id) else {
                return Ok(());
            };

            self.source_device_ids.remove(idx);
        }

        Ok(())
    }

    fn add_source_device(&mut self, device_info: SourceDeviceInfo) -> Result<(), Box<dyn Error>> {
        let device_info = device_info.clone();
        match device_info {
            SourceDeviceInfo::EvdevDeviceInfo(info) => {
                // Create an instance of the device
                log::debug!("Adding source device: {:?}", info);
                let device = source::evdev::EventDevice::new(info, self.tx.clone());
                // Get the capabilities of the source device.
                //let capabilities = device.get_capabilities()?;

                // TODO: Based on the capability map in the config, translate
                // the capabilities.
                // Keep track of the source device
                let id = device.get_id();
                let device_path = device.get_device_path();
                let source_device = source::SourceDevice::EventDevice(device);
                self.source_devices.push(source_device);
                self.source_device_paths.push(device_path);
                self.source_device_ids.push(id);
            }

            SourceDeviceInfo::HIDRawDeviceInfo(info) => {
                log::debug!("Adding source device: {:?}", info);
                let device = source::hidraw::HIDRawDevice::new(info, self.tx.clone());

                // Get the capabilities of the source device.
                //let capabilities = device.get_capabilities()?;

                let id = device.get_id();
                let device_path = device.get_device_path();
                let source_device = source::SourceDevice::HIDRawDevice(device);
                self.source_devices.push(source_device);
                self.source_device_paths.push(device_path);
                self.source_device_ids.push(id);
            }
        }
        Ok(())
    }
}
