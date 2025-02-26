pub mod client;
pub mod command;

use std::{
    borrow::Borrow,
    collections::{
        hash_map::{Entry, HashMap},
        BTreeSet, HashSet,
    },
    error::Error,
};

use evdev::InputEvent;
use tokio::{
    sync::mpsc,
    task::{JoinHandle, JoinSet},
    time::Duration,
};
use zbus::Connection;

use crate::{
    config::{
        path::get_profiles_path, CapabilityMap, CapabilityMapping, CompositeDeviceConfig,
        DeviceProfile, ProfileMapping,
    },
    dbus::interface::{
        composite_device::CompositeDeviceInterface, source::iio_imu::SourceIioImuInterface,
    },
    input::{
        capability::{Capability, Gamepad, GamepadButton, Mouse},
        event::{
            native::NativeEvent,
            value::{InputValue, TranslationError},
            Event,
        },
        output_event::UinputOutputEvent,
        source::{
            evdev::EventDevice, hidraw::HidRawDevice, iio::IioDevice, led::LedDevice, SourceDevice,
        },
    },
    udev::{device::UdevDevice, hide_device, unhide_device},
};

use self::{client::CompositeDeviceClient, command::CompositeCommand};

use super::{
    manager::ManagerCommand, output_event::OutputEvent, source::client::SourceDeviceClient,
    target::client::TargetDeviceClient,
};

/// Size of the command channel buffer for processing input events and commands.
const BUFFER_SIZE: usize = 16384;

/// The [InterceptMode] defines whether or not inputs should be routed over
/// DBus instead of to the target devices. This can be used by overlays to
/// intercept input.
#[derive(Debug, Clone, PartialEq, Eq, Copy)]
pub enum InterceptMode {
    /// Pass all input to the target devices
    None,
    /// Pass all inputs to the target devices except the guide button
    Pass,
    /// Intercept all input and send nothing to the target devices
    Always,
    /// Intercept all gamepad input that would be routed to target devices and
    /// send events over dbus instead
    GamepadOnly,
}

/// A [CompositeDevice] represents any number source input devices that
/// can translate input to any target devices
#[derive(Debug)]
pub struct CompositeDevice {
    /// Connection to DBus
    conn: Connection,
    /// Transmit channel to communicate with the input manager
    manager: mpsc::Sender<ManagerCommand>,
    /// Configuration for the CompositeDevice
    config: CompositeDeviceConfig,
    /// Name of the [CompositeDeviceConfig] loaded for the device
    name: String,
    /// Capabilities describe all input capabilities from all source devices
    capabilities: HashSet<Capability>,
    /// Capability mapping for the CompositeDevice
    capability_map: Option<CapabilityMap>,
    /// Currently loaded [DeviceProfile] for the [CompositeDevice]. The [DeviceProfile]
    /// is used to translate input events.
    device_profile: Option<DeviceProfile>,
    /// Path to the currently loaded [DeviceProfile] for the CompositeDevice.
    device_profile_path: Option<String>,
    /// Map of profile source events to translate to one or more profile mapping
    /// configs that define how the source event should be translated.
    device_profile_config_map: HashMap<Capability, Vec<ProfileMapping>>,
    /// List of input capabilities that can be translated by the capability map
    translatable_capabilities: Vec<Capability>,
    /// List of currently "pressed" actions used to translate multiple input
    /// sequences into a single input event.
    translatable_active_inputs: Vec<Capability>,
    /// List of translated events that were emitted less than 8ms ago. This
    /// is required to support "on release" style buttons on some devices where
    /// a button "up" event will fire immediately after a "down" event upon
    /// physical release of the button.
    translated_recent_events: HashSet<Capability>,
    /// Keep track of translated events we've emitted so we can send
    /// release events
    emitted_mappings: HashMap<String, CapabilityMapping>,
    /// The DBus path this [CompositeDevice] is listening on
    dbus_path: String,
    /// Mode defining how inputs should be routed
    intercept_mode: InterceptMode,
    /// Transmit channel for sending commands to this composite device
    tx: mpsc::Sender<CompositeCommand>,
    /// Receiver channel for listening for commands
    rx: mpsc::Receiver<CompositeCommand>,
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
    /// Set of available Force Feedback effect IDs that are not in use
    /// TODO: Just use the keys from ff_effect_id_source_map to determine next id
    ff_effect_ids: BTreeSet<i16>,
    /// Source devices use their own IDs for uploaded force feedback effects.
    /// This mapping maps the composite device effect ids to source device effect ids.
    /// E.g. {3: {"evdev://event0": 6, "evdev://event1": 2}}
    ff_effect_id_source_map: HashMap<i16, HashMap<String, i16>>,
    /// List of intercept mode activation Capabilities
    intercept_activation_caps: Vec<Capability>,
    /// Capability to send when intercept mode is activated for the first time.
    intercept_mode_target_cap: Capability,
    /// List of currently active events that could trigger intercept mode.
    intercept_active_inputs: Vec<Capability>,
    /// List of currently active buttons and keys. Used to block "up" events for
    /// keys that have already been handled.
    active_inputs: Vec<Capability>,
    /// Mapping preventing input release events to come from capability that
    /// didn't start the input in the first place.
    /// The key value pairs are:
    ///  - KEY   - Target capability
    ///  - VALUE - Source capability
    exclusive_inputs: HashMap<Capability, Capability>,
}

impl CompositeDevice {
    pub fn new(
        conn: Connection,
        manager: mpsc::Sender<ManagerCommand>,
        config: CompositeDeviceConfig,
        device_info: UdevDevice,
        dbus_path: String,
        capability_map: Option<CapabilityMap>,
    ) -> Result<Self, Box<dyn Error>> {
        log::info!("Creating CompositeDevice with config: {}", config.name);
        let (tx, rx) = mpsc::channel(BUFFER_SIZE);
        let name = config.name.clone();
        let mut device = Self {
            conn,
            manager,
            config,
            name,
            capabilities: HashSet::new(),
            capability_map,
            device_profile: None,
            device_profile_path: None,
            device_profile_config_map: HashMap::new(),
            translatable_capabilities: Vec::new(),
            translatable_active_inputs: Vec::new(),
            translated_recent_events: HashSet::new(),
            emitted_mappings: HashMap::new(),
            dbus_path,
            intercept_mode: InterceptMode::None,
            tx,
            rx,
            source_devices: HashMap::new(),
            source_devices_discovered: Vec::new(),
            source_devices_to_hide: Vec::new(),
            source_devices_blocked: HashSet::new(),
            source_device_paths: Vec::new(),
            source_device_tasks: JoinSet::new(),
            source_devices_used: Vec::new(),
            target_devices: HashMap::new(),
            target_devices_by_capability: HashMap::new(),
            target_devices_queued: HashSet::new(),
            target_devices_suspended: Vec::new(),
            target_dbus_devices: HashMap::new(),
            ff_effect_ids: (0..64).collect(),
            ff_effect_id_source_map: HashMap::new(),
            intercept_activation_caps: vec![Capability::Gamepad(Gamepad::Button(
                GamepadButton::Guide,
            ))],
            intercept_mode_target_cap: Capability::Gamepad(Gamepad::Button(GamepadButton::Guide)),
            intercept_active_inputs: Vec::new(),
            active_inputs: Vec::new(),
            exclusive_inputs: HashMap::new(),
        };

        // Load the capability map if one was defined
        if device.capability_map.is_some() {
            device.load_capability_map()?;
        }

        // Load the default profile
        let profile_dir = get_profiles_path();
        let profile_path = profile_dir.join("default.yaml");
        let profile_path = profile_path.to_string_lossy().to_string();
        let profile = DeviceProfile::from_yaml_file(profile_path.clone())?;
        device.load_device_profile(Some(profile), Some(profile_path))?;

        // If a capability map is defined, add those target capabilities to
        // the hashset of implemented capabilities.
        if let Some(map) = device.capability_map.as_ref() {
            for mapping in map.mapping.clone() {
                let cap = mapping.target_event.clone().into();
                if cap == Capability::NotImplemented {
                    continue;
                }
                device.capabilities.insert(cap);
            }
        }

        if let Err(e) = device.add_source_device(device_info) {
            return Err(e.to_string().into());
        }

        Ok(device)
    }

