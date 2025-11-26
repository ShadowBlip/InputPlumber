use zbus::Connection;
use zbus::{fdo, message::Header};
use zbus_macros::interface;

use crate::dbus::interface::Unregisterable;
use crate::dbus::polkit::check_polkit;

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
    async fn name(
        &self,
        #[zbus(connection)] conn: &Connection,
        #[zbus(header)] hdr: Option<Header<'_>>,
    ) -> fdo::Result<String> {
        check_polkit(conn, hdr, "org.shadowblip.Input.Gamepad.Name").await?;
        Ok(self.dev_name.clone())
    }
}

impl Unregisterable for TargetGamepadInterface {}
