use std::{
    collections::{HashMap, HashSet},
    error::Error,
    time::Duration,
};

use tokio::sync::mpsc::{self, Sender};
use zbus::Connection;

use crate::{
    dbus::interface::composite_device::CompositeDeviceInterface,
    input::{
        capability::Capability, event::native::NativeEvent, manager::ManagerCommand,
        target::client::TargetDeviceClient,
    },
};

use super::client::CompositeDeviceClient;

/// Manages target devices for a composite device
#[derive(Debug)]
pub struct CompositeDeviceTargets {
    /// Connection to DBus
    _dbus: Connection,
    /// Composite device this is apart of
    device: CompositeDeviceClient,
    /// Channel for sending manager requests
    manager: Sender<ManagerCommand>,
    /// Path to the composite device on DBus
    path: String,
    /// Map of DBus paths to their respective transmitter channel.
    /// E.g. {"/org/shadowblip/InputPlumber/devices/target/gamepad0": <Sender>}
    target_devices: HashMap<String, TargetDeviceClient>,
    /// Map of device capabilities to a list of target devices that implements
    /// that capability. This list contains the DBus path for the target device
    /// so its transmitter channel can be looked up in `target_devices`.
    /// E.g. {Capability::Keyboard: ["/org/shadowblip/InputPlumber/devices/target/keyboard0"]}
    target_devices_by_capability: HashMap<Capability, HashSet<String>>,
    /// List of target devices waiting to be attached to this composite device.
    /// This is used to block/requeue multiple calls to set_target_devices().
    /// E.g. ["/org/shadowblip/InputPlumber/devices/target/gamepad0"]
    target_devices_queued: HashSet<String>,
    /// List of active target device types (e.g. "deck", "ds5", "xb360") that
    /// were active before system suspend.
    target_devices_suspended: Vec<String>,
    /// Map of DBusDevice DBus paths to their respective transmitter channel.
    /// E.g. {"/org/shadowblip/InputPlumber/devices/target/dbus0": <Sender>}
    target_dbus_devices: HashMap<String, TargetDeviceClient>,
}

impl CompositeDeviceTargets {
    /// Create a new instance of the state of target devices
    pub fn new(
        dbus: Connection,
        path: String,
        device: CompositeDeviceClient,
        manager: Sender<ManagerCommand>,
    ) -> Self {
        Self {
            _dbus: dbus,
            path,
            device,
            manager,
            target_devices: Default::default(),
            target_devices_by_capability: Default::default(),
            target_devices_queued: Default::default(),
            target_devices_suspended: Default::default(),
            target_dbus_devices: Default::default(),
        }
    }

    /// Return a list of DBus paths of attached target devices
    /// E.g. ["/org/shadowblip/InputPlumber/devices/target/gamepad0"]
    pub fn get_device_paths(&self) -> Vec<String> {
        self.target_devices.keys().cloned().collect()
    }

    /// Return a list of DBus paths of any attached target dbus devices.
    /// E.g. ["/org/shadowblip/InputPlumber/devices/target/dbus0"]
    pub fn get_dbus_device_paths(&self) -> Vec<String> {
        self.target_dbus_devices.keys().cloned().collect()
    }

    /// Returns the list of active target device types (e.g. "deck", "ds5", "xb360")
    /// that were active before system suspend.
    pub fn get_suspended_devices(&self) -> Vec<String> {
        self.target_devices_suspended.clone()
    }

    /// Sets the DBus target devices attached to a [CompositeDevice]
    pub fn set_dbus_devices(&mut self, devices: HashMap<String, TargetDeviceClient>) {
        self.target_dbus_devices = devices;
    }