    /// Return the DBus path of the composite device
    pub fn dbus_path(&self) -> &str {
        self.dbus_path.as_str()
    }

    /// Creates a new instance of the composite device interface on DBus.
    pub async fn listen_on_dbus(&self) -> Result<JoinHandle<()>, Box<dyn Error>> {
        let conn = self.conn.clone();
        let client = self.client();
        let path = String::from(self.dbus_path());
        let profile = self.device_profile.clone();
        let profile_path = self.device_profile_path.clone();
        Ok(tokio::spawn(async move {
            log::debug!("Starting dbus interface: {path}");
            let iface = CompositeDeviceInterface::new(client, profile, profile_path);
            if let Err(e) = conn.object_server().at(path.clone(), iface).await {
                log::debug!("Failed to start dbus interface {path}: {e:?}");
            } else {
                log::debug!("Started listening on dbus interface: {path}");
            }
        }))
    }

    /// Starts the [CompositeDevice] and listens for events from all source
    /// devices to translate the events and send them to the appropriate target.
    pub async fn run(&mut self) -> Result<(), Box<dyn Error>> {
        log::debug!("Starting composite device");

        let dbus_path = self.dbus_path.clone();

        // Start all source devices
        self.run_source_devices().await?;

        // Set persist value from config if set, used to determine
        // if CompositeDevice self-closes after all SourceDevices have
        // been removed.
        let persist = self
            .config
            .options
            .as_ref()
            .map(|options| options.persist.unwrap_or(false))
            .unwrap_or(false);

        // Loop and listen for command events
        log::debug!("CompositeDevice started");
        let mut buffer = Vec::with_capacity(BUFFER_SIZE);
        'main: loop {
            let num = self.rx.recv_many(&mut buffer, BUFFER_SIZE).await;
            if num == 0 {
                log::warn!("Unable to receive more commands. Channel closed.");
                break;
            }
            let mut devices_removed = false;
            //log::trace!("Received {num} command(s)");
            for cmd in buffer.drain(..) {
                log::trace!("Received command: {:?}", cmd);
                match cmd {
                    CompositeCommand::ProcessEvent(device_id, event) => {
                        if let Err(e) = self.process_event(device_id, event).await {
                            log::error!("Failed to process event: {:?}", e);
                            // TODO: Use proper errors to check for 'SendError' and
                            // stop the composite device
                            break 'main;
                        }
                    }
                    CompositeCommand::ProcessOutputEvent(event) => {
                        if let Err(e) = self.process_output_event(event).await {
                            log::error!("Failed to process output event: {:?}", e);
                        }
                    }
                    CompositeCommand::GetCapabilities(sender) => {
                        if let Err(e) = sender.send(self.capabilities.clone()).await {
                            log::error!("Failed to send capabilities: {:?}", e);
                        }
                    }
                    CompositeCommand::GetTargetCapabilities(sender) => {
                        let target_caps = match self.get_target_capabilities().await {
                            Ok(caps) => caps,
                            Err(e) => {
                                log::error!("Failed to get target capabilities: {e:?}");
                                continue;
                            }
                        };
                        if let Err(e) = sender.send(target_caps).await {
                            log::error!("Failed to send target capabilities: {:?}", e);
                        }
                    }
                    CompositeCommand::SetInterceptMode(mode) => self.set_intercept_mode(mode).await,
                    CompositeCommand::GetInterceptMode(sender) => {
                        if let Err(e) = sender.send(self.intercept_mode).await {
                            log::error!("Failed to send intercept mode: {:?}", e);
                        }
                    }
                    CompositeCommand::GetConfig(sender) => {
                        if let Err(e) = sender.send(self.config.clone()).await {
                            log::error!("Failed to send config: {e:?}");
                        }
                    }
                    CompositeCommand::GetSourceDevicePaths(sender) => {
                        if let Err(e) = sender.send(self.get_source_device_paths()).await {
                            log::error!("Failed to send source device paths: {:?}", e);
                        }
                    }
                    CompositeCommand::GetTargetDevicePaths(sender) => {
                        let paths = self.target_devices.keys().cloned().collect();
                        if let Err(e) = sender.send(paths).await {
                            log::error!("Failed to send target device paths: {:?}", e);
                        }
                    }
                    CompositeCommand::GetDBusDevicePaths(sender) => {
                        let paths = self.target_dbus_devices.keys().cloned().collect();
                        if let Err(e) = sender.send(paths).await {
                            log::error!("Failed to send dbus device paths: {:?}", e);
                        }
                    }
                    CompositeCommand::SourceDeviceAdded(device) => {
                        if let Err(e) = self.on_source_device_added(device).await {
                            log::error!("Failed to add source device: {:?}", e);
                        }
                    }
                    CompositeCommand::SourceDeviceStopped(device) => {
                        log::debug!("Detected source device stopped: {}", device.devnode());
                        if let Err(e) = self.on_source_device_removed(device).await {
                            log::error!("Failed to remove source device: {:?}", e);
                        }
                    }
                    CompositeCommand::SourceDeviceRemoved(device) => {
                        log::debug!("Detected source device removed: {}", device.devnode());
                        devices_removed = true;
                        if let Err(e) = self.on_source_device_removed(device).await {
                            log::error!("Failed to remove source device: {:?}", e);
                        }
                    }
                    CompositeCommand::SetTargetDevices(target_types) => {
                        if let Err(e) = self.set_target_devices(target_types).await {
                            log::error!("Failed to set target devices: {e:?}");
                        }
                    }
                    CompositeCommand::AttachTargetDevices(targets) => {
                        if let Err(e) = self.attach_target_devices(targets).await {
                            log::error!("Failed to attach target devices: {e:?}");
                        }
                    }
                    CompositeCommand::GetName(sender) => {
                        let name = self.name.clone();
                        if let Err(e) = sender.send(name).await {
                            log::error!("Failed to send device name: {:?}", e);
                        }
                    }
                    CompositeCommand::GetProfileName(sender) => {
                        let profile_name = self
                            .device_profile
                            .as_ref()
                            .map(|profile| profile.name.clone())
                            .unwrap_or_default();
                        if let Err(e) = sender.send(profile_name).await {
                            log::error!("Failed to send profile name: {:?}", e);
                        }
                    }
                    CompositeCommand::LoadProfileFromYaml(profile, sender) => {
                        log::debug!("Loading profile from yaml: {profile}");
                        let profile = match DeviceProfile::from_yaml(profile) {
                            Ok(p) => p,
                            Err(e) => {
                                if let Err(er) = sender.send(Err(e.to_string())).await {
                                    log::error!("Failed to send failed to load profile: {er:?}");
                                }
                                continue;
                            }
                        };
                        let result = match self.load_device_profile(Some(profile.clone()), None) {
                            Ok(_) => Ok(()),
                            Err(e) => Err(e.to_string()),
                        };
                        CompositeDeviceInterface::update_profile(
                            &self.conn,
                            &self.dbus_path,
                            Some(profile),
                            None,
                        );
                        if let Err(e) = sender.send(result).await {
                            log::error!("Failed to send load profile result: {:?}", e);
                        }
                    }
                    CompositeCommand::LoadProfilePath(path, sender) => {
                        log::debug!("Loading profile from path: {path}");
                        let profile = match DeviceProfile::from_yaml_file(path.clone()) {
                            Ok(p) => p,
                            Err(e) => {
                                if let Err(er) = sender.send(Err(e.to_string())).await {
                                    log::error!("Failed to send failed to load profile: {er:?}");
                                }
                                continue;
                            }
                        };
                        let result = match self
                            .load_device_profile(Some(profile.clone()), Some(path.clone()))
                        {
                            Ok(_) => Ok(()),
                            Err(e) => Err(e.to_string()),
                        };
                        CompositeDeviceInterface::update_profile(
                            &self.conn,
                            &self.dbus_path,
                            Some(profile),
                            Some(path),
                        );
                        if let Err(e) = sender.send(result).await {
                            log::error!("Failed to send load profile result: {:?}", e);
                        }
                    }
                    CompositeCommand::UpdateSourceCapabilities(_device_id, _capabilities) => (),
                    CompositeCommand::UpdateTargetCapabilities(dbus_path, capabilities) => {
                        log::debug!(
                            "Updating target capabilities for '{dbus_path}': {capabilities:?}"
                        );
                        self.update_target_capabilities(dbus_path, capabilities);
                    }
                    CompositeCommand::WriteEvent(event) => {
                        if let Err(e) = self.write_event(event).await {
                            log::error!("Failed to write event: {:?}", e);
                        }
                    }
                    CompositeCommand::WriteChordEvent(events) => {
                        if let Err(e) = self.write_chord_events(events).await {
                            log::error!("Failed to write event: {:?}", e);
                        }
                    }
                    CompositeCommand::WriteSendEvent(event) => {
                        if let Err(e) = self.write_send_event(event).await {
                            log::error!("Failed to write event: {:?}", e);
                        }
                    }
                    CompositeCommand::HandleEvent(event) => {
                        if let Err(e) = self.handle_event(event).await {
                            log::error!("Failed to write event: {:?}", e);
                        }
                    }
                    CompositeCommand::RemoveRecentEvent(cap) => {
                        self.translated_recent_events.remove(&cap);
                    }
                    CompositeCommand::SetInterceptActivation(activation_caps, target_cap) => {
                        self.set_intercept_activation(activation_caps, target_cap)
                    }
                    CompositeCommand::Stop => {
                        log::debug!("Got STOP signal. Stopping CompositeDevice: {dbus_path}");
                        break 'main;
                    }
                    CompositeCommand::Suspend(sender) => {
                        log::info!("Preparing for system suspend for: {dbus_path}");
                        self.handle_suspend().await;
                        if let Err(e) = sender.send(()).await {
                            log::error!("Failed to send suspend response: {e:?}");
                        }
                    }
                    CompositeCommand::Resume(sender) => {
                        log::info!("Preparing for system resume for: {dbus_path}");
                        self.handle_resume().await;
                        if let Err(e) = sender.send(()).await {
                            log::error!("Failed to send resume response: {e:?}");
                        }
                    }
                    CompositeCommand::IsSuspended(sender) => {
                        let is_suspended = !self.target_devices_suspended.is_empty();
                        log::debug!("Checking if device is suspended: {is_suspended}");
                        if let Err(e) = sender.send(is_suspended).await {
                            log::error!("Failed to send suspended response: {e:?}");
                        }
                    }
                }
            }

            // If no source devices remain after processing the queue, stop
            // the device unless configured to persist.
            if devices_removed && self.source_devices_used.is_empty() {
                if persist {
                    log::debug!("No source devices remain, but CompositeDevice {dbus_path} has persist enabled. Clearing target devices states.");
                    for (path, target) in &self.target_devices {
                        log::debug!("Clearing target device: {path}");
                        if let Err(e) = target.clear_state().await {
                            log::error!("Failed to clear target device state {path}: {e:?}");
                        }
                    }
                } else {
                    log::debug!("No source devices remain. Stopping CompositeDevice {dbus_path}");
                    break 'main;
                }
            }
        }
        log::info!("CompositeDevice stopping: {dbus_path}");

