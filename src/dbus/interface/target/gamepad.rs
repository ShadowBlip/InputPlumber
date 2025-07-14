use zbus::fdo;
use zbus_macros::interface;

use crate::dbus::interface::Unregisterable;

/// The [TargetGamepadInterface] provides a DBus interface that can be exposed for managing
/// a [GenericGamepad].
pub struct TargetGamepadInterface {
    dev_name: String,
}

impl TargetGamepadInterface {
    pub fn new(dev_name: String) -> TargetGamepadInterface {
        TargetGamepadInterface { dev_name }
    }
}

impl Default for TargetGamepadInterface {
    fn default() -> Self {
        Self::new("Gamepad".to_string())
    }
}

#[interface(name = "org.shadowblip.Input.Gamepad")]
impl TargetGamepadInterface {
    /// Name of the DBus device
    #[zbus(property)]
    async fn name(&self) -> fdo::Result<String> {
        Ok(self.dev_name.clone())
    }
}

impl Unregisterable for TargetGamepadInterface {}
