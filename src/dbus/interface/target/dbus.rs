use zbus::{fdo, object_server::SignalContext};
use zbus_macros::interface;

/// The [TargetDBusInterface] provides a DBus interface that can be exposed for managing
/// a [DBusDevice]. It works by sending command messages to a channel that the
/// [DBusDevice] is listening on.
pub struct TargetDBusInterface {}

impl TargetDBusInterface {
    pub fn new() -> TargetDBusInterface {
        TargetDBusInterface {}
    }
}

impl Default for TargetDBusInterface {
    fn default() -> Self {
        Self::new()
    }
}

#[interface(name = "org.shadowblip.Input.DBusDevice")]
impl TargetDBusInterface {
    /// Name of the DBus device
    #[zbus(property)]
    async fn name(&self) -> fdo::Result<String> {
        Ok("DBusDevice".into())
    }

    /// Emitted when an input event occurs
    #[zbus(signal)]
    pub async fn input_event(
        ctxt: &SignalContext<'_>,
        event: String,
        value: f64,
    ) -> zbus::Result<()>;

    /// Emitted when a touch event occurs.
    #[zbus(signal)]
    pub async fn touch_event(
        ctxt: &SignalContext<'_>,
        event: String,
        index: u32,
        is_touching: bool,
        pressure: f64,
        x: f64,
        y: f64,
    ) -> zbus::Result<()>;
}
