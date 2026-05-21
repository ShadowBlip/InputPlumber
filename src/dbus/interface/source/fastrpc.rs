use crate::{
    dbus::{interface::Unregisterable, polkit::check_polkit},
    udev::device::UdevDevice,
};
use zbus::{fdo, message::Header, Connection};
use zbus_macros::interface;

/// DBusInterface exposing information about a FastRPC device
pub struct SourceFastRpcInterface {
    device: UdevDevice,
}

impl SourceFastRpcInterface {
    pub fn new(device: UdevDevice) -> SourceFastRpcInterface {
        SourceFastRpcInterface { device }
    }
}

#[interface(name = "org.shadowblip.Input.Source.FastRPCDevice")]
impl SourceFastRpcInterface {
    /// Returns the human readable name of the device (e.g. XBox 360 Pad)
    #[zbus(property)]
    async fn id(
        &self,
        #[zbus(connection)] conn: &Connection,
        #[zbus(header)] hdr: Option<Header<'_>>,
    ) -> fdo::Result<String> {
        check_polkit(conn, hdr, "org.shadowblip.Input.Source.FastRPCDevice.Id").await?;
        Ok(self.device.sysname())
    }
}

impl Unregisterable for SourceFastRpcInterface {}
