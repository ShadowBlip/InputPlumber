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
/// DBus instead of to the target devices.
#[derive(Debug, Clone)]
pub enum InterceptMode {
    /// Pass all input to the target devices
    None,
    /// Pass all inputs to the target devices except the guide button
    Pass,
    /// Intercept all input and send nothing to the target devices
    Always,
}

/// Evdev commands define all the different ways to interact with [EventDevice]
/// over a channel. These commands are processed in an asyncronous thread and
/// dispatched as they come in.
#[derive(Debug, Clone)]
pub enum Command {
    ProcessEvent(event::Event),
    SetInterceptMode(InterceptMode),
    Other,
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
    async fn intercept_mode(&self) -> fdo::Result<u32> {
        Ok(0)
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
    events: mpsc::Receiver<event::Event>,
    tx: broadcast::Sender<Command>,
    rx: broadcast::Receiver<Command>,
    source_devices: Vec<SourceDevice>,
    target_devices: Vec<TargetDevice>,
}

impl CompositeDevice {
    pub fn new(config: CompositeDeviceConfig) -> Result<Self, Box<dyn Error>> {
        let (tx, rx) = broadcast::channel(BUFFER_SIZE);
        let mut source_devices: Vec<SourceDevice> = Vec::new();

        // Create a channel for events to be sent through
        let (events_tx, events) = mpsc::channel(BUFFER_SIZE);

        // Open source devices based on configuration
        if let Some(evdev_devices) = config.get_matching_evdev()? {
            log::debug!("Found event devices");
            for info in evdev_devices {
                log::debug!("Adding source device: {:?}", info);
                let device = source::evdev::EventDevice::new(info, tx.clone(), events_tx.clone());
                let source_device = source::SourceDevice::EventDevice(device);
                source_devices.push(source_device);
            }
        }

        Ok(Self {
            intercept_mode: InterceptMode::None,
            config,
            events,
            tx,
            rx,
            source_devices,
            target_devices: Vec::new(),
        })
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

    /// Run the [CompositeDevice]
    pub async fn run(&mut self) -> Result<(), Box<dyn Error>> {
        // Keep a list of all the tasks
        let mut tasks = JoinSet::new();

        // Run all source devices
        let sources = self.source_devices.drain(..);
        for source in sources {
            match source {
                SourceDevice::EventDevice(device) => {
                    tasks.spawn(async move {
                        if let Err(e) = device.run().await {
                            log::error!("Failed running event device: {:?}", e);
                        }
                    });
                }
            }
        }

        // Wait for events from source devices
        //while let Some(event) = self.events.recv().await {
        //    self.process_event(event).await;
        //}

        // Process commands
        // Loop and listen for command events
        while let Ok(cmd) = self.rx.recv().await {
            log::debug!("Received command: {:?}", cmd);
            match cmd {
                Command::ProcessEvent(event) => self.process_event(event).await,
                Command::SetInterceptMode(mode) => self.set_intercept_mode(mode),
                Command::Other => todo!(),
            }
        }

        // Wait on all tasks
        while let Some(res) = tasks.join_next().await {
            res?;
        }

        log::debug!("All source devices have closed");

        Ok(())
    }

    /// Sets the intercept mode to the given value
    fn set_intercept_mode(&mut self, mode: InterceptMode) {
        log::debug!("Setting intercept mode to: {:?}", mode);
        self.intercept_mode = mode;
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
    }

    /// Creates a new instance of the composite device interface on DBus.
    pub async fn listen_on_dbus(
        &self,
        conn: Connection,
        path: String,
    ) -> Result<(), Box<dyn Error>> {
        let iface = DBusInterface::new(self.tx.clone());
        conn.object_server().at(path, iface).await?;
        Ok(())
    }
}