    /// Set the given target devices on the composite device. This will create
    /// new target devices, attach them to this device, and stop/remove any
    /// existing devices.
    pub async fn set_devices(&mut self, device_types: Vec<String>) -> Result<(), Box<dyn Error>> {
        log::info!("Setting target devices: {:?}", device_types);
        // Check to see if there are target device attachments pending. If so,
        // requeue this set_target_devices request.
        if !self.target_devices_queued.is_empty() {
            log::debug!(
                "Target devices already waiting for attachment. Re-queueing set target devices."
            );
            let device = self.device.clone();
            tokio::task::spawn(async move {
                if let Err(e) = device.set_target_devices(device_types).await {
                    log::error!("Error setting target devices! {e:?}");
                }
            });
            return Ok(());
        }

        // Identify which target devices are new
        let mut device_types_to_start: Vec<String> = vec![];
        for kind in device_types.iter() {
            if self.target_kind_running(kind).await? {
                log::debug!("Target device {kind} already running, nothing to do.");
                continue;
            }

            device_types_to_start.push(kind.clone());
        }

        // Identify the targets that need to close
        let mut targets_to_stop: HashMap<String, TargetDeviceClient> = HashMap::new();
        for (path, target) in self.target_devices.clone().into_iter() {
            let target_type = match target.get_type().await {
                Ok(value) => value,
                Err(e) => {
                    return Err(format!("Failed to request target type: {e:?}").into());
                }
            };
            if !device_types.contains(&target_type) {
                log::debug!("Target device {path} not in new devices list. Adding to stop list.");
                targets_to_stop.insert(path, target);
            }
        }

        // Stop all old target devices that aren't going to persist
        for (path, target) in targets_to_stop.clone().into_iter() {
            log::debug!("Stopping old target device: {path}");
            self.target_devices.remove(&path);
            for (_, target_devices) in self.target_devices_by_capability.iter_mut() {
                target_devices.remove(&path);
            }
            if let Err(e) = target.stop().await {
                log::error!("Failed to stop old target device: {e:?}");
            }
        }

        let composite_path = self.path.clone();

        // Create new target devices using the input manager
        for kind in device_types_to_start {
            // Ask the input manager to create a target device
            log::debug!("Requesting to create device: {kind}");
            let (sender, mut receiver) = mpsc::channel(1);
            self.manager
                .send(ManagerCommand::CreateTargetDevice { kind, sender })
                .await?;
            let Some(response) = receiver.recv().await else {
                log::warn!("Channel closed waiting for response from input manager");
                continue;
            };
            let target_path = match response {
                Ok(path) => path,
                Err(e) => {
                    let err = format!("Failed to create target: {e:?}");
                    log::error!("{err}");
                    continue;
                }
            };

            // Ask the input manager to attach the target device to this composite
            // device. Note that this *must* be run in an async task to prevent
            // deadlocking.
            log::debug!("Requesting to attach target device {target_path} to {composite_path}");
            let manager = self.manager.clone();
            let target_path_clone = target_path.clone();
            let composite_path_clone = composite_path.clone();
            tokio::task::spawn(async move {
                let (sender, mut receiver) = mpsc::channel(1);
                let result = manager
                    .send(ManagerCommand::AttachTargetDevice {
                        target_path: target_path_clone,
                        composite_path: composite_path_clone,
                        sender,
                    })
                    .await;
                if let Err(e) = result {
                    log::warn!(
                        "Failed to send attach request to input manager: {}",
                        e.to_string()
                    );
                    return;
                }
                let Some(response) = receiver.recv().await else {
                    log::warn!("Channel closed waiting for response from input manager");
                    return;
                };
                if let Err(e) = response {
                    log::error!("Failed to attach target device: {e:?}");
                }
            });

            // Enqueue the target device to wait for the attachment message from
            // the input manager to prevent multiple calls to set_target_devices()
            // from mangling attachment.
            self.target_devices_queued.insert(target_path);
        }

        // Signal change in target devices to DBus
        // TODO: Check this
        //self.signal_targets_changed().await;

        Ok(())

        //
    }

