use std::{
    borrow::Borrow,
    collections::{BTreeSet, HashMap, HashSet},
    error::Error,
};

use evdev::InputEvent;
use tokio::task::JoinSet;
use zbus::Connection;

use crate::{
    config::{capability_map::CapabilityMapConfig, CompositeDeviceConfig},
    dbus::interface::{
        composite_device::CompositeDeviceInterface, source::iio_imu::SourceIioImuInterface,
    },
    input::{
        capability::Capability,
        output_event::{OutputEvent, UinputOutputEvent},
        source::{
            client::SourceDeviceClient, evdev::EventDevice, hidraw::HidRawDevice, iio::IioDevice,
            led::LedDevice, SourceDevice,
        },
    },
    udev::{device::UdevDevice, hide_device, unhide_device},
};

use super::client::CompositeDeviceClient;

/// Manages source devices for a composite device
#[derive(Debug)]
pub struct CompositeDeviceSources {
    /// Capabilities describe all input capabilities from all source devices
    capabilities: HashSet<Capability>,
    /// Connection to DBus
    dbus: Connection,
    /// Composite device this is apart of
    device: CompositeDeviceClient,
    /// Set of available Force Feedback effect IDs that are not in use
    /// TODO: Just use the keys from ff_effect_id_source_map to determine next id
    ff_effect_ids: BTreeSet<i16>,
    /// Source devices use their own IDs for uploaded force feedback effects.
    /// This mapping maps the composite device effect ids to source device effect ids.
    /// E.g. {3: {"evdev://event0": 6, "evdev://event1": 2}}
    ff_effect_id_source_map: HashMap<i16, HashMap<String, i16>>,
    /// Path to the composite device on DBus
    path: String,
    /// Map of source device id to their respective transmitter channel.
    /// E.g. {"evdev://event0": <Sender>}
    source_devices: HashMap<String, SourceDeviceClient>,
    /// Source devices that this composite device will consume.
    source_devices_discovered: Vec<SourceDevice>,
    /// Source devices that should be hidden before they are started. This
    /// is a list of devnode paths to hide (e.g. ["/dev/input/event10", "/dev/hidraw1"])
    source_devices_to_hide: Vec<String>,
    /// HashSet of source devices that are blocked from passing their input events to target
    /// events.
    source_devices_blocked: HashSet<String>,
    /// Physical device path for source devices. E.g. ["/dev/input/event0"]
    source_device_paths: Vec<String>,
    /// All currently running source device threads
    source_device_tasks: JoinSet<()>,
    /// Unique identifiers for running source devices. E.g. ["evdev://event0"]
    source_devices_used: Vec<String>,
}

impl CompositeDeviceSources {
    /// Create a new instance of the state of source devices
    pub fn new(
        dbus: Connection,
        path: String,
        device: CompositeDeviceClient,
        capability_map: Option<&CapabilityMapConfig>,
    ) -> Self {
        // If a capability map is defined, add those target capabilities to
        // the hashset of implemented capabilities.
        let mut capabilities = HashSet::new();
        if let Some(map) = capability_map {
            match map {
                CapabilityMapConfig::V1(config) => {
                    for mapping in config.mapping.iter() {
                        let cap = mapping.target_event.clone().into();
                        if cap == Capability::NotImplemented {
                            continue;
                        }
                        capabilities.insert(cap);
                    }
                }
                CapabilityMapConfig::V2(config) => {
                    for mapping in config.mapping.iter() {
                        let cap = mapping.target_event.clone().into();
                        if cap == Capability::NotImplemented {
                            continue;
                        }
                        capabilities.insert(cap);
                    }
                }
            }
        }

        Self {
            capabilities,
            dbus,
            device,
            ff_effect_ids: (0..64).collect(),
            ff_effect_id_source_map: Default::default(),
            path,
            source_devices: Default::default(),
            source_devices_discovered: Default::default(),
            source_devices_to_hide: Default::default(),
            source_devices_blocked: Default::default(),
            source_device_paths: Default::default(),
            source_device_tasks: Default::default(),
            source_devices_used: Default::default(),
        }
    }

    /// Returns true if the given device id is a blocked device
    pub fn is_blocked_device(&self, device_id: &str) -> bool {
        self.source_devices_blocked.contains(device_id)
    }

    /// Returns true if any source devices are attached
    pub fn has_source_devices(&self) -> bool {
        !self.source_devices_used.is_empty()
    }

