use std::error::Error;

use evdev::{AttributeSet, Device, EventSummary, EventType, InputEvent, KeyCode};
use tokio::sync::broadcast;
use zbus::{fdo, Connection};
use zbus_macros::dbus_interface;

use crate::{constants::BUS_PREFIX, input::composite_device, procfs};

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
    handler: String,
    info: procfs::device::Device,
}

impl DBusInterface {
    pub fn new(handler: String, info: procfs::device::Device) -> DBusInterface {
        DBusInterface { info, handler }
    }

    /// Creates a new instance of the source evdev interface on DBus. Returns
    /// a structure with information about the source device.
    pub async fn listen_on_dbus(
        conn: Connection,
        handler: String,
        info: procfs::device::Device,
    ) -> Result<(), Box<dyn Error>> {
        let path = get_dbus_path(handler.clone());
        let iface = DBusInterface::new(handler.clone(), info);
        conn.object_server().at(path, iface).await?;
        Ok(())
    }
}

#[dbus_interface(name = "org.shadowblip.Input.Source.EventDevice")]
impl DBusInterface {
    #[dbus_interface(property)]
    async fn name(&self) -> fdo::Result<String> {
        Ok(self.info.name.clone())
    }

    #[dbus_interface(property)]
    async fn handlers(&self) -> fdo::Result<Vec<String>> {
        Ok(self.info.handlers.clone())
    }

    #[dbus_interface(property)]
    async fn phys_path(&self) -> fdo::Result<String> {
        Ok(self.info.phys_path.clone())
    }

    #[dbus_interface(property)]
    async fn sysfs_path(&self) -> fdo::Result<String> {
        Ok(self.info.sysfs_path.clone())
    }

    #[dbus_interface(property)]
    async fn unique_id(&self) -> fdo::Result<String> {
        Ok(self.info.unique_id.clone())
    }
}

/// [EventDevice] represents an input device using the input subsystem.
#[derive(Debug)]
pub struct EventDevice {
    info: procfs::device::Device,
    tx: broadcast::Sender<composite_device::Command>,
}

impl EventDevice {
    pub fn new(
        info: procfs::device::Device,
        tx: broadcast::Sender<composite_device::Command>,
    ) -> Self {
        Self { info, tx }
    }

    /// Run the source device handler
    pub async fn run(&self) -> Result<(), Box<dyn Error>> {
        let path = self.get_device_path();
        log::debug!("Opening device at: {}", path);
        let device = Device::open(path.clone())?;
        log::debug!("Reading events from {}", path);
        let mut events = device.into_event_stream()?;
        while let Ok(event) = events.next_event().await {
            log::debug!("Received event: {:?}", event);
            self.tx.send(composite_device::Command::Other)?;
        }
        log::debug!("Stopped reading device events");

        Ok(())
    }

    /// Returns the full path to the device handler (e.g. /dev/input/event3)
    fn get_device_path(&self) -> String {
        let handlers = &self.info.handlers;
        for handler in handlers {
            if !handler.starts_with("event") {
                continue;
            }
            return format!("/dev/input/{}", handler.clone());
        }
        "".into()
    }

    /// Processes all physical inputs for this device. This
    /// should be called in a tight loop to process input events.
    fn process_input(&mut self, event: InputEvent) {
        log::debug!("Process input for event: {:?}", event);
        self.process_phys_event(event);
    }

    /// Processes a single physical gamepad event. Depending on the intercept mode,
    /// this usually means forwarding events from the physical gamepad to the
    /// virtual gamepad. In other cases we want to translate physical input into
    /// DBus events that only an overlay will respond to.
    fn process_phys_event(&mut self, event: InputEvent) {}
}

/// Returns the DBus object path for evdev devices
pub fn get_dbus_path(handler: String) -> String {
    format!("{}/devices/source/{}", BUS_PREFIX, handler.clone())
}
