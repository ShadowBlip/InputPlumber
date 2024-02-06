use std::error::Error;

use tokio::sync::broadcast;
use zbus::{fdo, Connection};
use zbus_macros::dbus_interface;

use crate::config::CompositeDeviceConfig;

use super::{source::SourceDevice, target::TargetDevice};

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
    pub fn new(config: CompositeDeviceConfig) -> Self {
        let (tx, rx) = broadcast::channel(128);
        Self {
            config,
            tx,
            rx,
            source_devices: Vec::new(),
            target_devices: Vec::new(),
        }
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
    pub async fn run(&self) -> Result<(), Box<dyn Error>> {
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
