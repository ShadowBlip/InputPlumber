use crate::dbus::polkit::check_polkit;
use zbus::{fdo, message::Header, Connection};
use zbus_macros::interface;

/// The [TargetTouchscreenInterface] provides a DBus interface that can be exposed for managing
/// a [TouchscreenDevice]. It works by sending command messages to a channel that the
/// [TouchscreenDevice] is listening on.
#[allow(dead_code)]
pub struct TargetTouchscreenInterface {}

impl TargetTouchscreenInterface {
    #[allow(dead_code)]
    pub fn new() -> TargetTouchscreenInterface {
        TargetTouchscreenInterface {}
    }
}

impl Default for TargetTouchscreenInterface {
    fn default() -> Self {
        Self::new()
    }
}

#[interface(name = "org.shadowblip.Input.Touchscreen")]
impl TargetTouchscreenInterface {
    /// Name of the target device
    #[zbus(property)]
    #[allow(dead_code)]
    async fn name(
        &self,
        #[zbus(connection)] conn: &Connection,
        #[zbus(header)] hdr: Option<Header<'_>>,
    ) -> fdo::Result<String> {
        check_polkit(conn, hdr, "org.shadowblip.Input.Touchscreen.Name").await?;
        Ok("Touchscreen".into())
    }
}
