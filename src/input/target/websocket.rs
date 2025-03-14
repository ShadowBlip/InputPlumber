use std::{collections::HashSet, error::Error, fmt::Debug};

use futures::SinkExt;
use packed_struct::prelude::*;
use tokio::sync::mpsc::{self, error::TryRecvError, Receiver};
use tokio_tungstenite::tungstenite::Message;
use zbus::Connection;

use crate::{
    dbus::interface::target::{websocket::TargetWebsocketInterface, TargetInterface},
    drivers::unified_gamepad::reports::{
        input_capability_report::{InputCapabilityInfo, InputCapabilityReport},
        input_data_report::InputDataReport,
    },
    input::{
        capability::Capability, composite_device::client::CompositeDeviceClient,
        event::native::NativeEvent, output_capability::OutputCapability, output_event::OutputEvent,
    },
};

use super::{
    client::TargetDeviceClient, InputError, OutputError, TargetDeviceTypeId, TargetInputDevice,
    TargetOutputDevice,
};

/// Commands that can be sent from the dbus interface
pub enum DeviceCommand {
    WebsocketConnected,
}

/// A [WebsocketDevice] implements the Unified Controller Input Specification but writes
/// to a websocket server instead of an hidraw device.
pub struct WebsocketDevice {
    conn: Connection,
    dbus_path: Option<String>,
    dbus_rx: Option<Receiver<DeviceCommand>>,
    composite_device: Option<CompositeDeviceClient>,
    capabilities: HashSet<Capability>,
    capabilities_rx: Option<Receiver<HashSet<Capability>>>,
    capability_report: InputCapabilityReport,
    state: InputDataReport,
}

impl WebsocketDevice {
    /// Create a new [WebsocketDevice]
    pub fn new(conn: Connection) -> Self {
        Self {
            conn,
            dbus_path: None,
            dbus_rx: None,
            composite_device: None,
            capabilities: HashSet::new(),
            capabilities_rx: None,
            capability_report: InputCapabilityReport::default(),
            state: InputDataReport::default(),
        }
    }

    /// Checks to see if new capabilities are available in the capabilities channel
    fn receive_new_capabilities(&mut self) -> Option<HashSet<Capability>> {
        let rx = self.capabilities_rx.as_mut()?;

        match rx.try_recv() {
            Ok(capabilities) => Some(capabilities),
            Err(e) => match e {
                TryRecvError::Empty => None,
                TryRecvError::Disconnected => {
                    self.capabilities_rx = None;
                    None
                }
            },
        }
    }

    /// Send the capability report to the connected websocket server
    fn send_capability_report(&self) {
        // Signal that capabilities have changed
        let capability_report = self.capability_report.clone();
        let Some(dbus_path) = self.dbus_path.clone() else {
            log::warn!("No dbus interface exists with websocket stream");
            return;
        };
        let conn = self.conn.clone();
        tokio::task::spawn(async move {
            log::debug!("Sending capability report");
            // Pack the capability report to bytes
            let capability_report = match capability_report.pack_to_vec() {
                Ok(report) => report,
                Err(e) => {
                    log::error!("Failed to pack capability report: {e}");
                    return;
                }
            };

            // Get the object instance at the given path so we can send DBus signal
            // updates
            let iface_ref = match conn
                .object_server()
                .interface::<_, TargetWebsocketInterface>(dbus_path.clone())
                .await
            {
                Ok(iface) => iface,
                Err(e) => {
                    log::error!(
                        "Failed to get DBus interface for websocket device to signal: {e:?}"
                    );
                    return;
                }
            };
            let mut iface = iface_ref.get_mut().await;
            let Some(ws_conn) = iface.connection.as_mut() else {
                log::debug!("No websocket connection exists to send capability report");
                return;
            };
            let data = Message::binary(capability_report);
            if let Err(e) = ws_conn.send(data).await {
                log::error!("Failed to send capability report: {e}");
            }
        });
    }

    /// Update the device capabilities with the given capabilities
    fn update_capabilities(&mut self, capabilities: HashSet<Capability>) {
        log::debug!("Updating device capabilities with: {capabilities:?}");
        let Some(composite_device) = self.composite_device.as_ref() else {
            log::warn!("No composite device set to update capabilities");
            return;
        };

        // Set the capabilities of the device
        self.capabilities = capabilities.clone();

        // Update the capability report with the source capabilities
        let mut cap_info: Vec<InputCapabilityInfo> = capabilities
            .clone()
            .into_iter()
            .map(|cap| cap.into())
            .collect();
        cap_info.sort_by_key(|cap| cap.value_type.order_priority());

        // Update the capability report
        self.capability_report = InputCapabilityReport::default();
        for info in cap_info {
            log::trace!("Updating report with info: {info}");
            if let Err(e) = self.capability_report.add_capability(info) {
                log::warn!("Failed to add input capability for gamepad: {e}");
            }
        }
        log::debug!("Using capability report: {}", self.capability_report);

        // Inform the composite device that the capabilities have changed
        if let Some(dbus_path) = self.dbus_path.as_ref() {
            log::debug!("Updating composite device with new capabilities");
            if let Err(e) = composite_device
                .blocking_update_target_capabilities(dbus_path.clone(), capabilities)
            {
                log::warn!("Failed to update target capabilities: {e:?}");
            }
        }

        // Signal that capabilities have changed
        self.send_capability_report();

        log::debug!("Updated capabilities");
    }

