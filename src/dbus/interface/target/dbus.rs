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
    #[deprecated(since = "0.24.1", note = "use `event` instead")]
    #[zbus(signal)]
    pub async fn input_event(
        ctxt: &SignalContext<'_>,
        event: String,
        value: f64,
    ) -> zbus::Result<()>;

    /// Emitted when an input event occurs
    #[zbus(signal)]
    pub async fn event(
        ctxt: &SignalContext<'_>,
        event_name: String,
        kind: String, // informs client of what type of value to expect
        value: Vec<f64>,
    ) -> zbus::Result<()>;
}
