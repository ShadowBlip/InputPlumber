use zbus::{fdo, object_server::SignalEmitter};
use zbus_macros::interface;

use crate::drivers::unified_gamepad::reports::input_capability_report::InputCapabilityReport;

/// The [TargetDBusInterface] provides a DBus interface that can be exposed for managing
/// a [DBusDevice]. It works by sending command messages to a channel that the
/// [DBusDevice] is listening on.
pub struct TargetDebugInterface {
    pub capability_report: InputCapabilityReport,
}

impl TargetDebugInterface {
    pub fn new() -> TargetDebugInterface {
        TargetDebugInterface {
            capability_report: InputCapabilityReport::default(),
        }
    }
}

impl Default for TargetDebugInterface {
    fn default() -> Self {
        Self::new()
    }
}

#[interface(
    name = "org.shadowblip.Input.Debug",
    proxy(default_service = "org.shadowblip.InputPlumber",)
)]
impl TargetDebugInterface {
    /// Returns the input capability report data that can be used to decode
    /// the values from the input report.
    #[zbus(property)]
    pub async fn input_capability_report(&self) -> fdo::Result<Vec<u8>> {
        let data = self
            .capability_report
            .pack_to_vec()
            .map_err(|e| fdo::Error::Failed(e.to_string()))?;
        Ok(data)
    }

    /// Emitted when a input is emitted.
    #[zbus(signal)]
    pub async fn input_report(emitter: &SignalEmitter<'_>, data: Vec<u8>) -> zbus::Result<()>;
}