    /// Write the current device state to the virtual device
    fn write_state(&mut self) -> Result<(), Box<dyn Error>> {
        let Some(dbus_path) = self.dbus_path.clone() else {
            return Ok(());
        };
        let conn = self.conn.clone();

        // Write the state to the dbus interface
        let data = self.state.pack()?;
        tokio::task::spawn(async move {
            let data = Message::binary(data.to_vec());
            // Get the object instance at the given path so we can send DBus signal
            // updates
            let iface_ref = match conn
                .object_server()
                .interface::<_, TargetWebsocketInterface>(dbus_path.clone())
                .await
            {
                Ok(iface) => iface,
                Err(e) => {
                    log::error!("Failed to get DBus interface for debug device to signal: {e:?}");
                    return;
                }
            };
            let mut iface = iface_ref.get_mut().await;
            let Some(ws_conn) = iface.connection.as_mut() else {
                return;
            };
            if let Err(e) = ws_conn.send(data).await {
                log::error!("Failed to send state update: {e}");
            }
        });

        Ok(())
    }
}

impl TargetInputDevice for WebsocketDevice {
    /// Start the DBus interface for this target device
    fn start_dbus_interface(
        &mut self,
        dbus: Connection,
        path: String,
        client: TargetDeviceClient,
        type_id: TargetDeviceTypeId,
    ) {
        log::debug!("Starting dbus interface: {path}");
        log::trace!("Using device client: {client:?}");
        let (tx, rx) = mpsc::channel(128);
        self.dbus_path = Some(path.clone());
        self.dbus_rx = Some(rx);
        tokio::task::spawn(async move {
            let generic_interface = TargetInterface::new(&type_id);
            let iface = TargetWebsocketInterface::new(tx);

            let object_server = dbus.object_server();
            let (gen_result, result) = tokio::join!(
                object_server.at(path.clone(), generic_interface),
                object_server.at(path.clone(), iface)
            );

            if gen_result.is_err() || result.is_err() {
                log::debug!("Failed to start dbus interface: {path} generic: {gen_result:?} type-specific: {result:?}");
            } else {
                log::debug!("Started dbus interface: {path}");
            }
        });
    }

    fn stop_dbus_interface(&mut self, dbus: Connection, path: String) {
        log::debug!("Stopping dbus interface for {path}");
        tokio::task::spawn(async move {
            let object_server = dbus.object_server();
            let (target, generic) = tokio::join!(
                object_server.remove::<TargetWebsocketInterface, String>(path.clone()),
                object_server.remove::<TargetInterface, String>(path.clone())
            );
            if generic.is_err() || target.is_err() {
                if let Err(err) = target {
                    log::debug!("Failed to stop debug interface {path}: {err}");
                }
                if let Err(err) = generic {
                    log::debug!("Failed to stop target interface {path}: {err}");
                }
            } else {
                log::debug!("Stopped dbus interface for {path}");
            }
        });
    }

    fn on_composite_device_attached(
        &mut self,
        composite_device: CompositeDeviceClient,
    ) -> Result<(), InputError> {
        self.composite_device = Some(composite_device.clone());

        // Spawn a task to asyncronously fetch the source capabilities of
        // the composite device.
        let (tx, rx) = mpsc::channel(1);
        tokio::task::spawn(async move {
            log::debug!("Getting capabilities from the composite device!");
            let capabilities = match composite_device.get_capabilities().await {
                Ok(caps) => caps,
                Err(e) => {
                    log::warn!("Failed to fetch composite device capabilities: {e:?}");
                    return;
                }
            };
            if let Err(e) = tx.send(capabilities).await {
                log::warn!("Failed to send composite device capabilities: {e:?}");
            }
        });

        // Keep a reference to the receiver so it can be checked every poll iteration
        self.capabilities_rx = Some(rx);

        Ok(())
    }

    fn write_event(&mut self, event: NativeEvent) -> Result<(), InputError> {
        log::trace!("Received event: {event:?}");

        // Update the internal controller state when events are emitted.
        if let Err(e) = self.state.update(&self.capability_report, event.into()) {
            log::warn!("Failed to update gamepad state: {e}");
            log::warn!("Current capability report: {}", self.capability_report);
        }

        // Write the current state
        self.write_state()?;

        Ok(())
    }

    fn get_capabilities(&self) -> Result<Vec<Capability>, InputError> {
        // Get the input capabilities from the source device(s)
        let capabilities = Vec::from_iter(self.capabilities.iter().cloned());
        Ok(capabilities)
    }
}

impl TargetOutputDevice for WebsocketDevice {
    fn poll(
        &mut self,
        _composite_device: &Option<CompositeDeviceClient>,
    ) -> Result<Vec<OutputEvent>, OutputError> {
        // Check to see if there are any capability updates
        if let Some(new_capabilities) = self.receive_new_capabilities() {
            self.update_capabilities(new_capabilities);
        }

        // Check for dbus events
        if let Some(rx) = self.dbus_rx.as_mut() {
            if !rx.is_empty() {
                let Some(cmd) = rx.blocking_recv() else {
                    return Err(OutputError::DeviceError("Dbus channel was closed".into()));
                };
                match cmd {
                    DeviceCommand::WebsocketConnected => {
                        log::debug!("Connected to websocket server");
                        self.send_capability_report();
                    }
                }
            }
        }

        Ok(vec![])
    }

    fn get_output_capabilities(&self) -> Result<Vec<OutputCapability>, OutputError> {
        // TODO: Get the output capabilities from the source device(s)
        Ok(vec![])
    }
}

impl Debug for WebsocketDevice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DebugDevice")
            .field("state", &self.state)
            .finish()
    }
}
