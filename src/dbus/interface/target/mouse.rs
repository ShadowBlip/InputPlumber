use zbus::fdo;
use zbus_macros::interface;

/// The [TargetMouseInterface] provides a DBus interface that can be exposed for managing
/// a [MouseDevice]. It works by sending command messages to a channel that the
/// [MouseDevice] is listening on.
pub struct TargetMouseInterface {}

impl TargetMouseInterface {
    pub fn new() -> TargetMouseInterface {
        TargetMouseInterface {}
    }
}

impl Default for TargetMouseInterface {
    fn default() -> Self {
        Self::new()
    }
}

#[interface(name = "org.shadowblip.Input.Mouse")]
impl TargetMouseInterface {
    /// Name of the composite device
    #[zbus(property)]
    async fn name(&self) -> fdo::Result<String> {
        Ok("Mouse".into())
    }
}