    /// Get the capabilities of all source devices
    pub fn get_capabilities(&self) -> HashSet<Capability> {
        self.capabilities.clone()
    }

    /// Hide all attached source devices
    pub async fn hide_devices(&mut self) {
        // Hide the device if specified
        for source_path in self.source_devices_to_hide.drain(..) {
            log::debug!("Hiding device: {}", source_path);
            if let Err(e) = hide_device(source_path.as_str()).await {
                log::warn!("Failed to hide device '{source_path}': {e:?}");
            }
            log::debug!("Finished hiding device: {source_path}");
        }
    }

    /// Unhide all attached source devices
    pub async fn unhide_devices(&self) {
        for source_path in self.source_device_paths.clone() {
            if source_path.starts_with("/sys/bus/iio/devices") {
                log::debug!("Skipping unhiding IIO device: {source_path}");
                continue;
            }
            log::debug!("Un-hiding device: {}", source_path);
            if let Err(e) = unhide_device(source_path.clone()).await {
                log::debug!("Unable to unhide device {source_path}: {e}");
            }
        }
    }

    /// Creates and adds a source device using the given [UdevDevice]. The source
    /// device will only start after `run_devices()` has been called.
    pub fn add_device(
        &mut self,
        device: UdevDevice,
        config: &CompositeDeviceConfig,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        // Check to see if this source device should be blocked.
        let mut is_blocked = false;
        let mut is_blocked_evdev = false;
        let source_config = config.get_matching_device(&device);
        if let Some(source_config) = source_config.as_ref() {
            if let Some(blocked) = source_config.blocked {
                is_blocked = blocked;
            }
        }

        let subsystem = device.subsystem();

        // Hide the device if specified
        let should_passthru = source_config
            .as_ref()
            .and_then(|c| c.passthrough)
            .unwrap_or(false);
        let should_hide = !should_passthru && subsystem.as_str() != "iio";
        if should_hide {
            let source_path = device.devnode();
            self.source_devices_to_hide.push(source_path);
        }

        let source_device = match subsystem.as_str() {
            "input" => {
                log::debug!("Adding source device: {:?}", device.name());
                if is_blocked {
                    is_blocked_evdev = true;
                }
                let device = EventDevice::new(device, self.device.clone(), source_config.clone())?;
                SourceDevice::Event(device)
            }
            "hidraw" => {
                log::debug!("Adding source device: {:?}", device.name());
                let device = HidRawDevice::new(device, self.device.clone(), source_config.clone())?;
                SourceDevice::HidRaw(device)
            }
            "iio" => {
                log::debug!("Adding source device: {:?}", device.name());
                let device = IioDevice::new(device, self.device.clone(), source_config.clone())?;
                SourceDevice::Iio(device)
            }
            "leds" => {
                log::debug!("Adding source device: {:?}", device.name());
                let device = LedDevice::new(device, self.device.clone(), source_config.clone())?;
                SourceDevice::Led(device)
            }
            _ => {
                return Err(format!(
                    "Unspported subsystem: {subsystem}, unable to add source device {}",
                    device.name()
                )
                .into())
            }
        };

        // Get the capabilities of the source device.
        // TODO: When we *remove* a source device, we also need to remove
        // capabilities
        if !is_blocked {
            let capabilities = source_device.get_capabilities()?;
            for cap in capabilities {
                // TODO: Is this needed?
                //if self.translatable_capabilities.contains(&cap) {
                //    continue;
                //}
                self.capabilities.insert(cap);
            }
        }

        // Check if this device should be blocked from sending events to target devices.
        let id = source_device.get_id();
        if let Some(device_config) = config.get_matching_device(source_device.get_device_ref()) {
            if let Some(blocked) = device_config.blocked {
                // Blocked event devices should still be run so they can be
                // EVIOGRAB'd
                if blocked && !is_blocked_evdev {
                    self.source_devices_blocked.insert(id.clone());
                }
            }
        };

        // TODO: Based on the capability map in the config, translate
        // the capabilities.
        // Keep track of the source device
        let device_path = source_device.get_device_path();
        self.source_devices_discovered.push(source_device);
        self.source_device_paths.push(device_path);
        self.source_devices_used.push(id);

        self.signal_sources_changed();

        Ok(())
    }