        // Stop all target devices
        log::debug!("Stopping target devices");
        for (path, target) in &self.target_devices {
            log::debug!("Stopping target device: {path}");
            if let Err(e) = target.stop().await {
                log::error!("Failed to stop target device {path}: {e:?}");
            }
        }
        for (path, target) in &self.target_dbus_devices {
            log::debug!("Stopping target dbus device: {path}");
            if let Err(e) = target.stop().await {
                log::error!("Failed to stop dbus device {path}: {e:?}");
            }
        }

        // Unhide all source devices
        for source_path in self.source_device_paths.clone() {
            if source_path.starts_with("/sys/bus/iio/devices") {
                log::debug!("Skipping unhiding IIO device: {source_path}");
                continue;
            }
            log::debug!("Un-hiding device: {}", source_path);
            if let Err(e) = unhide_device(source_path.clone()).await {
                log::debug!("Unable to unhide device {source_path}: {:?}", e);
            }
        }

        // Send stop command to all source devices
        for (path, source) in &self.source_devices {
            log::debug!("Stopping source device: {path}");
            if let Err(e) = source.stop().await {
                log::debug!("Failed to stop source device {path}: {e:?}");
            }
        }

        // Wait on all tasks
        log::debug!("Waiting for source device tasks to finish");
        while let Some(res) = self.source_device_tasks.join_next().await {
            res?;
        }

        log::info!("CompositeDevice stopped: {dbus_path}");

