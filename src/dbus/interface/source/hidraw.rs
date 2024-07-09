use std::error::Error;

use zbus::{fdo, Connection};
use zbus_macros::interface;

use crate::{input::source::hidraw::get_dbus_path, udev::device::UdevDevice};

/// DBusInterface exposing information about a HIDRaw device
pub struct SourceHIDRawInterface {
    device: UdevDevice,
}

impl SourceHIDRawInterface {
    pub fn new(device: UdevDevice) -> SourceHIDRawInterface {
        SourceHIDRawInterface { device }
    }

    /// Creates a new instance of the source hidraw interface on DBus. Returns
    /// a structure with information about the source device.
    pub async fn listen_on_dbus(
        conn: Connection,
        sys_name: String,
        device: UdevDevice,
    ) -> Result<(), Box<dyn Error>> {
        log::debug!("Starting to listen on dbus interface for {sys_name}");
        let path = get_dbus_path(sys_name.clone());
        log::debug!("Got dbus path {path}");

        let iface = SourceHIDRawInterface::new(device);
        log::debug!("Created interface for {sys_name}");
        tokio::task::spawn(async move {
            log::debug!("Starting dbus interface: {path}");
            let result = conn.object_server().at(path.clone(), iface).await;
            if let Err(e) = result {
                log::debug!("Failed to start dbus interface {path}: {e:?}");
            } else {
                log::debug!("Started dbus interface: {path}");
            }
        });
        Ok(())
    }
}

#[interface(name = "org.shadowblip.Input.Source.HIDRawDevice")]
impl SourceHIDRawInterface {
    #[zbus(property)]
    async fn name(&self) -> fdo::Result<String> {
        Ok(self.device.name())
    }

    #[zbus(property)]
    async fn dev_path(&self) -> fdo::Result<String> {
        Ok(self.device.devnode())
    }

    #[zbus(property)]
    async fn id_product(&self) -> fdo::Result<String> {
        Ok(format!("{:04x}", self.device.id_product()))
    }

    #[zbus(property)]
    async fn id_vendor(&self) -> fdo::Result<String> {
        Ok(format!("{:04x}", self.device.id_vendor()))
    }

    #[zbus(property)]
    async fn interface_number(&self) -> fdo::Result<i32> {
        Ok(self.device.interface_number())
    }

    #[zbus(property)]
    async fn manufacturer(&self) -> fdo::Result<String> {
        Ok(self.device.manufacturer())
    }

    #[zbus(property)]
    async fn product(&self) -> fdo::Result<String> {
        Ok(self.device.product())
    }

    #[zbus(property)]
    async fn serial_number(&self) -> fdo::Result<String> {
        Ok(self.device.serial_number())
    }

    #[zbus(property)]
    async fn sysfs_path(&self) -> fdo::Result<String> {
        Ok(self.device.devpath())
    }
}
