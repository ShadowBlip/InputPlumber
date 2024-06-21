use zbus::fdo;
use zbus_macros::interface;

/// The [TargetTouchscreenInterface] provides a DBus interface that can be exposed for managing
/// a [TouchscreenDevice]. It works by sending command messages to a channel that the
/// [TouchscreenDevice] is listening on.
pub struct TargetTouchscreenInterface {}

impl TargetTouchscreenInterface {
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
    async fn name(&self) -> fdo::Result<String> {
        Ok("Touchscreen".into())
    }
}
