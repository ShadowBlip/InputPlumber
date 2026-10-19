use crate::dbus::{interface::Unregisterable, polkit::check_polkit};
use crate::input::source::led::managed::{Capabilities, LedConfig, LedHandle, LedState};
use crate::udev::device::UdevDevice;
use zbus::{fdo, message::Header, Connection};
use zbus_macros::interface;

pub struct SourceLedInterface {
    device: UdevDevice,
    handle: LedHandle,
}
impl SourceLedInterface {
    pub fn new(device: UdevDevice, handle: LedHandle) -> Self {
        Self { device, handle }
    }
    pub fn watch(&self, connection: Connection, path: String) {
        let mut updates = self.handle.subscribe();
        tokio::spawn(async move {
            while updates.changed().await.is_ok() {
                // A late subscriber always obtains the latest snapshot with GetAll.
                let Ok(interface) = connection
                    .object_server()
                    .interface::<_, Self>(path.as_str())
                    .await
                else {
                    continue;
                };
                let instance = interface.get().await;
                if let Err(e) = instance.state_changed(interface.signal_emitter()).await {
                    log::debug!("LED state notification: {e}");
                }
                if let Err(e) = instance
                    .capabilities_changed(interface.signal_emitter())
                    .await
                {
                    log::debug!("LED capabilities notification: {e}");
                }
            }
        });
    }
}
#[interface(name = "org.shadowblip.Input.Source.LEDDevice")]
impl SourceLedInterface {
    #[zbus(property)]
    async fn id(
        &self,
        #[zbus(connection)] conn: &Connection,
        #[zbus(header)] hdr: Option<Header<'_>>,
    ) -> fdo::Result<String> {
        check_polkit(conn, hdr, "org.shadowblip.Input.Source.LEDDevice.Id").await?;
        Ok(self.device.sysname())
    }
    #[zbus(property)]
    fn capabilities(&self) -> Capabilities {
        self.handle.snapshot().capabilities
    }
    /// One atomic snapshot: saved configuration, revision and application result.
    #[zbus(property)]
    fn state(&self) -> LedState {
        self.handle.snapshot().state
    }
    /// Accept one complete configuration. The reply confirms persistence, not visible output.
    async fn set_config(
        &self,
        config: LedConfig,
        #[zbus(connection)] conn: &Connection,
        #[zbus(header)] hdr: Header<'_>,
    ) -> fdo::Result<u64> {
        if hdr.sender().is_none() {
            return Err(fdo::Error::AccessDenied("Missing caller identity".into()));
        }
        check_polkit(
            conn,
            Some(hdr),
            "org.shadowblip.Input.Source.LEDDevice.SetConfig",
        )
        .await?;
        config
            .validate(&self.handle.snapshot().capabilities.effects)
            .map_err(fdo::Error::InvalidArgs)?;
        self.handle.apply(config).await.map_err(fdo::Error::Failed)
    }
}
impl Unregisterable for SourceLedInterface {}