    /// Remove the given source device.
    pub async fn remove_device(&mut self, device: UdevDevice) -> Result<(), Box<dyn Error>> {
        let path = device.devnode();
        let id = device.get_id();

        if let Some(idx) = self.source_device_paths.iter().position(|str| str == &path) {
            self.source_device_paths.remove(idx);
        };

        if let Some(idx) = self.source_devices_used.iter().position(|str| str == &id) {
            self.source_devices_used.remove(idx);
        };
        self.source_devices_blocked.remove(&id);

        // Signal to DBus that source devices have changed
        self.signal_sources_changed();

        log::debug!(
            "Current source device paths: {:?}",
            self.source_device_paths
        );
        log::debug!(
            "Current source devices used: {:?}",
            self.source_devices_used
        );

        Ok(())
    }

    /// Start and run the source devices that this composite device will
    /// consume.
    pub async fn run_devices(&mut self) -> Result<(), Box<dyn Error>> {
        // Hide the device if specified
        self.hide_devices().await;

        log::debug!("Starting new source devices");
        // Start listening for events from all source devices
        let sources = self.source_devices_discovered.drain(..);
        for source_device in sources {
            let device_id = source_device.get_id();
            // If the source device is blocked, don't bother running it
            if self.source_devices_blocked.contains(&device_id) {
                log::debug!("Source device '{device_id}' blocked. Skipping running.");
                continue;
            }

            let source_tx = source_device.client();
            self.source_devices.insert(device_id.clone(), source_tx);

            // Add the IIO IMU Dbus interface. We do this here because it needs the source
            // device transmitter and this is the only place we can refrence it at the moment.
            let device = source_device.get_device_ref().clone();
            if let SourceDevice::Iio(_) = source_device {
                SourceIioImuInterface::listen_on_dbus(self.dbus.clone(), device.clone()).await?;
            }

            let client = self.device.clone();
            self.source_device_tasks.spawn(async move {
                if let Err(e) = source_device.run().await {
                    log::error!("Failed running device: {:?}", e);
                }
                log::debug!("Source device closed");
                if let Err(e) = client.notify_source_device_stopped(device).await {
                    log::error!("Failed to send device stop command: {e}");
                }
            });
        }
        log::debug!("All source device tasks started");
        Ok(())
    }

    /// Stop all attached source devices
    pub async fn stop(&mut self) {
        log::debug!("Stopping all source devices");
        for (path, source) in &self.source_devices {
            log::debug!("Stopping source device: {path}");
            if let Err(e) = source.stop().await {
                log::debug!("Failed to stop source device {path}: {e}");
            }
        }

        log::debug!("Waiting for source device tasks to finish");
        while let Some(res) = self.source_device_tasks.join_next().await {
            let Err(e) = res else {
                continue;
            };
            log::error!("Error waiting for source device task to finish: {e}");
        }
    }

    /// Returns an array of all source devices ids being used by this device.
    pub fn get_devices_used(&self) -> Vec<String> {
        self.source_devices_used.clone()
    }

    /// Return a list of source device paths (e.g. /dev/hidraw0, /dev/input/event0)
    /// that this composite device is managing
    pub fn get_device_paths(&self) -> Vec<String> {
        self.source_device_paths.clone()
    }

