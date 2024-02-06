use std::error::Error;

use tokio::{
    join,
    sync::broadcast,
    task::{JoinHandle, JoinSet},
};
use zbus::{fdo, Connection};
use zbus_macros::dbus_interface;

use crate::{config::CompositeDeviceConfig, input::source};

use super::{
    source::{evdev, SourceDevice},
    target::TargetDevice,
};

/// Evdev commands define all the different ways to interact with [EventDevice]
/// over a channel. These commands are processed in an asyncronous thread and
/// dispatched as they come in.
#[derive(Debug, Clone)]
pub enum Command {
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
    config: CompositeDeviceConfig,
    tx: broadcast::Sender<Command>,
    rx: broadcast::Receiver<Command>,
    source_devices: Vec<SourceDevice>,
    target_devices: Vec<TargetDevice>,
}

impl CompositeDevice {
    pub fn new(config: CompositeDeviceConfig) -> Result<Self, Box<dyn Error>> {
        let (tx, rx) = broadcast::channel(128);
        let mut source_devices: Vec<SourceDevice> = Vec::new();

        // Open source devices based on configuration
        if let Some(evdev_devices) = config.get_matching_evdev()? {
            log::debug!("Found event devices");
            for info in evdev_devices {
                log::debug!("Adding source device: {:?}", info);
                let device = source::evdev::EventDevice::new(info, tx.clone());
                let source_device = source::SourceDevice::EventDevice(device);
                source_devices.push(source_device);
            }
        }

        Ok(Self {
            config,
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

        // Wait on all tasks
        while let Some(res) = tasks.join_next().await {
            res?;
        }

        log::debug!("All source devices have closed");

        Ok(())
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
