use std::error::Error;

use tokio::{
    sync::{broadcast, mpsc},
    task::JoinSet,
};
use zbus::{fdo, Connection};
use zbus_macros::dbus_interface;

use crate::{config::CompositeDeviceConfig, input::source};

use super::{event, source::SourceDevice, target::TargetDevice};

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
    ProcessEvent(event::Event),
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
    target_devices: Vec<TargetDevice>,
}

impl CompositeDevice {
    pub fn new(config: CompositeDeviceConfig) -> Result<Self, Box<dyn Error>> {
        let (tx, rx) = broadcast::channel(BUFFER_SIZE);
        let mut source_devices: Vec<SourceDevice> = Vec::new();
        let mut source_device_paths: Vec<String> = Vec::new();
        let mut source_device_ids: Vec<String> = Vec::new();

        // Open source devices based on configuration
        if let Some(evdev_devices) = config.get_matching_evdev()? {
            log::debug!("Found event devices");
            for info in evdev_devices {
                // Create an instance of the device
                log::debug!("Adding source device: {:?}", info);
                let device = source::evdev::EventDevice::new(info, tx.clone());

                // Keep track of the source device
                let id = device.get_id();
                let device_path = device.get_device_path();
                let source_device = source::SourceDevice::EventDevice(device);
                source_devices.push(source_device);
                source_device_paths.push(device_path);
                source_device_ids.push(id);
            }
        }
        log::debug!("Finished adding event devices");

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
            }
        }

        // Loop and listen for command events
        log::debug!("CompositeDevice started");
        while let Ok(cmd) = self.rx.recv().await {
            log::debug!("Received command: {:?}", cmd);
            match cmd {
                Command::ProcessEvent(event) => self.process_event(event).await,
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

    /// Process a single event
    async fn process_event(&self, event: event::Event) {
        log::debug!("Received event: {:?}", event);
        //match event {
        //    event::Event::Evdev(_) => todo!(),
        //    event::Event::HIDRaw => todo!(),
        //    event::Event::Native(_) => todo!(),
        //    event::Event::DBus(_) => todo!(),
        //}

        // Translate the event based on the device profile.
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
