use crate::dbus::{interface::Unregisterable, polkit::check_polkit};
use crate::udev::device::UdevDevice;
use zbus::{fdo, message::Header};
use zbus_macros::interface;

/// DBusInterface exposing information about a led device
pub struct SourceLedInterface {
    device: UdevDevice,
}

impl SourceLedInterface {
    pub fn new(device: UdevDevice) -> SourceLedInterface {
        SourceLedInterface { device }
    }
}

#[interface(name = "org.shadowblip.Input.Source.LEDDevice")]
impl SourceLedInterface {
    /// Returns the human readable name of the device (e.g. XBox 360 Pad)
    #[zbus(property)]
    async fn id(&self, #[zbus(header)] hdr: Option<Header<'_>>) -> fdo::Result<String> {
        check_polkit(hdr, "org.shadowblip.Input.Source.LEDDevice.Id").await?;
        Ok(self.device.sysname())
    }
}

impl Unregisterable for SourceLedInterface {}
