use crate::{
    constants::BUS_SOURCES_PREFIX,
    input::{output_event::OutputEvent, source::command::SourceCommand},
};
use std::{error::Error, time::Duration};
use tokio::sync::mpsc::Sender;
use zbus::{fdo, Connection};
use zbus_macros::interface;

/// DBusInterface exposing LED output
pub struct SourceOutputLedInterface {
    tx: Sender<SourceCommand>,
}

impl SourceOutputLedInterface {
    fn new(tx: Sender<SourceCommand>) -> SourceOutputLedInterface {
        SourceOutputLedInterface { tx }
    }

    /// Creates a new instance of the source led interface on DBus.
    pub fn listen_on_dbus(
        conn: Connection,
        device_id: &str,
        tx: Sender<SourceCommand>,
    ) -> Result<(), Box<dyn Error>> {
        // Create a task to start the dbus interface
        let conn_iface = conn.clone();
        let iface = Self::new(tx.clone());
        let dbus_path = get_dbus_path(device_id);
        tokio::task::spawn(async move {
            log::debug!("Starting dbus interface: {dbus_path}");
            let result = conn_iface
                .object_server()
                .at(dbus_path.clone(), iface)
                .await;
            if let Err(e) = result {
                log::debug!("Failed to start dbus interface {dbus_path}: {e:?}");
            } else {
                log::debug!("Started dbus interface: {dbus_path}");
            }
        });

        // Create a task to remove the interface whenever the receiver no longer
        // exists.
        let dbus_path = get_dbus_path(device_id);
        tokio::task::spawn(async move {
            // Wait for the receiver to close
            tx.closed().await;

            log::debug!("Stopping dbus interface: {dbus_path}");
            let result = conn
                .object_server()
                .remove::<Self, _>(dbus_path.clone())
                .await;
            if let Err(e) = result {
                log::debug!("Failed to remove dbus interface {dbus_path}: {e:?}");
            } else {
                log::debug!("Stopped dbus interface: {dbus_path}");
            }
        });

        Ok(())
    }
}

#[interface(
    name = "org.shadowblip.Output.Source.LED",
    proxy(default_service = "org.shadowblip.InputPlumber")
)]
impl SourceOutputLedInterface {
    /// Set the overall color of the device to the given RGB value.
    async fn set_color(&self, r: u8, g: u8, b: u8) -> fdo::Result<()> {
        let event = OutputEvent::LedColor { r, g, b };
        self.tx
            .send_timeout(SourceCommand::WriteEvent(event), Duration::from_millis(200))
            .await
            .map_err(|e| fdo::Error::Failed(e.to_string()))?;
        Ok(())
    }
}

/// Returns the DBus path for a given device ID
fn get_dbus_path(id: &str) -> String {
    let name = id.replace(':', "_");
    let name = name.split('/').last().unwrap_or_default();
    format!("{}/{}", BUS_SOURCES_PREFIX, name)
}
