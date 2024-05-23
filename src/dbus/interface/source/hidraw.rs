use std::error::Error;

use hidapi::DeviceInfo;
use zbus::{fdo, Connection};
use zbus_macros::interface;

use crate::input::source::hidraw::get_dbus_path;

/// DBusInterface exposing information about a HIDRaw device
pub struct SourceHIDRawInterface {
    info: DeviceInfo,
}

impl SourceHIDRawInterface {
    pub fn new(info: DeviceInfo) -> SourceHIDRawInterface {
        SourceHIDRawInterface { info }
    }

    /// Creates a new instance of the source hidraw interface on DBus. Returns
    /// a structure with information about the source device.
    pub async fn listen_on_dbus(conn: Connection, info: DeviceInfo) -> Result<(), Box<dyn Error>> {
        let path = get_dbus_path(info.path().to_string_lossy().to_string());
        let iface = SourceHIDRawInterface::new(info);
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
    async fn path(&self) -> fdo::Result<String> {
        Ok(self.info.path().to_string_lossy().to_string())
    }

    #[zbus(property)]
    async fn vendor_id(&self) -> fdo::Result<String> {
        Ok(format!("{:04x}", self.info.vendor_id()))
    }

    #[zbus(property)]
    async fn product_id(&self) -> fdo::Result<String> {
        Ok(format!("{:04x}", self.info.product_id()))
    }

    #[zbus(property)]
    async fn serial_number(&self) -> fdo::Result<String> {
        Ok(self.info.serial_number().unwrap_or_default().to_string())
    }

    #[zbus(property)]
    async fn release_number(&self) -> fdo::Result<String> {
        Ok(format!("{:04x}", self.info.release_number()))
    }

    #[zbus(property)]
    async fn manufacturer(&self) -> fdo::Result<String> {
        Ok(self
            .info
            .manufacturer_string()
            .unwrap_or_default()
            .to_string())
    }

    #[zbus(property)]
    async fn product(&self) -> fdo::Result<String> {
        Ok(self.info.product_string().unwrap_or_default().to_string())
    }

    #[zbus(property)]
    async fn interface_number(&self) -> fdo::Result<i32> {
        Ok(self.info.interface_number())
    }
}
