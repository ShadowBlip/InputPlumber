use crate::input::source::iio::get_dbus_path;
use crate::udev::device::UdevDevice;
use std::error::Error;
use zbus::{fdo, Connection};
use zbus_macros::interface;

/// DBusInterface exposing information about a led device
pub struct SourceLedInterface {
    device: UdevDevice,
}

impl SourceLedInterface {
    pub fn new(device: UdevDevice) -> SourceLedInterface {
        SourceLedInterface { device }
    }
    /// Creates a new instance of the source led interface on DBus. Returns
    /// a structure with information about the source device.
    pub async fn listen_on_dbus(
        conn: Connection,
        device: UdevDevice,
    ) -> Result<(), Box<dyn Error>> {
        let iface = SourceLedInterface::new(device);
        let Ok(id) = iface.id() else {
            return Ok(());
        };
        let path = get_dbus_path(id);
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

#[interface(name = "org.shadowblip.Input.Source.LEDDevice")]
impl SourceLedInterface {
    /// Returns the human readable name of the device (e.g. XBox 360 Pad)
    #[zbus(property)]
    fn id(&self) -> fdo::Result<String> {
        Ok(self.device.sysname())
    }
}
