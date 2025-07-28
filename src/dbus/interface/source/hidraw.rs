use zbus::{fdo, message::Header};
use zbus_macros::interface;

use crate::{
    dbus::{interface::Unregisterable, polkit::check_polkit},
    udev::device::UdevDevice,
};

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
    async fn name(&self, #[zbus(header)] hdr: Option<Header<'_>>) -> fdo::Result<String> {
        check_polkit(hdr, "org.shadowblip.Input.Source.HIDRawDevice.Name").await?;
        Ok(self.device.name())
    }

    #[zbus(property)]
    async fn dev_path(&self, #[zbus(header)] hdr: Option<Header<'_>>) -> fdo::Result<String> {
        check_polkit(hdr, "org.shadowblip.Input.Source.HIDRawDevice.DevPath").await?;
        Ok(self.device.devnode())
    }

    #[zbus(property)]
    async fn id_product(&self, #[zbus(header)] hdr: Option<Header<'_>>) -> fdo::Result<String> {
        check_polkit(hdr, "org.shadowblip.Input.Source.HIDRawDevice.IdProduct").await?;
        Ok(format!("{:04x}", self.device.id_product()))
    }

    #[zbus(property)]
    async fn id_vendor(&self, #[zbus(header)] hdr: Option<Header<'_>>) -> fdo::Result<String> {
        check_polkit(hdr, "org.shadowblip.Input.Source.HIDRawDevice.IdVendor").await?;
        Ok(format!("{:04x}", self.device.id_vendor()))
    }

    #[zbus(property)]
    async fn interface_number(&self, #[zbus(header)] hdr: Option<Header<'_>>) -> fdo::Result<i32> {
        check_polkit(
            hdr,
            "org.shadowblip.Input.Source.HIDRawDevice.InterfaceNumber",
        )
        .await?;
        Ok(self.device.interface_number())
    }

    #[zbus(property)]
    async fn manufacturer(&self, #[zbus(header)] hdr: Option<Header<'_>>) -> fdo::Result<String> {
        check_polkit(hdr, "org.shadowblip.Input.Source.HIDRawDevice.Manufacturer").await?;
        Ok(self.device.manufacturer())
    }

    #[zbus(property)]
    async fn product(&self, #[zbus(header)] hdr: Option<Header<'_>>) -> fdo::Result<String> {
        check_polkit(hdr, "org.shadowblip.Input.Source.HIDRawDevice.Product").await?;
        Ok(self.device.product())
    }

    #[zbus(property)]
    async fn serial_number(&self, #[zbus(header)] hdr: Option<Header<'_>>) -> fdo::Result<String> {
        check_polkit(hdr, "org.shadowblip.Input.Source.HIDRawDevice.SerialNumber").await?;
        Ok(self.device.serial_number())
    }

    #[zbus(property)]
    async fn sysfs_path(&self, #[zbus(header)] hdr: Option<Header<'_>>) -> fdo::Result<String> {
        check_polkit(hdr, "org.shadowblip.Input.Source.HIDRawDevice.SysfsPath").await?;
        Ok(self.device.devpath())
    }
}

impl Unregisterable for SourceHIDRawInterface {}