        Ok(())
    }

    /// Return a [CompositeDeviceClient] to communicate with the device while it
    /// is running
    pub fn client(&self) -> CompositeDeviceClient {
        self.tx.clone().into()
    }

    /// Returns an array of all source devices ids being used by this device.
    pub fn get_source_devices_used(&self) -> Vec<String> {
        self.source_devices_used.clone()
    }

    /// Sets the DBus target devices on the [CompositeDevice].
    pub fn set_dbus_devices(&mut self, devices: HashMap<String, TargetDeviceClient>) {
        self.target_dbus_devices = devices;
    }

    /// Return a list of source device paths (e.g. /dev/hidraw0, /dev/input/event0)
    /// that this composite device is managing
    fn get_source_device_paths(&self) -> Vec<String> {
        self.source_device_paths.clone()
    }

    /// Start and run the source devices that this composite device will
    /// consume.
    async fn run_source_devices(&mut self) -> Result<(), Box<dyn Error>> {
        // Hide the device if specified
        for source_path in self.source_devices_to_hide.drain(..) {
            log::debug!("Hiding device: {}", source_path);
            if let Err(e) = hide_device(source_path.as_str()).await {
                log::warn!("Failed to hide device '{source_path}': {e:?}");
            }
            log::debug!("Finished hiding device: {source_path}");
        }

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
            let tx = self.tx.clone();

            // Add the IIO IMU Dbus interface. We do this here because it needs the source
            // device transmitter and this is the only place we can refrence it at the moment.
            let device = source_device.get_device_ref().clone();
            if let SourceDevice::Iio(_) = source_device {
                SourceIioImuInterface::listen_on_dbus(self.conn.clone(), device.clone()).await?;
            }

            self.source_device_tasks.spawn(async move {
                if let Err(e) = source_device.run().await {
                    log::error!("Failed running device: {:?}", e);
                }
                log::debug!("Source device closed");
                if let Err(e) = tx.send(CompositeCommand::SourceDeviceStopped(device)).await {
                    log::error!("Failed to send device stop command: {:?}", e);
                }
            });
        }
        log::debug!("All source device tasks started");
        Ok(())
    }

    /// Process a single event from a source device. Events are piped through
    /// a translation layer, then dispatched to the appropriate target device(s)
    async fn process_event(
        &mut self,
        device_id: String,
        raw_event: Event,
    ) -> Result<(), Box<dyn Error>> {
        if self.source_devices_blocked.contains(&device_id) {
            log::trace!("Blocking event! {:?}", raw_event);
            return Ok(());
        }
        log::trace!("Received event: {:?} from {device_id}", raw_event);

        // Convert the event into a NativeEvent
        let event: NativeEvent = match raw_event {
            Event::Evdev(event) => event.into(),
            Event::HIDRaw => todo!(),
            Event::Native(event) => event,
            Event::DBus(_) => todo!(),
        };
        let cap = event.as_capability();
        log::trace!("Event capability: {:?}", cap);

        // Only send valid events to the target device(s)
        if cap == Capability::NotImplemented {
            log::trace!(
                "Refusing to send '{}' event to target devices.",
                cap.to_string()
            );
            return Ok(());
        }

        // Check if the event needs to be translated based on the
        // capability map. Translated events will be re-enqueued, so this will
        // return early.
        log::trace!(
            "Translatable capabilities: {:?}",
            self.translatable_capabilities
        );
        if self.capability_map.is_some() && self.translatable_capabilities.contains(&cap) {
            log::trace!("Capability mapping found for event");
            self.translate_capability(&event).await?;
            return Ok(());
        }
        self.handle_event(event).await?;

        Ok(())
    }

    /// Process a single output event from a target device.
    async fn process_output_event(&mut self, event: OutputEvent) -> Result<(), Box<dyn Error>> {
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

    /// Translate and write the given event to the appropriate target devices
    async fn handle_event(&mut self, event: NativeEvent) -> Result<(), Box<dyn Error>> {
        // Check if we need to reverse the event list.
        let is_pressed = event.pressed();
        // Check if this is is a single event or multiple events.
        let mut is_chord = false;
        // Track the delay for chord events.
        let mut sleep_time = 0;

        // Translate the event using the device profile.
        let mut events = if self.device_profile.is_some() {
            self.translate_event(&event)
                .await?
                .into_iter()
                .filter_map(|event| self.filter_event(event))
                .collect()
        } else {
            vec![event]
        };

        // Check if we need to reverse the event list.
        if events.len() > 1 {
            //log::trace!("Got chord: {events:?}");
            is_chord = true;
            if !is_pressed {
                events = events.into_iter().rev().collect();
                // To support on_release events, we need to sleep past the time it takes to emit
                // the down events.
                sleep_time = 80 * events.len() as u64;
                //log::trace!("Chord is an UP event. New chord: {events:?}");
            }
        }

        let intercept = self.intercept_mode == InterceptMode::Pass;

        for event in events {
            let cap = event.as_capability();

            // Track what is currently active so we can ignore extra events.
            match cap {
                Capability::None
                | Capability::Touchpad(_)
                | Capability::NotImplemented
                | Capability::Sync
                | Capability::DBus(_) => {}
                Capability::Keyboard(_) => {
                    if !self.is_new_active_event(&cap, is_pressed) {
                        continue;
                    }
                    if self
                        .is_intercept_event(&event, is_pressed, intercept)
                        .await?
                    {
                        continue;
                    }
                }
                Capability::Gamepad(ref t) => match t {
                    Gamepad::Button(_) => {
                        if !self.is_new_active_event(&cap, is_pressed) {
                            continue;
                        }
                        if self
                            .is_intercept_event(&event, is_pressed, intercept)
                            .await?
                        {
                            continue;
                        }
                    }
                    Gamepad::Axis(_)
                    | Gamepad::Trigger(_)
                    | Gamepad::Accelerometer
                    | Gamepad::Gyro => {}
                },
                Capability::Mouse(ref t) => match t {
                    Mouse::Motion => {}
                    Mouse::Button(_) => {
                        if !self.is_new_active_event(&cap, is_pressed) {
                            continue;
                        }
                        if self
                            .is_intercept_event(&event, is_pressed, intercept)
                            .await?
                        {
                            continue;
                        }
                    }
                },
                Capability::Touchscreen(_) => (),
            }

            // if this is a chord with no matches to the intercept_active_inputs, add a keypress
            // delay for event chords. This is required to support steam chords as it will passed
            // through or miss events if they aren't properly
            // timed.
            if is_chord {
                let tx = self.tx.clone();
                tokio::spawn(async move {
                    tokio::time::sleep(Duration::from_millis(sleep_time)).await;
                    if let Err(e) = tx.send(CompositeCommand::WriteEvent(event)).await {
                        log::error!("Failed to send chord event command: {:?}", e);
                    }
                });
                // Increment the sleep time.
                sleep_time += 80;
                continue;
            }

            // for single events we can emit immediatly without tokio overhead.
            self.write_event(event).await?;
        }
        Ok(())
    }

    /// Returns true if this is the first event in intercept_activation_caps, or a follow on event
    /// if the first event has already been pressed. Otherwise returns false.
    fn should_hold_intercept_input(&self, cap: &Capability) -> bool {
        let Some(first_cap) = self.intercept_activation_caps.first() else {
            log::debug!("No activation capabilities are set. Do not hold input.");
            return false;
        };
        if self.intercept_active_inputs.is_empty() && cap == first_cap {
            log::debug!("This is the first event in the activation capabilities. Hold input.");
            return true;
        }
        if !self.intercept_active_inputs.is_empty() {
            log::debug!("There are other activation capabilities. Hold input.");
            return true;
        }
        log::debug!("No other buttons are pressed and this is not the first in the list. Do not hold input.");
        false
    }
    // Filter out input-cancelling events that do not come from same
    // capability as the initiator
    fn filter_event(&mut self, event: NativeEvent) -> Option<NativeEvent> {
        let Some(src_cap) = event.get_source_capability() else {
            return Some(event);
        };
        let target_cap = event.as_capability();
        // Handle only button presses
        if !matches!(
            target_cap,
            Capability::Gamepad(Gamepad::Button(_))
                | Capability::Keyboard(_)
                | Capability::Mouse(Mouse::Button(_))
        ) {
            return Some(event);
        }
        let pressed = event.pressed();
        match self.exclusive_inputs.entry(target_cap) {
            Entry::Vacant(e) => {
                if pressed {
                    e.insert(src_cap.clone());
                }
                Some(event)
            }
            Entry::Occupied(e) => {
                if e.get() == &src_cap {
                    if !pressed {
                        e.remove();
                    }
                    Some(event)
                } else {
                    None
                }
            }
        }
    }

    /// Writes the given event to the appropriate target device.
    async fn write_event(&self, event: NativeEvent) -> Result<(), Box<dyn Error>> {
        let cap = event.as_capability();

        // If this event implements the DBus capability, send the event to DBus devices
        if matches!(cap, Capability::DBus(_)) {
            log::trace!("Emit dbus event: {:?}", event);
            #[allow(clippy::for_kv_map)]
            for (_, target) in &self.target_dbus_devices {
                target.write_event(event.clone()).await?;
            }
            return Ok(());
        }

        // If the device is in intercept mode, only send events to DBus
        // target devices.
        if self.intercept_mode == InterceptMode::Always {
            log::trace!("Emit intercepted event: {:?}", event);
            for target in self.target_dbus_devices.values() {
                target.write_event(event.clone()).await?;
            }
            return Ok(());
        }

        // If the device is in gamepad intercept mode, send gamepad events to
        // DBus target devices.
        if self.intercept_mode == InterceptMode::GamepadOnly
            && matches!(cap, Capability::Gamepad(_))
        {
            log::trace!("Emit intercepted event: {:?}", event);
            for target in self.target_dbus_devices.values() {
                target.write_event(event.clone()).await?;
            }
            return Ok(());
        }

        // Find all target devices capable of handling this event
        let Some(target_paths) = self.target_devices_by_capability.get(&cap) else {
            log::trace!("No target devices capable of handling this event: {cap}");
            return Ok(());
        };
        let target_devices: Vec<(&str, &TargetDeviceClient)> = target_paths
            .iter()
            .filter_map(|path| {
                let device = self.target_devices.get(path);
                device.map(|client| (path.as_str(), client))
            })
            .collect();

        // Only write the event to devices that are capabile of handling it
        log::trace!("Emit passed event: {:?}", event);
        for (name, target) in target_devices {
            if let Err(e) = target.write_event(event.clone()).await {
                log::error!("Failed to write event to: {name}: {e:?}");
            }
        }
        Ok(())
    }

    /// Handles writing events that come from the dbus send_event interface
    async fn write_send_event(&mut self, event: NativeEvent) -> Result<(), Box<dyn Error>> {
        let cap = event.as_capability();
        self.is_new_active_event(&cap, event.pressed());
        // Check to see if the event is in recently translated.
        // If it is, spawn a task to delay emit the event.
        let sleep_time = Duration::from_millis(4);
        let cap = event.as_capability();
        if self.translated_recent_events.contains(&cap) {
            log::debug!("Event emitted too quickly. Delaying emission.");
            let tx = self.tx.clone();
            tokio::task::spawn(async move {
                tokio::time::sleep(sleep_time).await;
                if let Err(e) = tx.send(CompositeCommand::WriteEvent(event)).await {
                    log::error!("Failed to send delayed event command: {:?}", e);
                }
            });

            return Ok(());
        }

        // Add the event to our list of recently device translated events
        self.translated_recent_events.insert(event.as_capability());

        // Spawn a task to remove the event from recent translated
        let tx = self.tx.clone();
        tokio::task::spawn(async move {
            tokio::time::sleep(sleep_time).await;
            if let Err(e) = tx.send(CompositeCommand::RemoveRecentEvent(cap)).await {
                log::error!("Failed to send remove recent event command: {:?}", e);
            }
        });

        //log::trace!("Emitting event: {:?}", event);
        self.write_event(event).await?;

        Ok(())
    }

    // Handles writing chord events that come fron the dbus send_button_chord interface
    async fn write_chord_events(&self, events: Vec<NativeEvent>) -> Result<(), Box<dyn Error>> {
        // Track the delay for chord events.
        let mut sleep_time = 0;

        for event in events {
            let tx = self.tx.clone();
            log::debug!("Send event {:?} at sleep time {sleep_time}", event);
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(sleep_time)).await;
                if let Err(e) = tx.send(CompositeCommand::WriteEvent(event)).await {
                    log::error!("Failed to send chord event command: {:?}", e);
                }
            });
            // Increment the sleep time.
            sleep_time += 80;
        }
        Ok(())
    }

    /// Loads the input capabilities to translate from the capability map
    fn load_capability_map(&mut self) -> Result<(), Box<dyn Error>> {
        let Some(map) = self.capability_map.as_ref() else {
            return Err("Cannot translate device capabilities without capability map!".into());
        };

        // Loop over each mapping and try to match source events
        for mapping in map.mapping.iter() {
            for source_event in mapping.source_events.iter() {
                let cap = source_event.clone().into();
                if cap == Capability::NotImplemented {
                    continue;
                }
                self.translatable_capabilities.push(cap);
            }
        }

        Ok(())
    }

    /// Sets the intercept mode to the given value
    async fn set_intercept_mode(&mut self, mode: InterceptMode) {
        log::debug!("Setting intercept mode to: {:?}", mode);
        self.intercept_mode = mode;

        // Nothing else is required when turning off input interception.
        if mode == InterceptMode::None || mode == InterceptMode::Pass {
            return;
        }

        // If intercept mode is being turned on, clear the state from
        // any target devices to prevent further input events.
        for (path, device) in self.target_devices.iter() {
            // If intercept is set to only intercept gamepad input, only target
            // gamepad devices need to have their state cleared.
            if mode == InterceptMode::GamepadOnly && !path.contains("gamepad") {
                log::debug!("Intercept mode is set to GamepadOnly, skipping clearing state on target device {path}");
                continue;
            }

            log::debug!("Clearing state on device: {path}");
            if let Err(e) = device.clear_state().await {
                log::error!("Failed to clear state on target device {path}: {e:?}");
            }
        }
    }

    /// Translates the given event into a different event based on the given
    /// [CapabilityMap].
    async fn translate_capability(&mut self, event: &NativeEvent) -> Result<(), Box<dyn Error>> {
        // Get the capability map to translate input events
        let Some(map) = self.capability_map.as_ref() else {
            return Err("Cannot translate device capability without capability map!".into());
        };

        // Add or remove the event from translatable_active_inputs.
        let event_capability = event.as_capability();
        let capability_idx = self
            .translatable_active_inputs
            .iter()
            .position(|c| c == &event_capability);
        if event.pressed() {
            if capability_idx.is_none() {
                log::trace!("Adding capability to active inputs: {:?}", event_capability);
                self.translatable_active_inputs.push(event_capability);
                log::trace!(
                    "Active translatable inputs: {:?}",
                    self.translatable_active_inputs
                );
            } else {
                return Ok(());
            }
        } else if capability_idx.is_some() {
            log::trace!(
                "Removing capability from active inputs: {:?}",
                event_capability
            );
            let idx = capability_idx.unwrap();
            self.translatable_active_inputs.remove(idx);
            log::trace!(
                "Active translatable inputs: {:?}",
                self.translatable_active_inputs
            );
        } else {
            return Ok(());
        }

        // Keep a list of events to emit. The reason for this is some mapped
        // capabilities may use one or more of the same source capability and
        // they would release at the same time.
        let mut emit_queue = Vec::new();

        // Loop over each mapping and try to match source events
        for mapping in map.mapping.iter() {
            // If the event was not pressed and it exists in the emitted_mappings array,
            // then we need to check to see if ALL of its events no longer exist in
            // translatable_active_inputs.
            if !event.pressed() && self.emitted_mappings.contains_key(&mapping.name) {
                let mut has_source_event_pressed = false;

                // Loop through each source capability in the mapping
                for source_event in mapping.source_events.iter() {
                    let cap = source_event.clone().into();
                    if cap == Capability::NotImplemented {
                        continue;
                    }
                    if self.translatable_active_inputs.contains(&cap) {
                        has_source_event_pressed = true;
                        break;
                    }
                }

                // If no more inputs are being pressed, send a release event.
                if !has_source_event_pressed {
                    let cap = mapping.target_event.clone().into();
                    if cap == Capability::NotImplemented {
                        continue;
                    }
                    let event = NativeEvent::new(cap, InputValue::Bool(false));
                    log::trace!("Adding event to emit queue: {:?}", event);
                    emit_queue.push(event);
                    self.emitted_mappings.remove(&mapping.name);
                }
            }

            // If the event is pressed, check for any matches to send a 'press' event
            if event.pressed() {
                let mut is_missing_source_event = false;
                for source_event in mapping.source_events.iter() {
                    let cap = source_event.clone().into();
                    if cap == Capability::NotImplemented {
                        continue;
                    }
                    if !self.translatable_active_inputs.contains(&cap) {
                        is_missing_source_event = true;
                        break;
                    }
                }

                if !is_missing_source_event {
                    let cap = mapping.target_event.clone().into();
                    if cap == Capability::NotImplemented {
                        continue;
                    }
                    let event = NativeEvent::new(cap, InputValue::Bool(true));
                    log::trace!("Adding event to emit queue: {:?}", event);
                    emit_queue.push(event);
                    self.emitted_mappings
                        .insert(mapping.name.clone(), mapping.clone());
                }
            }
        }

        // Emit the translated events. If this translated event has been emitted
        // very recently, delay sending subsequent events of the same type.
        let sleep_time = Duration::from_millis(4);
        for event in emit_queue {
            // Check to see if the event is in recently translated.
            // If it is, spawn a task to delay emit the event.
            let cap = event.as_capability();
            if self.translated_recent_events.contains(&cap) {
                log::debug!("Event emitted too quickly. Delaying emission.");
                let tx = self.tx.clone();
                tokio::task::spawn(async move {
                    tokio::time::sleep(sleep_time).await;
                    if let Err(e) = tx.send(CompositeCommand::HandleEvent(event)).await {
                        log::error!("Failed to send delayed event command: {:?}", e);
                    }
                });

                continue;
            }

            // Add the event to our list of recently device translated events
            self.translated_recent_events.insert(event.as_capability());

            // Spawn a task to remove the event from recent translated
            let tx = self.tx.clone();
            tokio::task::spawn(async move {
                tokio::time::sleep(sleep_time).await;
                if let Err(e) = tx.send(CompositeCommand::RemoveRecentEvent(cap)).await {
                    log::error!("Failed to send remove recent event command: {:?}", e);
                }
            });

            log::trace!("Emitting event: {:?}", event);
            self.handle_event(event).await?;
        }

        Ok(())
    }

    /// Translates the given event into a Vec of events based on the currently loaded
    /// [DeviceProfile]
    async fn translate_event(
        &self,
        event: &NativeEvent,
    ) -> Result<Vec<NativeEvent>, Box<dyn Error>> {
        // Lookup the profile mapping associated with this event capability. If
        // none is found, return the original un-translated event.
        let source_cap = event.as_capability();
        if let Some(mappings) = self.device_profile_config_map.get(&source_cap) {
            // Find which mappings in the device profile matches this source event
            let matched_mappings = mappings
                .iter()
                .filter(|mapping| mapping.source_matches_properties(event));

            let mut events = Vec::new();
            // Based on all found mappings, translate the event
            for mapping in matched_mappings {
                log::trace!(
                    "Found translation for event {:?} in profile mapping: {}",
                    source_cap,
                    mapping.name
                );

                // Translate the event into the defined target event(s)
                for target_event in mapping.target_events.iter() {
                    // TODO: We can cache this conversion for faster translation
                    let target_cap: Capability = target_event.clone().into();
                    let result = event.get_value().translate(
                        &source_cap,
                        &mapping.source_event,
                        &target_cap,
                        target_event,
                    );
                    let value = match result {
                        Ok(v) => v,
                        Err(err) => {
                            match err {
                                TranslationError::NotImplemented => {
                                    log::warn!(
                                        "Translation not implemented for profile mapping '{}': {:?} -> {:?}",
                                        mapping.name,
                                        source_cap,
                                        target_cap,
                                    );
                                    continue;
                                }
                                TranslationError::ImpossibleTranslation(msg) => {
                                    log::warn!(
                                        "Impossible translation for profile mapping '{}': {msg}",
                                        mapping.name
                                    );
                                    continue;
                                }
                                TranslationError::InvalidSourceConfig(msg) => {
                                    log::warn!("Invalid source event config in profile mapping '{}': {msg}", mapping.name);
                                    continue;
                                }
                                TranslationError::InvalidTargetConfig(msg) => {
                                    log::warn!("Invalid target event config in profile mapping '{}': {msg}", mapping.name);
                                    continue;
                                }
                            }
                        }
                    };
                    if matches!(value, InputValue::None) {
                        continue;
                    }

                    let event = NativeEvent::new_translated(source_cap.clone(), target_cap, value);
                    events.push(event);
                }
            }
            return Ok(events);
        }

        log::trace!("No translation mapping found for event: {:?}", source_cap);
        Ok(vec![event.clone()])
    }

    /// Executed whenever a source device is added to this [CompositeDevice].
    async fn on_source_device_added(&mut self, device: UdevDevice) -> Result<(), Box<dyn Error>> {
        if let Err(e) = self.add_source_device(device) {
            return Err(e.to_string().into());
        }
        self.run_source_devices().await?;

        // Signal to DBus that source devices have changed
        self.signal_sources_changed().await;

        log::debug!(
            "Finished adding source device. All sources: {:?}",
            self.source_devices_used
        );
        Ok(())
    }

    /// Executed whenever a source device is removed from this [CompositeDevice]
    async fn on_source_device_removed(&mut self, device: UdevDevice) -> Result<(), Box<dyn Error>> {
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
        self.signal_sources_changed().await;

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

    /// Creates and adds a source device using the given [SourceDeviceInfo]
    fn add_source_device(
        &mut self,
        device: UdevDevice,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        // Check to see if this source device should be blocked.
        let mut is_blocked = false;
        let mut is_blocked_evdev = false;
        let source_config = self.config.get_matching_device(&device);
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
                let device = EventDevice::new(device, self.client(), source_config.clone())?;
                SourceDevice::Event(device)
            }
            "hidraw" => {
                log::debug!("Adding source device: {:?}", device.name());
                let device = HidRawDevice::new(device, self.client(), source_config.clone())?;
                SourceDevice::HidRaw(device)
            }
            "iio" => {
                log::debug!("Adding source device: {:?}", device.name());
                let device = IioDevice::new(device, self.client(), source_config.clone())?;
                SourceDevice::Iio(device)
            }
            "leds" => {
                log::debug!("Adding source device: {:?}", device.name());
                let device = LedDevice::new(device, self.client(), source_config.clone())?;
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
                if self.translatable_capabilities.contains(&cap) {
                    continue;
                }
                self.capabilities.insert(cap);
            }
        }

        // Check if this device should be blocked from sending events to target devices.
        let id = source_device.get_id();
        if let Some(device_config) = self
            .config
            .get_matching_device(source_device.get_device_ref())
        {
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

        Ok(())
    }

    /// Load the given device profile
    pub fn load_device_profile(
        &mut self,
        profile: Option<DeviceProfile>,
        profile_path: Option<String>,
    ) -> Result<(), Box<dyn Error>> {
        // Remove all outdated capability mappings.
        log::debug!("Clearing old device profile mappings");
        self.device_profile_config_map.clear();

        // Load the device profile
        self.device_profile = profile;
        self.device_profile_path = profile_path;
        let Some(profile) = self.device_profile.as_ref() else {
            log::debug!("Unloaded device profile");
            return Ok(());
        };
        if let Some(path) = self.device_profile_path.as_ref() {
            log::info!("Loading device profile `{}` from: {path}", profile.name);
        } else {
            log::info!("Loading device profile {}", profile.name);
        }

        // Loop through every mapping in the profile, extract the source and target events,
        // and map them into our profile map.
        for mapping in profile.mapping.iter() {
            log::trace!("Loading mapping from profile: {}", mapping.name);

            // Convert the source event configuration in the mapping into a
            // capability that can be easily matched on during event translation
            let source_event_cap: Capability = mapping.source_event.clone().into();

            // Convert the target events configuration into a vector of capabilities
            // that can be easily used to create translated events.
            let mut target_events_caps = Vec::new();
            for cap_config in mapping.target_events.clone() {
                let cap: Capability = cap_config.into();
                target_events_caps.push(cap);
            }

            // Insert the translation config for this event
            let config_map = self
                .device_profile_config_map
                .entry(source_event_cap)
                .or_default();
            config_map.push(mapping.clone());
        }

        // Set the target devices to use if it is defined in the profile
        if let Some(target_devices) = profile.target_devices.clone() {
            let tx = self.tx.clone();
            tokio::task::spawn(async move {
                if let Err(e) = tx
                    .send(CompositeCommand::SetTargetDevices(target_devices))
                    .await
                {
                    log::error!("Failed to send set target devices: {e:?}");
                }
            });
        }

        // Clear the state from all target devices
        let target_devices = self.target_devices.clone();
        tokio::task::spawn(async move {
            for (path, device) in target_devices.iter() {
                log::debug!("Clearing state on device: {path}");
                if let Err(e) = device.clear_state().await {
                    log::error!("Failed to clear state on target device {path}: {e:?}");
                }
            }
        });

        log::debug!("Successfully loaded device profile: {}", profile.name);
        Ok(())
    }

    fn set_intercept_activation(
        &mut self,
        activation_caps: Vec<Capability>,
        target_cap: Capability,
    ) {
        self.intercept_activation_caps = activation_caps;
        self.intercept_mode_target_cap = target_cap;
    }

    /// Adds or removes the given capability to the active inputs and returns true. If an up event is
    /// sent in that is not active it will retuirn false.
    fn is_new_active_event(&mut self, cap: &Capability, is_pressed: bool) -> bool {
        let active = self.active_inputs.contains(cap);
        if is_pressed && !active {
            log::debug!("New active capability: {cap:?}");
            self.active_inputs.push(cap.clone());
        }
        // Ignore up events for actions we've already handled.
        if !is_pressed && !active {
            log::debug!("Blocked up event for capability: {cap:?}");
            return false;
        }
        if !is_pressed && active {
            log::debug!("Removed inactive capability: {cap:?}");
            let index = self.active_inputs.iter().position(|r| r == cap).unwrap();
            self.active_inputs.remove(index);
        }
        true
    }

    async fn is_intercept_event(
        &mut self,
        event: &NativeEvent,
        is_pressed: bool,
        intercept: bool,
    ) -> Result<bool, Box<dyn Error>> {
        if self.intercept_activation_caps.len() == 1 {
            log::debug!("Checking single intercept event.");
            return self
                .is_intercept_event_single(event, is_pressed, intercept)
                .await;
        }
        log::debug!("Checking multi intercept event.");
        self.is_intercept_event_multi(event, is_pressed, intercept)
            .await
    }

    async fn is_intercept_event_single(
        &mut self,
        event: &NativeEvent,
        is_pressed: bool,
        intercept: bool,
    ) -> Result<bool, Box<dyn Error>> {
        let cap = event.as_capability();
        // Check if we have met the criteria for InterceptMode:Always
        if intercept && self.intercept_activation_caps.contains(&cap) && is_pressed {
            log::debug!("Found matching intercept event: {:?}", cap);
            log::debug!("It is a DOWN event!");
            // Stop here if this is a repeat event.
            if self.intercept_active_inputs.contains(&cap) {
                log::debug!("The event is already in the list. Skipping.");
                return Ok(true);
            };

            self.intercept_active_inputs.push(cap.clone());
            // Send the intercept target.
            log::debug!("Found activation chord!");
            self.set_intercept_mode(InterceptMode::Always).await;
            let target_event =
                NativeEvent::new(self.intercept_mode_target_cap.clone(), event.get_value());
            log::trace!("Release event: {target_event:?}");
            self.write_chord_events(vec![target_event]).await?;

            return Ok(true);
        } else if self.intercept_activation_caps.contains(&cap)
            && self.intercept_active_inputs.contains(&cap)
            && !is_pressed
        {
            // Check if we already sent the intercept event. We might not be in the same intercept mode
            // so dont check intercept.
            log::debug!("It is an UP event!");

            log::trace!("Remove from intercept active inputs: {cap:?}");
            let index = self
                .intercept_active_inputs
                .iter()
                .position(|r| r == &cap)
                .unwrap();
            self.intercept_active_inputs.remove(index);
            if self.active_inputs.contains(&cap) {
                log::trace!("Remove from active_inputs: {cap:?}");
                let index = self.active_inputs.iter().position(|r| r == &cap).unwrap();
                self.active_inputs.remove(index);
            }

            let target_event = NativeEvent::new(cap.clone(), event.get_value());
            log::trace!("Release event: {target_event:?}");
            self.write_chord_events(vec![target_event]).await?;

            return Ok(true);
        }
        log::trace!("Keep processing event: {event:?}");
        Ok(false)
    }

    async fn is_intercept_event_multi(
        &mut self,
        event: &NativeEvent,
        is_pressed: bool,
        intercept: bool,
    ) -> Result<bool, Box<dyn Error>> {
        let cap = event.as_capability();
        // Process the event depending on the intercept mode
        // Check if we have met the criteria for InterceptMode:Always
        if intercept && self.intercept_activation_caps.contains(&cap) {
            log::debug!("Found matching intercept event: {:?}", cap);
            if is_pressed && self.should_hold_intercept_input(&cap) {
                // Stop here if this is a repeat event.
                if self.intercept_active_inputs.contains(&cap) {
                    log::debug!("The event is already in the list. Skipping.");
                    return Ok(true);
                };
                // This is only a partial match, capture the event.
                self.intercept_active_inputs.push(cap.clone());
                if self.intercept_active_inputs.len() != self.intercept_activation_caps.len() {
                    log::debug!("More events needed to activate intercept mode.");
                    return Ok(true);
                }

                // We must have a match, we are of the correct length and all capabilities matched.
                log::debug!("Found activation chord!");
                for c in self.intercept_activation_caps.clone() {
                    if self.active_inputs.contains(&c) {
                        log::trace!("Removed inactive capability: {c:?}");
                        let index = self.active_inputs.iter().position(|r| r == &c).unwrap();
                        self.active_inputs.remove(index);
                    }
                }
                self.intercept_active_inputs.clear();

                self.set_intercept_mode(InterceptMode::Always).await;
                // Generate a new chord
                let event = NativeEvent::new(
                    self.intercept_mode_target_cap.clone(),
                    InputValue::Bool(true),
                );
                let event2 = NativeEvent::new(
                    self.intercept_mode_target_cap.clone(),
                    InputValue::Bool(false),
                );
                let chord: Vec<NativeEvent> = vec![event, event2];
                log::trace!("Release new chord: {chord:?}");
                self.write_chord_events(chord).await?;
                return Ok(true);
            } else if !is_pressed {
                log::debug!("It is an UP event!");
                // We only had a partial match and one of those events is released,
                // release it
                if self.intercept_active_inputs.contains(&cap) {
                    let index = self
                        .intercept_active_inputs
                        .iter()
                        .position(|r| r == &cap)
                        .unwrap();
                    self.intercept_active_inputs.remove(index);
                    let event = NativeEvent::new(cap.clone(), InputValue::Bool(true));
                    let event2 = NativeEvent::new(cap, InputValue::Bool(false));
                    let chord: Vec<NativeEvent> = vec![event, event2];
                    self.write_chord_events(chord).await?;
                    return Ok(true);
                }
            }
        } else if !self.intercept_active_inputs.is_empty() && is_pressed {
            // Handle chords with partial matches. Up events will be handled normally.
            log::debug!("This event is not what we're looking for.");
            self.intercept_active_inputs.push(cap);
            let mut chord: Vec<NativeEvent> = Vec::new();

            // Send all currently held events as a chord
            for c in self.intercept_active_inputs.clone() {
                let event = NativeEvent::new(c.clone(), InputValue::Bool(true));
                chord.push(event);
            }
            log::trace!("Release new chord: {chord:?}");
            self.write_chord_events(chord).await?;
            self.intercept_active_inputs.clear();
            return Ok(true);
        }

        log::trace!("Keep processing event: {event:?}");
        Ok(false)
    }

    /// Set the given target devices on the composite device. This will create
    /// new target devices, attach them to this device, and stop/remove any
    /// existing devices.
    async fn set_target_devices(
        &mut self,
        device_types: Vec<String>,
    ) -> Result<(), Box<dyn Error>> {
        log::info!("Setting target devices: {:?}", device_types);
        // Check to see if there are target device attachments pending. If so,
        // requeue this set_target_devices request.
        if !self.target_devices_queued.is_empty() {
            log::debug!(
                "Target devices already waiting for attachment. Re-queueing set target devices."
            );
            let tx = self.tx.clone();
            tokio::task::spawn(async move {
                if let Err(e) = tx
                    .send(CompositeCommand::SetTargetDevices(device_types))
                    .await
                {
                    log::error!("Error sending composite device command! {e:?}");
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

        let composite_path = self.dbus_path.clone();

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

    // Get the capabilities of all target devices
    async fn get_target_capabilities(&self) -> Result<HashSet<Capability>, Box<dyn Error>> {
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
    fn update_target_capabilities(&mut self, dbus_path: String, capabilities: HashSet<Capability>) {
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
    async fn attach_target_devices(
        &mut self,
        targets: HashMap<String, TargetDeviceClient>,
    ) -> Result<(), Box<dyn Error>> {
        let dbus_path = self.dbus_path.clone();

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
            if let Err(e) = target.set_composite_device(self.client()).await {
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

    /// Emit a DBus signal when target devices change
    async fn signal_targets_changed(&self) {
        let dbus_path = self.dbus_path.clone();
        let conn = self.conn.clone();

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

    /// Emit a DBus signal when source devices change
    async fn signal_sources_changed(&self) {
        let dbus_path = self.dbus_path.clone();
        let conn = self.conn.clone();

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

    /// Called when notified by the input manager that system suspend is about
    /// to happen.
    async fn handle_suspend(&mut self) {
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
    async fn handle_resume(&mut self) {
        log::info!(
            "Restoring target devices: {:?}",
            self.target_devices_suspended
        );

        // Set the target devices back to the ones used before suspend
        if let Err(err) = self
            .set_target_devices(self.target_devices_suspended.clone())
            .await
        {
            log::error!("Failed to set restore target devices: {err:?}");
        }

        // Clear the list of suspended target devices
        self.target_devices_suspended.clear();
    }
}
