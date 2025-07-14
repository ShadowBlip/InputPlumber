use crate::dbus::interface::Unregisterable;
use crate::udev::device::UdevDevice;
use zbus::fdo;
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
    fn id(&self) -> fdo::Result<String> {
        Ok(self.device.sysname())
    }
}

impl Unregisterable for SourceLedInterface {}
