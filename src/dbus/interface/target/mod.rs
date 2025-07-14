pub mod dbus;
pub mod debug;
pub mod gamepad;
pub mod keyboard;
pub mod mouse;
pub mod touchscreen;

use zbus::fdo;
use zbus_macros::interface;

use crate::input::target::TargetDeviceTypeId;

use super::Unregisterable;

/// The [TargetInterface] provides a DBus interface that can be exposed for managing
/// a target input device.
pub struct TargetInterface {
    dev_name: String,
    device_type: String,
}

impl TargetInterface {
    pub fn new(device_type: &TargetDeviceTypeId) -> TargetInterface {
        TargetInterface {
            dev_name: device_type.name().to_owned(),
            device_type: device_type.as_str().to_owned(),
        }
    }
}

#[interface(
    name = "org.shadowblip.Input.Target",
    proxy(default_service = "org.shadowblip.InputPlumber",)
)]
impl TargetInterface {
    /// Name of the DBus device
    #[zbus(property)]
    async fn name(&self) -> fdo::Result<String> {
        Ok(self.dev_name.clone())
    }

    #[zbus(property)]
    async fn device_type(&self) -> fdo::Result<String> {
        Ok(self.device_type.clone())
    }
}

impl Unregisterable for TargetInterface {}
