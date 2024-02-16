use std::{collections::HashMap, error::Error};

use tokio::{
    sync::{broadcast, mpsc},
    task::JoinSet,
};
use zbus::{fdo, Connection, SignalContext};
use zbus_macros::dbus_interface;

use crate::{
    config::CompositeDeviceConfig,
    input::{event::native::NativeEvent, source},
    udev::{hide_device, unhide_device},
};

use super::{capability, event::Event, source::SourceDevice, target::TargetDevice};

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
    SourceDeviceAdded,
    SourceDeviceStopped(String),
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

    /// Emitted when an input event occurs when the device is in intercept mode
    #[dbus_interface(signal)]
    async fn input_event(ctxt: &SignalContext<'_>, event: String, value: f64) -> zbus::Result<()>;
}

/// Defines a handle to a [CompositeDevice] for communication
#[derive(Debug)]
pub struct Handle {
    tx: broadcast::Sender<Command>,
    rx: broadcast::Receiver<Command>,
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
    /// Unique identifiers for running source devices. E.g. ["evdev://event0"]
    source_devices_used: Vec<String>,
    /// Map of DBus paths to their respective transmitter channel.
    /// E.g. {"/org/shadowblip/InputPlumber/devices/target/gamepad0": <Sender>}
    target_devices: HashMap<String, mpsc::Sender<NativeEvent>>,
    /// List of paths to [DBusDevice] targets
    /// E.g. ["/org/shadowblip/InputPlumber/devices/target/dbus0"]
    target_dbus_devices: Vec<String>,
}

impl CompositeDevice {
    pub fn new(conn: Connection, config: CompositeDeviceConfig) -> Result<Self, Box<dyn Error>> {
        let (tx, rx) = broadcast::channel(BUFFER_SIZE);
        let mut source_devices: Vec<SourceDevice> = Vec::new();
        let mut source_device_paths: Vec<String> = Vec::new();
        let mut source_device_ids: Vec<String> = Vec::new();

        // Open evdev source devices based on configuration
        if let Some(evdev_devices) = config.get_matching_evdev()? {
            if !evdev_devices.is_empty() {
                log::debug!("Found event devices");
            }
            for info in evdev_devices {
                // Create an instance of the device
                log::debug!("Adding source device: {:?}", info);
                let device = source::evdev::EventDevice::new(info, tx.clone());

                // Get the capabilities of the source device.
                //let capabilities = device.get_capabilities()?;

                // TODO: Based on the capability map in the config, translate
                // the capabilities.

                // Keep track of the source device
                let id = device.get_id();
                let device_path = device.get_device_path();
                let source_device = source::SourceDevice::EventDevice(device);
                source_devices.push(source_device);
                source_device_paths.push(device_path);
                source_device_ids.push(id);
            }
        }

        // Open hidraw source devices based on configuration
        if let Some(hidraw_devices) = config.get_matching_hidraw()? {
            if !hidraw_devices.is_empty() {
                log::debug!("Found hidraw devices");
            }
            for info in hidraw_devices {
                log::debug!("Adding source device: {:?}", info);
                let device = source::hidraw::HIDRawDevice::new(info, tx.clone());

                // Get the capabilities of the source device.
                //let capabilities = device.get_capabilities()?;

                let id = device.get_id();
                let device_path = device.get_device_path();
                let source_device = source::SourceDevice::HIDRawDevice(device);
                source_devices.push(source_device);
                source_device_paths.push(device_path);
                source_device_ids.push(id);
            }
        }

        log::debug!("Finished adding source devices");

        Ok(Self {
            conn,
            dbus_path: None,
            intercept_mode: InterceptMode::None,
            tx,
            rx,
            source_devices,
            source_device_paths,
            source_device_ids,
            source_devices_used: Vec::new(),
            target_devices: HashMap::new(),
            target_dbus_devices: Vec::new(),
        })
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
        Ok(())
    }

