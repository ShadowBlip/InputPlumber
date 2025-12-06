use crate::dbus::{interface::Unregisterable, polkit::check_polkit};
use crate::udev::device::UdevDevice;
use zbus::Connection;
use zbus::{fdo, message::Header};
use zbus_macros::interface;

/// DBusInterface exposing information about a TTY device
pub struct SourceTtyInterface {
    device: UdevDevice,
}

impl SourceTtyInterface {
    pub fn new(device: UdevDevice) -> SourceTtyInterface {
        SourceTtyInterface { device }
    }
}

#[interface(name = "org.shadowblip.Input.Source.TTYDevice")]
impl SourceTtyInterface {
    /// Returns the human readable name of the device (e.g. XBox 360 Pad)
    #[zbus(property)]
    async fn id(
        &self,
        #[zbus(connection)] conn: &Connection,
        #[zbus(header)] hdr: Option<Header<'_>>,
    ) -> fdo::Result<String> {
        check_polkit(conn, hdr, "org.shadowblip.Input.Source.TTYDevice.Id").await?;
        Ok(self.device.sysname())
    }
}

impl Unregisterable for SourceTtyInterface {}
