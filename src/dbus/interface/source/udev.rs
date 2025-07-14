use std::collections::HashMap;

use zbus::fdo;
use zbus_macros::interface;

use crate::{dbus::interface::Unregisterable, udev::device::UdevDevice};

/// The [SourceUdevDeviceInterface] provides a DBus interface to expose udev
/// information over dbus
pub struct SourceUdevDeviceInterface {
    device: UdevDevice,
}

impl SourceUdevDeviceInterface {
    pub fn new(device: UdevDevice) -> SourceUdevDeviceInterface {
        SourceUdevDeviceInterface { device }
    }
}

#[interface(
    name = "org.shadowblip.Input.Source.UdevDevice",
    proxy(default_service = "org.shadowblip.InputPlumber",)
)]
impl SourceUdevDeviceInterface {
    /// Returns the full device node path to the device (e.g. /dev/input/event3)
    #[zbus(property)]
    pub fn device_path(&self) -> fdo::Result<String> {
        Ok(self.device.devnode())
    }

    /// Returns the bus type of the device
    #[zbus(property)]
    async fn id_bustype(&self) -> fdo::Result<String> {
        Ok(format!("{}", self.device.id_bustype()))
    }

    /// Returns the product id of the device
    #[zbus(property)]
    async fn id_product(&self) -> fdo::Result<String> {
        Ok(format!("{:04x}", self.device.id_product()))
    }

    /// Returns the vendor id of the device
    #[zbus(property)]
    async fn id_vendor(&self) -> fdo::Result<String> {
        Ok(format!("{:04x}", self.device.id_vendor()))
    }

    /// Returns the version id of the device
    #[zbus(property)]
    async fn id_version(&self) -> fdo::Result<String> {
        Ok(format!("{}", self.device.id_version()))
    }

    /// Returns the human readable name of the device (e.g. XBox 360 Pad)
    #[zbus(property)]
    async fn name(&self) -> fdo::Result<String> {
        Ok(self.device.name())
    }

    /// Returns the phys_path of the device (e.g usb-0000:07:00.3-2/input0)
    #[zbus(property)]
    async fn phys_path(&self) -> fdo::Result<String> {
        Ok(self.device.phys())
    }

    /// Returns the subsystem that the device belongs to. E.g. "input", "hidraw"
    #[zbus(property)]
    pub fn subsystem(&self) -> fdo::Result<String> {
        Ok(self.device.subsystem())
    }

    /// Returns the full sysfs path of the device (e.g. /sys/devices/pci0000:00)
    #[zbus(property)]
    async fn sysfs_path(&self) -> fdo::Result<String> {
        Ok(self.device.devpath())
    }

    /// Returns the uniq of the device
    #[zbus(property)]
    async fn unique_id(&self) -> fdo::Result<String> {
        Ok(self.device.uniq())
    }

    /// Returns the udev device properties of the device
    #[zbus(property)]
    async fn properties(&self) -> fdo::Result<HashMap<String, String>> {
        Ok(self.device.get_properties())
    }
}

impl Unregisterable for SourceUdevDeviceInterface {}
