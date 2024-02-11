use std::error::Error;

use tokio::{
    sync::{broadcast, mpsc},
    task::JoinSet,
};
use zbus::{fdo, Connection};
use zbus_macros::dbus_interface;

use crate::{
    config::CompositeDeviceConfig,
    input::{
        event::native::NativeEvent,
        source,
        target::{gamepad::GenericGamepad, xb360::XBox360Controller},
    },
};

use super::{event::Event, source::SourceDevice, target::TargetDevice};

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
    #[dbus_interface(property)]
    async fn name(&self) -> fdo::Result<String> {
        Ok("CompositeDevice".into())
    }

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
    intercept_mode: InterceptMode,
    config: CompositeDeviceConfig,
    tx: broadcast::Sender<Command>,
    rx: broadcast::Receiver<Command>,
    source_devices: Vec<SourceDevice>,
    source_device_paths: Vec<String>,
    source_device_ids: Vec<String>,
    source_devices_used: Vec<String>,
    target_devices: Vec<mpsc::Sender<NativeEvent>>,
}

impl CompositeDevice {
    pub fn new(config: CompositeDeviceConfig) -> Result<Self, Box<dyn Error>> {
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
                let capabilities = device.get_capabilities()?;

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
            intercept_mode: InterceptMode::None,
            config,
            tx,
            rx,
            source_devices,
            source_device_paths,
            source_device_ids,
            source_devices_used: Vec::new(),
            target_devices: Vec::new(),
        })
    }

    /// Run the [CompositeDevice]
    pub async fn run(&mut self) -> Result<(), Box<dyn Error>> {
        log::debug!("Starting composite device");
        // Keep a list of all the tasks
        let mut tasks = JoinSet::new();

        // Run all source devices
        let sources = self.source_devices.drain(..);
        for source in sources {
            match source {
                // If the source device is an event device (i.e. from /dev/input/),
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

        // Create and run all target devices
        let targets = self.create_target_devices()?;
        for target in targets {
            match target {
                TargetDevice::Null => (),
                TargetDevice::Keyboard(_) => todo!(),
                TargetDevice::Mouse(_) => todo!(),
                TargetDevice::GenericGamepad(mut gamepad) => {
                    let transmitter = gamepad.transmitter();
                    self.target_devices.push(transmitter);
                    tokio::spawn(async move {
                        if let Err(e) = gamepad.run().await {
                            log::error!("Failed to run target gamepad: {:?}", e);
                        }
                        log::debug!("Target gamepad device closed");
                    });
                }
                TargetDevice::XBox360(_) => todo!(),
            }
        }

        // Loop and listen for command events
        log::debug!("CompositeDevice started");
        while let Ok(cmd) = self.rx.recv().await {
            log::debug!("Received command: {:?}", cmd);
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
            }
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

    /// Returns an array of all source devices being used by this device.
    pub fn get_source_device_ids(&self) -> Vec<String> {
        self.source_device_ids.clone()
    }

    /// Return a list of source device paths (e.g. /dev/hidraw0, /dev/input/event0)
    /// that this composite device is managing
    fn get_source_device_paths(&self) -> Vec<String> {
        self.source_device_paths.clone()
    }

    /// Create target (output) devices to emulate. Returns the created devices
    /// as an array.
    fn create_target_devices(&self) -> Result<Vec<TargetDevice>, Box<dyn Error>> {
        log::debug!("Creating target devices");
        let mut target_devices: Vec<TargetDevice> = Vec::new();

        // Create a transmitter channel that target devices can use to communitcate
        // with the composite device
        let tx = self.transmitter();

        // Create the target devices to emulate based on the config
        let config = &self.config;
        let default_output_device = &config.default_output_device;
        let device_id = default_output_device.clone().unwrap_or("null".into());
        let gamepad_device = match device_id.as_str() {
            "xb360" => TargetDevice::XBox360(XBox360Controller::new(tx)),
            "null" | "none" => TargetDevice::Null,
            _ => TargetDevice::GenericGamepad(GenericGamepad::new(tx)),
        };
        target_devices.push(gamepad_device);
        log::debug!("Created target gamepad");

        // TODO: Create a keyboard device to emulate

        // TODO: Create a mouse device to emulate

        Ok(target_devices)
    }

    /// Process a single event from a source device. Events are piped through
    /// a translation layer, then dispatched to the appropriate target device(s)
    async fn process_event(&self, raw_event: Event) -> Result<(), Box<dyn Error>> {
        log::debug!("Received event: {:?}", raw_event);

        // Convert the event into a NativeEvent
        let event: NativeEvent = match raw_event {
            Event::Evdev(event) => event.into(),
            Event::HIDRaw => todo!(),
            Event::Native(event) => event,
            Event::DBus(_) => todo!(),
        };

        // TODO: Check if the event needs to be translated based on the
        // capability map.

        // Translate the event based on the device profile.

        // Send the event to the appropriate target device
        for target in &self.target_devices {
            target.send(event.clone()).await?;
        }

        Ok(())
    }

    /// Processes a single translated event. These events are piped to the
    /// appropriate target device(s)
    async fn process_translated_event(&self, event: Event) {}

    /// Translates the given event.
    async fn translate_event(&self, event: Event) -> Vec<Event> {
        Vec::new()
    }

    /// Creates a new instance of the composite device interface on DBus.
    pub async fn listen_on_dbus(
        &self,
        conn: Connection,
        path: String,
    ) -> Result<(), Box<dyn Error>> {
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let iface = DBusInterface::new(tx);
            if let Err(e) = conn.object_server().at(path, iface).await {
                log::error!("Failed to setup DBus interface for device: {:?}", e);
            }
        });
        Ok(())
    }

    /// Sets the intercept mode to the given value
    fn set_intercept_mode(&mut self, mode: InterceptMode) {
        log::debug!("Setting intercept mode to: {:?}", mode);
        self.intercept_mode = mode;
    }
}