    /// Starts the [CompositeDevice] and listens for events from all source
    /// devices to translate the events and send them to the appropriate target.
    pub async fn run(&mut self, targets: Vec<TargetDevice>) -> Result<(), Box<dyn Error>> {
        log::debug!("Starting composite device");

        // Start all source devices
        let mut tasks = self.run_source_devices().await?;

        // Start all target devices
        self.run_target_devices(targets)?;

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
                Command::SourceDeviceAdded => todo!(),
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
                Command::Stop => {
                    log::debug!("Stopping CompositeDevice");
                    break;
                }
            }
        }
        log::debug!("CompositeDevice stopped");

        // Unhide all source devices
        for source_path in self.source_device_paths.clone() {
            log::debug!("Un-hiding device: {}", source_path);
            unhide_device(source_path).await?;
        }

        // Wait on all tasks
        while let Some(res) = tasks.join_next().await {
            res?;
        }

        log::debug!("All source devices have closed");

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

    /// Return a list of source device paths (e.g. /dev/hidraw0, /dev/input/event0)
    /// that this composite device is managing
    fn get_source_device_paths(&self) -> Vec<String> {
        self.source_device_paths.clone()
    }

    /// Start and run the source devices that this composite device will
    /// consume.
    async fn run_source_devices(&mut self) -> Result<JoinSet<()>, Box<dyn Error>> {
        // Keep a list of all the tasks
        let mut tasks = JoinSet::new();

        // Hide all source devices
        // TODO: Make this configurable
        for source_path in self.source_device_paths.clone() {
            log::debug!("Hiding device: {}", source_path);
            hide_device(source_path).await?;
        }

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
                    tasks.spawn(async move {
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
                    tasks.spawn(async move {
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

        Ok(tasks)
    }

    /// Start and run the given target devices
    fn run_target_devices(&mut self, targets: Vec<TargetDevice>) -> Result<(), Box<dyn Error>> {
        for target in targets {
            match target {
                TargetDevice::Null => (),
                TargetDevice::Keyboard(_) => todo!(),
                TargetDevice::Mouse(mut mouse) => {
                    let event_tx = mouse.transmitter();
                    let Some(path) = mouse.get_dbus_path() else {
                        return Err("No DBus path found for target device".into());
                    };
                    self.target_devices.insert(path, event_tx);
                    tokio::spawn(async move {
                        if let Err(e) = mouse.run().await {
                            log::error!("Failed to run target mouse: {:?}", e);
                        }
                        log::debug!("Target mouse device closed");
                    });
                }
                TargetDevice::GenericGamepad(mut gamepad) => {
                    let event_tx = gamepad.transmitter();
                    let Some(path) = gamepad.get_dbus_path() else {
                        return Err("No DBus path found for target device".into());
                    };
                    self.target_devices.insert(path, event_tx);
                    tokio::spawn(async move {
                        if let Err(e) = gamepad.run().await {
                            log::error!("Failed to run target gamepad: {:?}", e);
                        }
                        log::debug!("Target gamepad device closed");
                    });
                }
                TargetDevice::XBox360(_) => todo!(),
                TargetDevice::DBus(mut device) => {
                    let event_tx = device.transmitter();
                    let Some(path) = device.get_dbus_path() else {
                        return Err("No DBus path found for target device".into());
                    };
                    self.target_devices.insert(path.clone(), event_tx);
                    self.target_dbus_devices.push(path);
                    tokio::spawn(async move {
                        if let Err(e) = device.run().await {
                            log::error!("Failed to run target dbus device: {:?}", e);
                        }
                        log::debug!("Target dbus device closed");
                    });
                }
            }
        }

        Ok(())
    }

    /// Process a single event from a source device. Events are piped through
    /// a translation layer, then dispatched to the appropriate target device(s)
    async fn process_event(&mut self, raw_event: Event) -> Result<(), Box<dyn Error>> {
        //log::debug!("Received event: {:?}", raw_event);

        // Convert the event into a NativeEvent
        let event: NativeEvent = match raw_event {
            Event::Evdev(event) => event.into(),
            Event::HIDRaw => todo!(),
            Event::Native(event) => event,
            Event::DBus(_) => todo!(),
        };

        // TODO: Check if the event needs to be translated based on the
        // capability map.

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
        for (path, target) in &self.target_devices {
            // If the device is in intercept mode, only send events to DBus
            // target devices.
            let is_dbus_device = self.is_dbus_device(path);
            if matches!(self.intercept_mode, InterceptMode::Always) {
                if is_dbus_device {
                    target.send(event.clone()).await?;
                }
                continue;
            }
            if is_dbus_device {
                continue;
            }
            target.send(event.clone()).await?;
        }
        Ok(())
    }

    /// Returns true if the given DBus path belongs to a [DBusDevice].
    fn is_dbus_device(&self, path: &String) -> bool {
        self.target_dbus_devices.contains(path)
    }

    /// Sets the intercept mode to the given value
    fn set_intercept_mode(&mut self, mode: InterceptMode) {
        log::debug!("Setting intercept mode to: {:?}", mode);
        self.intercept_mode = mode;
    }
}
