use std::error::Error;

use zbus::{fdo, Connection};
use zbus_macros::interface;

use crate::{input::source::evdev::get_dbus_path, procfs};

/// The [SourceEventDeviceInterface] provides a DBus interface that can be exposed for managing
/// a [Manager]. It works by sending command messages to a channel that the
/// [Manager] is listening on.
pub struct SourceEventDeviceInterface {
    info: procfs::device::Device,
}

impl SourceEventDeviceInterface {
    pub fn new(_handler: String, info: procfs::device::Device) -> SourceEventDeviceInterface {
        SourceEventDeviceInterface { info }
    }

    /// Creates a new instance of the source evdev interface on DBus. Returns
    /// a structure with information about the source device.
    pub async fn listen_on_dbus(
        conn: Connection,
        handler: String,
        info: procfs::device::Device,
    ) -> Result<(), Box<dyn Error>> {
        let path = get_dbus_path(handler.clone());
        let iface = SourceEventDeviceInterface::new(handler.clone(), info);
        conn.object_server().at(path, iface).await?;
        Ok(())
    }
}

#[interface(name = "org.shadowblip.Input.Source.EventDevice")]
impl SourceEventDeviceInterface {
    #[zbus(property)]
    async fn name(&self) -> fdo::Result<String> {
        Ok(self.info.name.clone())
    }

    #[zbus(property)]
    async fn handlers(&self) -> fdo::Result<Vec<String>> {
        Ok(self.info.handlers.clone())
    }

    #[zbus(property)]
    async fn phys_path(&self) -> fdo::Result<String> {
        Ok(self.info.phys_path.clone())
    }

    #[zbus(property)]
    async fn sysfs_path(&self) -> fdo::Result<String> {
        Ok(self.info.sysfs_path.clone())
    }

    #[zbus(property)]
    async fn unique_id(&self) -> fdo::Result<String> {
        Ok(self.info.unique_id.clone())
    }

    /// Returns the full path to the device handler (e.g. /dev/input/event3)
    #[zbus(property)]
    pub fn device_path(&self) -> fdo::Result<String> {
        let handlers = &self.info.handlers;
        for handler in handlers {
            if !handler.starts_with("event") {
                continue;
            }
            return Ok(format!("/dev/input/{}", handler.clone()));
        }
        Ok("".into())
    }
}
