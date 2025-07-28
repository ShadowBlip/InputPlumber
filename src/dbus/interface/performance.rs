use std::error::Error;

use zbus::{fdo, message::Header, object_server::SignalEmitter, Connection};
use zbus_macros::interface;

use crate::{
    dbus::polkit::check_polkit,
    input::{
        capability::Capability,
        event::context::{EventContext, SerializedSpan},
    },
};

use super::Unregisterable;

/// The [PerformanceInterface] provides a simple dbus interface for collecting
/// latency and performance metrics from a running device.
pub struct PerformanceInterface {
    enabled: bool,
}

impl PerformanceInterface {
    pub fn new() -> PerformanceInterface {
        PerformanceInterface { enabled: false }
    }
}

#[interface(
    name = "org.shadowblip.Input.Metrics",
    proxy(
        default_service = "org.shadowblip.InputPlumber",
        default_path = "/org/shadowblip/InputPlumber/Manager"
    )
)]
impl PerformanceInterface {
    #[zbus(property)]
    async fn enabled(&self, #[zbus(header)] hdr: Option<Header<'_>>) -> fdo::Result<bool> {
        check_polkit(hdr, "org.shadowblip.Input.Metrics.Enabled").await?;
        Ok(self.enabled)
    }

    #[zbus(property)]
    async fn set_enabled(
        &mut self,
        value: bool,
        #[zbus(header)] hdr: Option<Header<'_>>,
    ) -> fdo::Result<()> {
        check_polkit(hdr, "org.shadowblip.Input.Metrics.SetEnabled").await?;
        self.enabled = value;
        Ok(())
    }

    #[zbus(signal)]
    pub async fn event_metrics(
        emitter: &SignalEmitter<'_>,
        capability: String,
        spans: Vec<SerializedSpan>,
    ) -> zbus::Result<()>;
}

impl PerformanceInterface {
    pub async fn emit_metrics(
        conn: &Connection,
        path: &str,
        capability: Capability,
        context: &EventContext,
    ) -> Result<(), Box<dyn Error>> {
        // Get the object instance at the given path so we can send DBus signal
        // updates
        let iface_ref = conn
            .object_server()
            .interface::<_, PerformanceInterface>(path)
            .await?;
        let enabled = iface_ref.get().await.enabled(None).await?;
        if !enabled {
            return Ok(());
        }

        // Serialize and emit the spans for this event
        let cap = format!("{capability:?}");
        let counter = context.metrics();
        let spans: Vec<SerializedSpan> = counter.iter().map(|span| span.into()).collect();
        iface_ref.event_metrics(cap, spans).await?;

        Ok(())
    }
}

impl Default for PerformanceInterface {
    fn default() -> Self {
        Self::new()
    }
}

impl Unregisterable for PerformanceInterface {}