    // Deterimines if a given target device kind is already running
    async fn target_kind_running(&self, kind: &str) -> Result<bool, Box<dyn Error>> {
        // TODO: Save this on the DS5 target device so we can properly look it up.
        let kind = match kind {
            "ds5" => "ds5_edge",
            _ => kind,
        };
        for target in self.target_devices.values() {
            let target_type = match target.get_type().await {
                Ok(value) => value,
                Err(e) => {
                    return Err(format!("Failed to request target type: {e:?}").into());
                }
            };
            if kind == target_type {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Write the given event to all target devices that are capable of emitting
    /// this event.
    pub async fn write_event(&self, event: NativeEvent) {
        // Find all target devices capable of handling this event
        let cap = event.as_capability();
        let Some(target_paths) = self.target_devices_by_capability.get(&cap) else {
            log::trace!("No target devices capable of handling this event: {cap}");
            return;
        };

        // Filter for capable target devices
        let target_devices: Vec<(&str, &TargetDeviceClient)> = target_paths
            .iter()
            .filter_map(|path| {
                let device = self.target_devices.get(path);
                device.map(|client| (path.as_str(), client))
            })
            .collect();

        // Only write the event to devices that are capable of handling it
        log::trace!("Emit passed event: {event:?}");
        for (name, target) in target_devices {
            if let Err(e) = target.write_event(event.clone()).await {
                log::error!("Failed to write event to: {name}: {e:?}");
            }
        }
    }

    /// Write the given input event to all target dbus devices
    pub async fn write_dbus_event(&self, event: NativeEvent) {
        log::trace!("Emit dbus event: {event:?}");
        for target in self.target_dbus_devices.values() {
            if let Err(e) = target.write_event(event.clone()).await {
                log::error!("Failed to write dbus event: {e}");
            }
        }
    }

    /// Clear the state of all target devices to release any buttons that might
    /// be held, or joysticks that are in a certain direction, etc.
    pub async fn clear_state(&self) {
        for (path, target) in &self.target_devices {
            log::debug!("Clearing target device state: {path}");
            if let Err(e) = target.clear_state().await {
                log::error!("Failed to clear target device state {path}: {e}");
            }
        }
    }

    /// Clear the state of all target devices that are gamepads in order to
    /// release any buttons, re-center any joysticks, etc.
    pub async fn clear_gamepad_state(&self) {
        for (path, target) in &self.target_devices {
            if !path.contains("gamepad") {
                continue;
            }
            log::debug!("Clearing target device state: {path}");
            if let Err(e) = target.clear_state().await {
                log::error!("Failed to clear target device state {path}: {e}");
            }
        }
    }

    /// Spawn task(s) to clear the state of all target devices to release any
    /// buttons that might be held, re-center any joysticks, etc.
    pub fn schedule_clear_state(&self) {
        for (path, target) in self.target_devices.clone() {
            tokio::task::spawn(async move {
                log::debug!("Clearing target device state: {path}");
                if let Err(e) = target.clear_state().await {
                    log::error!("Failed to clear target device state {path}: {e}");
                }
            });
        }
    }

    /// Stop all target devices
    pub async fn stop(&self) {
        for (path, target) in &self.target_devices {
            log::debug!("Stopping target device: {path}");
            if let Err(e) = target.stop().await {
                log::error!("Failed to stop target device {path}: {e}");
            }
        }
        for (path, target) in &self.target_dbus_devices {
            log::debug!("Stopping target dbus device: {path}");
            if let Err(e) = target.stop().await {
                log::error!("Failed to stop dbus device {path}: {e}");
            }
        }
    }

    // Get the capabilities of all target devices
    pub async fn get_capabilities(&self) -> Result<HashSet<Capability>, Box<dyn Error>> {
        let mut target_caps = HashSet::new();
        for target in self.target_devices.values() {
            let caps = match target.get_capabilities().await {
                Ok(caps) => caps,
                Err(e) => {
                    return Err(format!("Failed to get target capabilities: {e:?}").into());
                }
            };
            for cap in caps {
                target_caps.insert(cap);
            }
        }
        for target in self.target_dbus_devices.values() {
            let caps = match target.get_capabilities().await {
                Ok(caps) => caps,
                Err(e) => {
                    return Err(format!("Failed to get target capabilities: {e:?}").into());
                }
            };
            for cap in caps {
                target_caps.insert(cap);
            }
        }

        Ok(target_caps)
    }

    /// Update the target capabilities of the given target device
    pub fn update_capabilities(&mut self, dbus_path: String, capabilities: HashSet<Capability>) {
        // Track the target device by capabilities it has
        for cap in capabilities.into_iter() {
            self.target_devices_by_capability
                .entry(cap)
                .and_modify(|devices| {
                    devices.insert(dbus_path.clone());
                })
                .or_insert_with(|| {
                    let mut devices = HashSet::new();
                    devices.insert(dbus_path.clone());
                    devices
                });
        }
    }

    /// Attach the given target devices to the composite device
    pub async fn attach_devices(
        &mut self,
        targets: HashMap<String, TargetDeviceClient>,
    ) -> Result<(), Box<dyn Error>> {
        let dbus_path = self.path.clone();

        // Keep track of all target devices
        for (path, target) in targets.into_iter() {
            // Query the target device for its capabilities
            let caps = match target.get_capabilities().await {
                Ok(caps) => caps,
                Err(e) => {
                    return Err(format!("Failed to get target capabilities: {e:?}").into());
                }
            };

            log::debug!("Attaching target device: {path}");
            if let Err(e) = target.set_composite_device(self.device.clone()).await {
                return Err(
                    format!("Failed to set composite device for target device: {:?}", e).into(),
                );
            }
            log::debug!("Attached device {path} to {dbus_path}");

            // Track the target device by capabilities it has
            for cap in caps {
                self.target_devices_by_capability
                    .entry(cap)
                    .and_modify(|devices| {
                        devices.insert(path.clone());
                    })
                    .or_insert_with(|| {
                        let mut devices = HashSet::new();
                        devices.insert(path.clone());
                        devices
                    });
            }

            // Add the target device
            self.target_devices_queued.remove(&path);
            self.target_devices.insert(path.clone(), target);
        }

        // TODO: check this
        //self.signal_targets_changed().await;

        Ok(())
    }

    /// Called when notified by the input manager that system suspend is about
    /// to happen.
    pub async fn handle_suspend(&mut self) {
        // Clear the list of suspended target devices
        self.target_devices_suspended.clear();

        // Record what target devices are currently used so they can be restored
        // when the system is resumed.
        for (path, target) in self.target_devices.clone().into_iter() {
            let target_type = match target.get_type().await {
                Ok(kind) => kind,
                Err(err) => {
                    log::error!("Failed to get target device type: {err:?}");
                    continue;
                }
            };

            self.target_devices_suspended.push(target_type);
            self.target_devices.remove(&path);
            for (_, target_devices) in self.target_devices_by_capability.iter_mut() {
                target_devices.remove(&path);
            }
            if let Err(e) = target.stop().await {
                log::error!("Failed to stop old target device: {e:?}");
            }

            // Wait a few beats to ensure that the target device is really gone
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
        log::info!(
            "Target devices before suspend: {:?}",
            self.target_devices_suspended
        );
    }

    /// Called when notified by the input manager that system resume is about
    /// to happen.
    pub async fn handle_resume(&mut self) {
        log::info!(
            "Restoring target devices: {:?}",
            self.target_devices_suspended
        );

        // Set the target devices back to the ones used before suspend
        if let Err(err) = self
            .set_devices(self.target_devices_suspended.clone())
            .await
        {
            log::error!("Failed to set restore target devices: {err:?}");
        }

        // Clear the list of suspended target devices
        self.target_devices_suspended.clear();
    }

    /// Emit a DBus signal when target devices change
    async fn _signal_targets_changed(&self) {
        let dbus_path = self.path.clone();
        let conn = self._dbus.clone();

        tokio::task::spawn(async move {
            // Get the object instance at the given path so we can send DBus signal
            // updates
            let iface_ref = match conn
                .object_server()
                .interface::<_, CompositeDeviceInterface>(dbus_path.clone())
                .await
            {
                Ok(iface) => iface,
                Err(e) => {
                    log::error!(
                        "Failed to get DBus interface for composite device to signal: {e:?}"
                    );
                    return;
                }
            };
            // Emit the target devices changed signal
            let iface = iface_ref.get().await;
            if let Err(e) = iface
                .target_devices_changed(iface_ref.signal_emitter())
                .await
            {
                log::error!("Failed to send target devices changed signal: {e:?}");
            }
        });
    }
}