    /// Emit a DBus signal when source devices change
    fn signal_sources_changed(&self) {
        let dbus_path = self.path.clone();
        let conn = self.dbus.clone();

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
                .source_device_paths_changed(iface_ref.signal_emitter())
                .await
            {
                log::error!("Failed to send source devices changed signal: {e:?}");
            }
        });
    }

    /// Process a single output event from a target device.
    pub async fn process_output_event(&mut self, event: OutputEvent) -> Result<(), Box<dyn Error>> {
        //log::trace!("Received output event: {:?}", event);

        // Handle any output events that need to upload FF effect data
        if let OutputEvent::Uinput(uinput) = event.borrow() {
            match uinput {
                UinputOutputEvent::FFUpload(id, data, target_dev) => {
                    // If this effect was already uploaded, just return the id
                    // back to the target device and inform all source devices
                    // to update the effect with the given data.
                    if let Some(source_effect_ids) = self.ff_effect_id_source_map.get(id) {
                        for (source_id, source_effect_id) in source_effect_ids.iter() {
                            let Some(source) = self.source_devices.get(source_id) else {
                                continue;
                            };
                            log::debug!("Updating effect {source_effect_id} from {source_id}");
                            if let Err(e) = source.update_effect(*source_effect_id, *data).await {
                                log::error!("Error updating effect '{id}' on {source_id}: {e:?}");
                            }
                        }
                        target_dev.send(Some(*id))?;
                        return Ok(());
                    }

                    // Upload the effect data to the source devices
                    let mut source_effect_ids = HashMap::new();
                    for (source_id, source) in self.source_devices.iter() {
                        log::debug!("Uploading effect to {source_id}");
                        match source.upload_effect(*data).await {
                            Ok(source_effect_id) => {
                                // An effect ID of -1 indicates the device does not support
                                // FF events.
                                if source_effect_id == -1 {
                                    continue;
                                }
                                log::debug!("Successfully uploaded effect to {source_id} with source effect id {source_effect_id}");
                                source_effect_ids.insert(source_id.clone(), source_effect_id);
                            }
                            Err(e) => {
                                log::error!("Error uploading effect to {source_id}: {e:?}");
                            }
                        }
                    }

                    // If no source devices uploaded the effect, don't bother
                    // allocating an effect id.
                    if source_effect_ids.is_empty() {
                        log::debug!("No source device available to handle FF effect");
                        target_dev.send(None)?;
                    }

                    // If upload was successful, return an effect ID
                    let id = self.ff_effect_ids.iter().next().copied();
                    if let Some(id) = id {
                        log::debug!("Uploaded effect with effect id {id}");
                        self.ff_effect_ids.remove(&id);
                        self.ff_effect_id_source_map.insert(id, source_effect_ids);
                        target_dev.send(Some(id))?;
                    } else {
                        target_dev.send(None)?;
                    }
                }
                UinputOutputEvent::FFErase(effect_id) => {
                    let effect_id = *effect_id as i16;
                    // Erase the effect from source devices
                    if let Some(source_effect_ids) = self.ff_effect_id_source_map.get(&effect_id) {
                        for (source_id, source_effect_id) in source_effect_ids.iter() {
                            let Some(source) = self.source_devices.get(source_id) else {
                                continue;
                            };
                            log::debug!("Erasing effect from {source_id}");
                            if let Err(e) = source.erase_effect(*source_effect_id).await {
                                log::warn!("Failed to erase FF effect from {source_id}: {:?}", e);
                            }
                        }
                    }

                    // Add the effect ID to list of available effect ids
                    log::debug!("Erased effect with effect id {effect_id}");
                    self.ff_effect_ids.insert(effect_id);
                    self.ff_effect_id_source_map.remove(&effect_id);
                }
            }

            log::trace!("Available effect IDs: {:?}", self.ff_effect_ids);
            log::debug!("Used effect IDs: {:?}", self.ff_effect_id_source_map);

            return Ok(());
        }

        // TODO: Only write the event to devices that are capabile of handling it
        for (source_id, source) in self.source_devices.iter() {
            // If this is a force feedback event, translate the effect id into
            // the source device's effect id.
            if let OutputEvent::Evdev(input_event) = event {
                if input_event.event_type().0 == evdev::EventType::FORCEFEEDBACK.0 {
                    // Lookup the source effect ids for the effect
                    let effect_id = input_event.code() as i16;
                    let value = input_event.value();
                    let Some(source_effect_ids) = self.ff_effect_id_source_map.get(&effect_id)
                    else {
                        log::warn!("Received FF event with unknown id: {effect_id}");
                        continue;
                    };

                    // Lookup the source effect id for this source device
                    let Some(source_effect_id) = source_effect_ids.get(source_id) else {
                        log::warn!("Unable to find source effect id for effect {effect_id} from {source_id}");
                        continue;
                    };

                    // Create a new FF event with the source device effect id.
                    let new_event = InputEvent::new_now(
                        evdev::EventType::FORCEFEEDBACK.0,
                        *source_effect_id as u16,
                        value,
                    );
                    let output_event = OutputEvent::Evdev(new_event);

                    // Write the FF event to the source device
                    if let Err(e) = source.write_event(output_event).await {
                        log::error!("Failed to send Output event to {}. {:?}", source_id, e)
                    }
                    continue;
                }
            }

            if let Err(e) = source.write_event(event.clone()).await {
                log::error!("Failed to send Output event to {}. {:?}", source_id, e)
            }
        }

        //log::trace!("Finished processing output events.");

        Ok(())
    }
}
