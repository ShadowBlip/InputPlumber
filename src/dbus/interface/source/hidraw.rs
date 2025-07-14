use zbus::fdo;
use zbus_macros::interface;

use crate::{dbus::interface::Unregisterable, udev::device::UdevDevice};

/// DBusInterface exposing information about a HIDRaw device
pub struct SourceHIDRawInterface {
    device: UdevDevice,
}

impl SourceHIDRawInterface {
    pub fn new(device: UdevDevice) -> SourceHIDRawInterface {
        SourceHIDRawInterface { device }
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

impl Unregisterable for SourceHIDRawInterface {}
