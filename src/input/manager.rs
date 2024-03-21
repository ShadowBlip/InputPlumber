use std::collections::HashMap;
use std::error::Error;
use std::fs;

use tokio::sync::broadcast;
use tokio::sync::mpsc;
use zbus::fdo;
use zbus::zvariant::ObjectPath;
use zbus::Connection;
use zbus_macros::dbus_interface;

use crate::config::CapabilityMap;
use crate::config::CompositeDeviceConfig;
use crate::config::SourceDevice;
use crate::constants::BUS_PREFIX;
use crate::constants::BUS_TARGETS_PREFIX;
use crate::dmi::data::DMIData;
use crate::dmi::get_dmi_data;
use crate::input::composite_device;
use crate::input::composite_device::CompositeDevice;
use crate::input::source;
use crate::input::source::hidraw;
use crate::input::target::dbus::DBusDevice;
use crate::input::target::gamepad::GenericGamepad;
use crate::input::target::keyboard::KeyboardDevice;
use crate::input::target::mouse::MouseDevice;
use crate::input::target::steam_deck::SteamDeckDevice;
use crate::input::target::xb360::XBox360Controller;
use crate::input::target::TargetDeviceType;
use crate::procfs;
use crate::watcher;

use super::composite_device::Handle;
use super::target::TargetCommand;

const DEV_PATH: &str = "/dev";
const INPUT_PATH: &str = "/dev/input";
const BUFFER_SIZE: usize = 1024;

/// Manager commands define all the different ways to interact with [Manager]
/// over a channel. These commands are processed in an asyncronous thread and
/// dispatched as they come in.
#[derive(Debug, Clone)]
pub enum Command {
    SourceDeviceAdded { id: String, info: SourceDeviceInfo },
    SourceDeviceRemoved { id: String },
    EventDeviceAdded { name: String },
    EventDeviceRemoved { name: String },
    HIDRawAdded { name: String },
    HIDRawRemoved { name: String },
    CreateCompositeDevice { config: CompositeDeviceConfig },
    CompositeDeviceStopped(String),
}

#[derive(Debug, Clone)]
pub enum SourceDeviceInfo {
    EvdevDeviceInfo(procfs::device::Device),
    HIDRawDeviceInfo(hidapi::DeviceInfo),
}
/// The [DBusInterface] provides a DBus interface that can be exposed for managing
/// a [Manager]. It works by sending command messages to a channel that the
/// [Manager] is listening on.
struct DBusInterface {
    tx: broadcast::Sender<Command>,
}

impl DBusInterface {
    fn new(tx: broadcast::Sender<Command>) -> DBusInterface {
        DBusInterface { tx }
    }
}

#[dbus_interface(name = "org.shadowblip.InputManager")]
impl DBusInterface {
    #[dbus_interface(property)]
    async fn intercept_mode(&self) -> fdo::Result<String> {
        Ok("InputPlumber".to_string())
    }

    /// Create a composite device using the give composite device config. The
    /// path should be the absolute path to a composite device configuration file.
    async fn create_composite_device(&self, config_path: String) -> fdo::Result<String> {
        let device = CompositeDeviceConfig::from_yaml_file(config_path)
            .map_err(|err| fdo::Error::Failed(err.to_string()))?;
        self.tx
            .send(Command::CreateCompositeDevice { config: device })
            .map_err(|err| fdo::Error::Failed(err.to_string()))?;
        Ok("".to_string())
    }
}

/// Manages input devices
///
/// The [Manager] discovers input devices and interepts their input so
/// it can control what inputs should get passed on to the game and
/// what only an overlay should process. This works by grabbing exclusive
/// access to the physical gamepads and creating a virtual
/// gamepad that games can see.
///
/// SteamInput does this differently. It instead sets the 'SDL_GAMECONTROLLER_IGNORE_DEVICES'
/// environment variable whenever it launches a game to make the game ignore all
/// physical gamepads EXCEPT for Steam virtual gamepads.
/// https://github.com/godotengine/godot/pull/76045
pub struct Manager {
    /// The DBus connection
    dbus: Connection,
    /// System DMI data
    dmi_data: DMIData,
    /// The transmit side of the [rx] channel used to send [Command] messages.
    /// This can be cloned to allow child objects to communicate up to the
    /// manager.
    tx: broadcast::Sender<Command>,
    /// The receive side of the channel used to listen for [Command] messages
    /// from other objects.
    rx: broadcast::Receiver<Command>,
    /// Mapping of source devices to their SourceDevice objects.
    /// E.g. {"evdev://event0": <SourceDevice>}
    source_devices: HashMap<String, SourceDevice>,
    /// Mapping of source devices to their DBus path
    /// E.g. {"evdev://event0": "/org/shadowblip/InputPlumber/devices/source/event0"}
    source_device_dbus_paths: HashMap<String, String>,
    /// Map of source devices being used by a [CompositeDevice].
    /// E.g. {"evdev://event0": "/org/shadowblip/InputPlumber/CompositeDevice0"}
    source_devices_used: HashMap<String, String>,
    /// Mapping of DBus path to its corresponding [CompositeDevice] handle
    /// E.g. {"/org/shadowblip/InputPlumber/CompositeDevice0": <Handle>}
    composite_devices: HashMap<String, composite_device::Handle>,
    /// Mapping of all source devices used by composite devices with the CompositeDevice path as
    /// the key for the hashmap.
    /// E.g. {"/org/shadowblip/InputPlumber/CompositeDevice0": Vec<SourceDevice>}
    composite_device_sources: HashMap<String, Vec<SourceDevice>>,
    /// Map of target devices being used by a [CompositeDevice].
    /// E.g. {"/org/shadowblip/InputPlumber/CompositeDevice0": Vec<"/org/shadowblip/InputPlumber/devices/target/dbus0">}
    composite_device_targets: HashMap<String, Vec<String>>,
    /// Mapping of DBus path to its corresponding [CompositeDeviceConfig]
    /// E.g. {"/org/shadowblip/InputPlumber/CompositeDevice0": <CompositeDeviceConfig>}
    used_configs: HashMap<String, CompositeDeviceConfig>,
    /// Mapping of target devices to their respective handles
    /// E.g. {"/org/shadowblip/InputPlumber/devices/target/dbus0": <Handle>}
    target_devices: HashMap<String, mpsc::Sender<TargetCommand>>,
}

impl Manager {
    /// Returns a new instance of Gamepad Manager
    pub fn new(conn: Connection) -> Manager {
        let (tx, rx) = broadcast::channel(BUFFER_SIZE);

        log::debug!("Loading DMI data");
        let dmi_data = get_dmi_data();
        log::debug!("Got DMI data: {:?}", dmi_data);

        Manager {
            dbus: conn,
            dmi_data,
            rx,
            tx,
            composite_devices: HashMap::new(),
            source_devices: HashMap::new(),
            source_device_dbus_paths: HashMap::new(),
            source_devices_used: HashMap::new(),
            target_devices: HashMap::new(),
            used_configs: HashMap::new(),
            composite_device_sources: HashMap::new(),
            composite_device_targets: HashMap::new(),
        }
    }

    /// Starts listening for [Command] messages to be sent from clients and
    /// dispatch those events.
    pub async fn run(&mut self) -> Result<(), Box<dyn Error>> {
        // Start tasks for discovering new input devices
        self.watch_input_devices().await?;

        // Create a DBus interface
        self.listen_on_dbus().await?;

        // Loop and listen for command events
        while let Ok(cmd) = self.rx.recv().await {
            log::debug!("Received command: {:?}", cmd);
            match cmd {
                Command::EventDeviceAdded { name } => {
                    if let Err(e) = self.on_event_device_added(name).await {
                        log::error!("Error adding event device: {:?}", e);
                    }
                }
                Command::EventDeviceRemoved { name } => {
                    if let Err(e) = self.on_event_device_removed(name).await {
                        log::error!("Error removing event device: {:?}", e);
                    }
                }
                Command::HIDRawAdded { name } => {
                    if let Err(e) = self.on_hidraw_added(name).await {
                        log::error!("Error adding hidraw device: {:?}", e);
                    }
                }
                Command::HIDRawRemoved { name } => {
                    if let Err(e) = self.on_hidraw_removed(name).await {
                        log::error!("Error removing hidraw device: {:?}", e);
                    }
                }
                Command::CreateCompositeDevice { config } => {
                    if let Err(e) = self.create_composite_device(config).await {
                        log::error!("Error creating composite device: {:?}", e);
                    }
                }
                Command::SourceDeviceAdded { id, info } => {
                    if let Err(e) = self.on_source_device_added(id, info).await {
                        log::error!("Error handling added source device: {:?}", e);
                    }
                }
                Command::SourceDeviceRemoved { id } => {
                    if let Err(e) = self.on_source_device_removed(id).await {
                        log::error!("Error handling removed source device: {:?}", e);
                    }
                }
                Command::CompositeDeviceStopped(path) => {
                    if let Err(e) = self.on_composite_device_stopped(path).await {
                        log::error!("Error handling stopped composite device: {:?}", e);
                    }
                }
            }
        }

        log::info!("Stopped input manager");

        Ok(())
    }

    /// Create a new [CompositeDevice] from the given [CompositeDeviceConfig]
    async fn create_composite_device(
        &mut self,
        _config: CompositeDeviceConfig,
    ) -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    /// Create a [CompositeDevice] from the given configuration
    async fn create_composite_device_from_config(
        &mut self,
        config: &CompositeDeviceConfig,
        device_info: SourceDeviceInfo,
    ) -> Result<CompositeDevice, Box<dyn Error>> {
        // Lookup the capability map associated with this config if it exists
        let capability_map = if let Some(map_id) = config.capability_map_id.clone() {
            log::debug!("Found capability mapping in config: {}", map_id);
            let capability_map = self.load_capability_mappings().await;
            capability_map.get(&map_id).cloned()
        } else {
            None
        };

        // Create a composite device to manage these devices
        log::info!("Found matching source devices: {:?}", config.name);
        let config = config.clone();
        let device = CompositeDevice::new(self.dbus.clone(), config, device_info, capability_map)?;

        // Check to see if there's already a CompositeDevice for
        // these source devices.
        // TODO: Should we allow multiple composite devices with the same source?
        let mut devices_in_use = false;
        let source_device_ids = device.get_source_devices_used();
        for (id, path) in self.source_devices_used.iter() {
            if !source_device_ids.contains(id) {
                continue;
            }
            log::debug!("Source device '{}' already in use by: {}", id, path);
            devices_in_use = true;
            break;
        }
        if devices_in_use {
            return Err("Source device(s) are already in use".into());
        }

        Ok(device)
    }

    /// Create target input device to emulate based on the given device type.
    async fn create_target_device(
        &mut self,
        kind: &str,
    ) -> Result<TargetDeviceType, Box<dyn Error>> {
        log::debug!("Creating target device: {kind}");
        // Create the target device to emulate based on the kind
        let device = match kind {
            "gamepad" => TargetDeviceType::GenericGamepad(GenericGamepad::new(self.dbus.clone())),
            "deck" => TargetDeviceType::SteamDeck(SteamDeckDevice::new(self.dbus.clone())),
            "xb360" => TargetDeviceType::XBox360(XBox360Controller::new()),
            "dbus" => TargetDeviceType::DBus(DBusDevice::new(self.dbus.clone())),
            "mouse" => TargetDeviceType::Mouse(MouseDevice::new(self.dbus.clone())),
            "keyboard" => TargetDeviceType::Keyboard(KeyboardDevice::new(self.dbus.clone())),
            _ => TargetDeviceType::Null,
        };
        log::debug!("Created target input device: {kind}");
        Ok(device)
    }

    /// Start and run the given target devices. Returns a HashMap of transmitters
    /// to send events to the given targets.
    async fn start_target_devices(
        &mut self,
        targets: Vec<TargetDeviceType>,
    ) -> Result<HashMap<String, mpsc::Sender<TargetCommand>>, Box<dyn Error>> {
        let mut target_devices = HashMap::new();
        for target in targets {
            match target {
                TargetDeviceType::Null => (),
                TargetDeviceType::Keyboard(mut device) => {
                    let path = self.next_target_path("keyboard")?;
                    let event_tx = device.transmitter();
                    target_devices.insert(path.clone(), event_tx.clone());
                    self.target_devices.insert(path.clone(), event_tx.clone());
                    device.listen_on_dbus(path).await?;
                    tokio::spawn(async move {
                        if let Err(e) = device.run().await {
                            log::error!("Failed to run target keyboard: {:?}", e);
                        }
                        log::debug!("Target keyboard device closed");
                    });
                }
                TargetDeviceType::Mouse(mut mouse) => {
                    let path = self.next_target_path("mouse")?;
                    let event_tx = mouse.transmitter();
                    target_devices.insert(path.clone(), event_tx.clone());
                    self.target_devices.insert(path.clone(), event_tx.clone());
                    mouse.listen_on_dbus(path).await?;
                    tokio::spawn(async move {
                        if let Err(e) = mouse.run().await {
                            log::error!("Failed to run target mouse: {:?}", e);
                        }
                        log::debug!("Target mouse device closed");
                    });
                }
                TargetDeviceType::GenericGamepad(mut gamepad) => {
                    let path = self.next_target_path("gamepad")?;
                    let event_tx = gamepad.transmitter();
                    target_devices.insert(path.clone(), event_tx.clone());
                    self.target_devices.insert(path.clone(), event_tx.clone());
                    gamepad.listen_on_dbus(path).await?;
                    tokio::spawn(async move {
                        if let Err(e) = gamepad.run().await {
                            log::error!("Failed to run target gamepad: {:?}", e);
                        }
                        log::debug!("Target gamepad device closed");
                    });
                }
                TargetDeviceType::DBus(mut device) => {
                    let path = self.next_target_path("dbus")?;
                    let event_tx = device.transmitter();
                    target_devices.insert(path.clone(), event_tx.clone());
                    self.target_devices.insert(path.clone(), event_tx.clone());
                    device.listen_on_dbus(path).await?;
                    tokio::spawn(async move {
                        if let Err(e) = device.run().await {
                            log::error!("Failed to run target dbus device: {:?}", e);
                        }
                        log::debug!("Target dbus device closed");
                    });
                }
                TargetDeviceType::SteamDeck(mut device) => {
                    let path = self.next_target_path("gamepad")?;
                    let event_tx = device.transmitter();
                    target_devices.insert(path.clone(), event_tx.clone());
                    self.target_devices.insert(path.clone(), event_tx.clone());
                    device.listen_on_dbus(path).await?;
                    tokio::spawn(async move {
                        if let Err(e) = device.run().await {
                            log::error!("Failed to run target steam deck device: {:?}", e);
                        }
                        log::debug!("Target steam deck device closed");
                    });
                }
                TargetDeviceType::XBox360(_) => todo!(),
            }
        }

        Ok(target_devices)
    }

    /// Starts the given [CompositeDevice]
    async fn start_composite_device(
        &mut self,
        mut device: CompositeDevice,
        config: CompositeDeviceConfig,
        target_types: Option<Vec<String>>,
        source_device: SourceDevice,
    ) -> Result<(), Box<dyn Error>> {
        // Generate the DBus tree path for this composite device
        let path = self.next_composite_dbus_path();

        // Keep track of the source devices that this composite device is
        // using.
        let source_device_ids = device.get_source_devices_used();
        for id in source_device_ids {
            self.source_devices_used.insert(id.clone(), path.clone());
            self.source_devices.insert(id, source_device.clone());
        }

        let composite_path = path.clone();
        if !self.composite_device_sources.contains_key(&composite_path) {
            self.composite_device_sources
                .insert(composite_path.clone(), Vec::new());
        }
        let sources = self
            .composite_device_sources
            .get_mut(&composite_path)
            .unwrap();
        sources.push(source_device);

        // Create a DBus interface for the device
        device.listen_on_dbus(path.clone()).await?;

        // Get a handle to the device
        let handle = device.handle();

        // Keep track of target devices that this composite device is using
        let mut target_device_paths = Vec::new();

        // Create a DBus target device
        let dbus_device = self.create_target_device("dbus").await?;
        let dbus_devices = self.start_target_devices(vec![dbus_device]).await?;
        let dbus_paths = dbus_devices.keys();
        for dbus_path in dbus_paths {
            target_device_paths.push(dbus_path.clone());
        }
        device.set_dbus_devices(dbus_devices);

        // Create target devices based on the configuration
        let mut target_devices = Vec::new();
        if let Some(target_devices_config) = target_types {
            for kind in target_devices_config {
                let device = self.create_target_device(kind.as_str()).await?;
                target_devices.push(device);
            }
        }

        // Start the target input devices
        let targets = self.start_target_devices(target_devices).await?;
        let target_paths = targets.keys();
        for target_path in target_paths {
            target_device_paths.push(target_path.clone());
        }

        // Run the device
        let dbus_path = path.clone();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            if let Err(e) = device.run(targets).await {
                log::error!("Error running device: {:?}", e);
            }
            log::debug!("Composite device stopped running: {:?}", dbus_path);
            if let Err(e) = tx.send(Command::CompositeDeviceStopped(dbus_path)) {
                log::error!("Error sending composite device stopped: {:?}", e);
            }
        });
        let comp_path = path.clone();

        // Add the device to our maps
        self.composite_devices.insert(comp_path, handle);
        log::debug!("Managed source devices: {:?}", self.source_devices_used);
        self.used_configs.insert(path, config);
        log::debug!("Used configs: {:?}", self.used_configs);
        self.composite_device_targets
            .insert(composite_path.clone(), target_device_paths);
        log::debug!("Used target devices: {:?}", self.composite_device_targets);

        Ok(())
    }

    /// Called when a composite device stops running
    async fn on_composite_device_stopped(&mut self, path: String) -> Result<(), Box<dyn Error>> {
        log::debug!("Removing composite device: {}", path);

        // Remove the DBus interface
        let dbus_path = ObjectPath::from_string_unchecked(path.clone());
        self.dbus
            .object_server()
            .remove::<composite_device::DBusInterface, ObjectPath>(dbus_path)
            .await?;

        // Find any source devices that were in use by the composite device
        let mut to_remove = Vec::new();
        for (id, composite_dbus_path) in self.source_devices_used.iter() {
            if composite_dbus_path.clone() != path {
                continue;
            }
            to_remove.push(id.clone());
        }
        for id in to_remove {
            self.source_devices_used.remove::<String>(&id);
        }

        // Find any target devices that were in use by the composite device
        if let Some(target_device_paths) = self.composite_device_targets.get(&path) {
            for target_device_path in target_device_paths {
                self.target_devices.remove(target_device_path);
            }
        }

        // Remove the composite device from our list
        self.composite_devices.remove::<String>(&path);
        log::debug!("Composite device removed: {}", path);
        self.used_configs.remove::<String>(&path);
        log::debug!("Used config removed: {}", path);
        self.composite_device_targets.remove(&path);
        log::debug!("Used target devices: {:?}", self.composite_device_targets);

        Ok(())
    }

    /// Called when any source device is added. This method will load all
    /// device configurations to check and see if any configuration matches
    /// the input devices on the system. If a match is found, a [CompositeDevice]
    /// will be created and started.
    async fn on_source_device_added(
        &mut self,
        id: String,
        device_info: SourceDeviceInfo,
    ) -> Result<(), Box<dyn Error>> {
        // Check all existing composite devices to see if this device is part of
        // their config
        'start: for composite_device in self.composite_devices.keys() {
            let Some(config) = self.used_configs.get(composite_device) else {
                continue;
            };
            log::debug!("Checking existing config {:?} for device", config.name);
            let source_devices = config.source_devices.clone();
            match device_info.clone() {
                SourceDeviceInfo::EvdevDeviceInfo(info) => {
                    log::debug!("Checking if existing composite device is missing event device");
                    for source_device in source_devices {
                        if source_device.evdev.is_none() {
                            continue;
                        }
                        if config.has_matching_evdev(&info, &source_device.clone().evdev.unwrap()) {
                            // Check if the device has already been used in this config or not, stop here if the device must be unique.
                            if let Some(sources) =
                                self.composite_device_sources.get(composite_device)
                            {
                                for source in sources {
                                    if source != &source_device {
                                        continue;
                                    }
                                    if let Some(unique) = source_device.clone().unique {
                                        if unique {
                                            log::debug!("Found unique device {:?}, not adding to composite device {}", source_device, composite_device);
                                            break 'start;
                                        }
                                    } else {
                                        log::debug!("Found unique device {:?}, not adding to composite device {}", source_device, composite_device);
                                        break 'start;
                                    }
                                }
                            }

                            log::info!("Found missing device, adding source device {id} to existing composite device: {composite_device}");
                            let handle = self.composite_devices.get(composite_device.as_str());
                            if handle.is_none() {
                                log::error!(
                                    "No existing composite device found for key {}",
                                    composite_device.as_str()
                                );
                                continue;
                            }
                            self.add_event_device_to_composite_device(&info, handle.unwrap())?;
                            self.source_devices_used
                                .insert(id.clone(), composite_device.clone());
                            let composite_id = composite_device.clone();
                            if !self.composite_device_sources.contains_key(&composite_id) {
                                self.composite_device_sources
                                    .insert(composite_id.clone(), Vec::new());
                            }
                            let sources = self
                                .composite_device_sources
                                .get_mut(&composite_id)
                                .unwrap();
                            sources.push(source_device.clone());
                            self.source_devices.insert(id, source_device.clone());

                            return Ok(());
                        }
                    }
                }
                SourceDeviceInfo::HIDRawDeviceInfo(info) => {
                    log::debug!("Checking if existing composite device is missing hidraw device");
                    for source_device in source_devices {
                        if source_device.hidraw.is_none() {
                            continue;
                        }
                        if config.has_matching_hidraw(&info, &source_device.clone().hidraw.unwrap())
                        {
                            // Check if the device has already been used in this config or not, stop here if the device must be unique.
                            if let Some(sources) =
                                self.composite_device_sources.get(composite_device)
                            {
                                for source in sources {
                                    if source != &source_device {
                                        continue;
                                    }
                                    if let Some(unique) = source_device.clone().unique {
                                        if unique {
                                            log::debug!("Found unique device {:?}, not adding to composite device {}", source_device, composite_device);
                                            break 'start;
                                        }
                                    } else {
                                        log::debug!("Found unique device {:?}, not adding to composite device {}", source_device, composite_device);
                                        break 'start;
                                    }
                                }
                            }

                            log::info!("Found missing device, adding source device {id} to existing composite device: {composite_device}");
                            let handle = self.composite_devices.get(composite_device.as_str());
                            if handle.is_none() {
                                log::error!(
                                    "No existing composite device found for key {}",
                                    composite_device.as_str()
                                );
                                continue;
                            }
                            self.add_hidraw_device_to_composite_device(&info, handle.unwrap())?;
                            self.source_devices_used
                                .insert(id.clone(), composite_device.clone());
                            let composite_id = composite_device.clone();
                            if !self.composite_device_sources.contains_key(&composite_id) {
                                self.composite_device_sources
                                    .insert(composite_id.clone(), Vec::new());
                            }
                            let sources = self
                                .composite_device_sources
                                .get_mut(&composite_id)
                                .unwrap();
                            sources.push(source_device.clone());

                            self.source_devices.insert(id, source_device.clone());
                            return Ok(());
                        }
                    }
                }
            }
            log::debug!("Device does not match existing device: {:?}", config.name);
        }
        log::debug!("No existing composite device matches device.");

        // Check all CompositeDevice configs to see if this device creates
        // a match that will automatically create a CompositeDevice.
        let configs = self.load_device_configs().await;
        log::debug!("Checking unused configs");
        for config in configs {
            log::debug!("Checking config {:?} for device", config.name);

            // Check to see if this configuration matches the system
            if !config.has_valid_matches(self.dmi_data.clone()) {
                log::debug!("Configuration does not match system");
                continue;
            }

            let source_devices = config.source_devices.clone();
            match device_info.clone() {
                SourceDeviceInfo::EvdevDeviceInfo(info) => {
                    for source_device in source_devices {
                        if source_device.evdev.is_none() {
                            continue;
                        }
                        // how to refrence source devices used by this config?

                        if config.has_matching_evdev(&info, &source_device.clone().evdev.unwrap()) {
                            log::info!("Found a matching event device, creating composite device");
                            let device = self
                                .create_composite_device_from_config(&config, device_info.clone())
                                .await?;

                            // Get the target input devices from the config
                            let target_devices_config = config.target_devices.clone();

                            // Create the composite deivce
                            self.start_composite_device(
                                device,
                                config,
                                target_devices_config,
                                source_device.clone(),
                            )
                            .await?;

                            return Ok(());
                        }
                    }
                }
                SourceDeviceInfo::HIDRawDeviceInfo(info) => {
                    for source_device in source_devices {
                        if source_device.hidraw.is_none() {
                            continue;
                        }
                        if config.has_matching_hidraw(&info, &source_device.clone().hidraw.unwrap())
                        {
                            log::info!("Found a matching hidraw device, creating composite device");
                            let device = self
                                .create_composite_device_from_config(&config, device_info.clone())
                                .await?;

                            // Get the target input devices from the config
                            let target_devices_config = config.target_devices.clone();

                            // Create the composite deivce
                            self.start_composite_device(
                                device,
                                config,
                                target_devices_config,
                                source_device.clone(),
                            )
                            .await?;

                            return Ok(());
                        }
                    }
                }
            }
            log::debug!("Device does not match config: {:?}", config.name);
        }
        log::debug!("No unused configs found for device.");

        Ok(())
    }

    /// Called when any source device is removed
    async fn on_source_device_removed(&mut self, id: String) -> Result<(), Box<dyn Error>> {
        log::debug!("Source device removed: {}", id);
        let Some(composite_device_path) = self.source_devices_used.get(&id) else {
            log::debug!("Source device not being managed by a composite device");
            return Ok(());
        };

        let Some(handle) = self.composite_devices.get(composite_device_path) else {
            return Err(format!("CompostiteDevice {} not found", composite_device_path).into());
        };

        handle
            .tx
            .send(composite_device::Command::SourceDeviceRemoved(id.clone()))?;

        let Some(sources) = self.composite_device_sources.get_mut(composite_device_path) else {
            return Err(format!("CompostiteDevice {} not found", composite_device_path).into());
        };
        let Some(device) = self.source_devices.get(&id) else {
            return Err(format!("Device {} not found in source devices", id).into());
        };

        let idx = sources.iter().position(|item| item == device);
        if idx.is_none() {
            return Err(format!("Device {} not found in composite device sources", id).into());
        }
        sources.remove(idx.unwrap());
        self.source_devices.remove(&id);
        Ok(())
    }

    /// Called when an event device (e.g. /dev/input/event5) is added
    async fn on_event_device_added(&mut self, handler: String) -> Result<(), Box<dyn Error>> {
        log::debug!("Event device added: {}", handler);

        // Look up the connected device using procfs
        let mut info: Option<procfs::device::Device> = None;
        let devices = procfs::device::get_all()?;
        for device in devices {
            for name in device.handlers.clone() {
                if name != handler {
                    continue;
                }
                info = Some(device.clone());
            }
        }
        let Some(info) = info else {
            return Err(format!("Failed to find device information for: {}", handler).into());
        };

        // Create a DBus interface for the event device
        source::evdev::DBusInterface::listen_on_dbus(
            self.dbus.clone(),
            handler.clone(),
            info.clone(),
        )
        .await?;

        // Signal that a source device was added
        let id = format!("evdev://{}", handler);
        self.tx.send(Command::SourceDeviceAdded {
            id: id.clone(),
            info: SourceDeviceInfo::EvdevDeviceInfo(info),
        })?;

        // Add the device as a source device
        let path = source::evdev::get_dbus_path(handler.clone());
        self.source_device_dbus_paths.insert(id, path);

        Ok(())
    }

    /// Called when an event device (e.g. /dev/input/event5) is removed
    async fn on_event_device_removed(&mut self, handler: String) -> Result<(), Box<dyn Error>> {
        log::debug!("Event device removed: {}", handler);

        // Remove the device from our hashmap
        let id = format!("evdev://{}", handler);
        self.source_device_dbus_paths.remove(&id);
        self.source_devices_used.remove(&id);

        // Remove the DBus interface
        let path = source::evdev::get_dbus_path(handler);
        let path = ObjectPath::from_string_unchecked(path.clone());
        self.dbus
            .object_server()
            .remove::<source::evdev::DBusInterface, ObjectPath>(path.clone())
            .await?;

        // Signal that a source device was removed
        self.tx.send(Command::SourceDeviceRemoved { id })?;

        Ok(())
    }

    /// Called when a hidraw device (e.g. /dev/hidraw0) is added
    async fn on_hidraw_added(&mut self, name: String) -> Result<(), Box<dyn Error>> {
        log::debug!("HIDRaw added: {}", name);
        let path = format!("/dev/{}", name);

        // Look up the connected device using hidapi
        let devices = hidraw::list_devices()?;
        let device = devices
            .iter()
            .find(|dev| dev.path().to_string_lossy() == path)
            .cloned();
        let Some(info) = device else {
            return Err(format!("Failed to find device information for: {}", path).into());
        };

        // Create a DBus interface for the hidraw device
        source::hidraw::DBusInterface::listen_on_dbus(self.dbus.clone(), info.clone()).await?;

        // Signal that a source device was added
        let id = format!("hidraw://{}", name);
        self.tx.send(Command::SourceDeviceAdded {
            id: id.clone(),
            info: SourceDeviceInfo::HIDRawDeviceInfo(info),
        })?;

        // Add the device as a source device
        let path = source::hidraw::get_dbus_path(path);
        self.source_device_dbus_paths.insert(id, path);

        Ok(())
    }

    /// Called when a hidraw device (e.g. /dev/hidraw0) is removed
    async fn on_hidraw_removed(&mut self, name: String) -> Result<(), Box<dyn Error>> {
        log::debug!("HIDRaw removed: {}", name);
        let id = format!("hidraw://{}", name);

        // Signal that a source device was removed
        self.tx
            .send(Command::SourceDeviceRemoved { id: id.clone() })?;

        self.source_device_dbus_paths.remove(&id);
        self.source_devices_used.remove(&id);

        Ok(())
    }

    /// Returns the next available target device dbus path
    fn next_target_path(&self, kind: &str) -> Result<String, Box<dyn Error>> {
        let max = 2048;
        let mut i = 0;
        loop {
            if i > max {
                return Err("Devices exceeded maximum of 2048".into());
            }
            let path = format!("{BUS_TARGETS_PREFIX}/{kind}{i}");
            if self.target_devices.get(&path).is_some() {
                i += 1;
                continue;
            }
            return Ok(path);
        }
    }

    /// Returns the next available composite device dbus path
    fn next_composite_dbus_path(&self) -> String {
        let max = 2048;
        let mut i = 0;
        loop {
            if i > max {
                return "Devices exceeded".to_string();
            }
            let path = format!("{}/CompositeDevice{}", BUS_PREFIX, i);
            if self.composite_devices.get(&path).is_some() {
                i += 1;
                continue;
            }
            return path;
        }
    }

    /// Starts watching for input devices that are added and removed.
    async fn watch_input_devices(&self) -> Result<(), Box<dyn Error>> {
        // Create a channel to handle watch events
        let (watcher_tx, mut watcher_rx) = mpsc::channel(BUFFER_SIZE);

        // Start watcher thread to listen for hidraw device changes
        let tx = watcher_tx.clone();
        tokio::task::spawn_blocking(move || {
            log::debug!("Started watcher thread");
            watcher::watch(DEV_PATH.into(), tx)
        });

        // Start watcher thread to listen for event device changes
        let tx = watcher_tx.clone();
        tokio::task::spawn_blocking(move || {
            log::debug!("Started watcher thread");
            watcher::watch(INPUT_PATH.into(), tx)
        });

        log::debug!("Performing initial input device discovery");
        // Perform an initial hidraw device discovery
        let paths = std::fs::read_dir(DEV_PATH)?;
        for entry in paths {
            if let Err(e) = entry {
                log::warn!("Unable to read from directory: {:?}", e);
                continue;
            }
            let path = entry.unwrap().file_name();
            let path = path.into_string().ok();
            let Some(path) = path else {
                continue;
            };
            if !path.starts_with("hidraw") {
                continue;
            }
            log::debug!("Discovered hidraw device: {:?}", path);
            let result = watcher_tx
                .send(watcher::WatchEvent::Create {
                    name: path,
                    base_path: DEV_PATH.into(),
                })
                .await;
            if let Err(e) = result {
                log::error!("Unable to send command: {:?}", e);
            }
        }

        // Perform an initial event device discovery
        let paths = std::fs::read_dir(INPUT_PATH)?;
        for entry in paths {
            if let Err(e) = entry {
                log::warn!("Unable to read from directory: {:?}", e);
                continue;
            }
            let path = entry.unwrap().file_name();
            let path = path.into_string().ok();
            let Some(path) = path else {
                continue;
            };
            if !path.starts_with("event") {
                continue;
            }
            log::debug!("Discovered event device: {:?}", path);
            let result = watcher_tx
                .send(watcher::WatchEvent::Create {
                    name: path,
                    base_path: INPUT_PATH.into(),
                })
                .await;
            if let Err(e) = result {
                log::error!("Unable to send command: {:?}", e);
            }
        }

        log::debug!("Initial input device discovery complete");

        // Start a task to dispatch filesystem watch events to the `run()` loop
        let cmd_tx = self.tx.clone();
        tokio::spawn(async move {
            log::debug!("Dispatching filesystem watch events");
            while let Some(event) = watcher_rx.recv().await {
                log::debug!("Received watch event: {:?}", event);
                match event {
                    // Create events
                    watcher::WatchEvent::Create { name, base_path } => {
                        if base_path == INPUT_PATH && name.starts_with("event") {
                            let result = cmd_tx.send(Command::EventDeviceAdded { name });
                            if let Err(e) = result {
                                log::error!("Unable to send command: {:?}", e);
                            }
                        } else if name.starts_with("hidraw") {
                            let result = cmd_tx.send(Command::HIDRawAdded { name });
                            if let Err(e) = result {
                                log::error!("Unable to send command: {:?}", e);
                            }
                        }
                    }
                    // Delete events
                    watcher::WatchEvent::Delete { name, base_path } => {
                        if base_path == INPUT_PATH && name.starts_with("event") {
                            let result = cmd_tx.send(Command::EventDeviceRemoved { name });
                            if let Err(e) = result {
                                log::error!("Unable to send command: {:?}", e);
                            }
                        } else if name.starts_with("hidraw") {
                            let result = cmd_tx.send(Command::HIDRawRemoved { name });
                            if let Err(e) = result {
                                log::error!("Unable to send command: {:?}", e);
                            }
                        }
                    }
                    _ => {}
                }
            }
        });

        Ok(())
    }

    /// Loads all capability mappings in all default locations and returns a hashmap
    /// of the CapabilityMap ID and the [CapabilityMap].
    pub async fn load_capability_mappings(&self) -> HashMap<String, CapabilityMap> {
        let mut mappings = HashMap::new();
        let paths = vec![
            "./rootfs/usr/share/inputplumber/capability_maps",
            "/etc/inputplumber/capability_maps.d",
            "/usr/share/inputplumber/capability_maps",
        ];

        // Look for capability mappings in all known locations
        for path in paths {
            let files = fs::read_dir(path);
            if files.is_err() {
                log::debug!("Failed to load directory {}: {}", path, files.unwrap_err());
                continue;
            }
            let mut files: Vec<_> = files.unwrap().map(|r| r.unwrap()).collect();
            files.sort_by_key(|dir| dir.file_name());

            // Look at each file in the directory and try to load them
            for file in files {
                let filename = file.file_name();
                let filename = filename.as_os_str().to_str().unwrap();

                // Skip any non-yaml files
                if !filename.ends_with(".yaml") {
                    continue;
                }

                // Try to load the composite device profile
                log::debug!("Found file: {}", file.path().display());
                let mapping = CapabilityMap::from_yaml_file(file.path().display().to_string());
                if mapping.is_err() {
                    log::debug!(
                        "Failed to parse capability mapping: {}",
                        mapping.unwrap_err()
                    );
                    continue;
                }
                let map = mapping.unwrap();
                mappings.insert(map.id.clone(), map);
            }
        }

        mappings
    }

    /// Looks in all default locations for [CompositeDeviceConfig] definitions and
    /// load/parse them. Returns an array of these configs which can be used
    /// to automatically create a [CompositeDevice].
    pub async fn load_device_configs(&self) -> Vec<CompositeDeviceConfig> {
        let task = tokio::task::spawn_blocking(move || {
            let mut devices: Vec<CompositeDeviceConfig> = Vec::new();
            let paths = vec![
                "./rootfs/usr/share/inputplumber/devices",
                "/etc/inputplumber/devices.d",
                "/usr/share/inputplumber/devices",
            ];

            // Look for composite device profiles in all known locations
            for path in paths {
                log::debug!("Checking {path} for composite device configs");
                let files = fs::read_dir(path);
                if files.is_err() {
                    log::debug!("Failed to load directory {}: {}", path, files.unwrap_err());
                    continue;
                }
                let mut files: Vec<_> = files.unwrap().map(|r| r.unwrap()).collect();
                files.sort_by_key(|dir| dir.file_name());

                // Look at each file in the directory and try to load them
                for file in files {
                    let filename = file.file_name();
                    let filename = filename.as_os_str().to_str().unwrap();

                    // Skip any non-yaml files
                    if !filename.ends_with(".yaml") {
                        continue;
                    }

                    // Try to load the composite device profile
                    log::debug!("Found file: {}", file.path().display());
                    let device =
                        CompositeDeviceConfig::from_yaml_file(file.path().display().to_string());
                    if device.is_err() {
                        log::debug!(
                            "Failed to parse composite device config: {}",
                            device.unwrap_err()
                        );
                        continue;
                    }
                    let device = device.unwrap();
                    devices.push(device);
                }
            }

            devices
        });

        let result = task.await;
        if let Err(ref e) = result {
            log::error!("Failed to run task to list device configs: {:?}", e);
        }

        result.unwrap_or_default()
    }

    /// Creates a DBus object
    async fn listen_on_dbus(&self) -> Result<(), Box<dyn Error>> {
        let iface = DBusInterface::new(self.tx.clone());
        let manager_path = format!("{}/Manager", BUS_PREFIX);
        self.dbus.object_server().at(manager_path, iface).await?;
        Ok(())
    }

    /// Send a signal using the given composite device handle that a new source
    /// device should be started.
    fn add_event_device_to_composite_device(
        &self,
        device_info: &procfs::device::Device,
        handle: &Handle,
    ) -> Result<(), Box<dyn Error>> {
        let tx = &handle.tx;
        let device_info = device_info.clone();
        tx.send(composite_device::Command::SourceDeviceAdded(
            SourceDeviceInfo::EvdevDeviceInfo(device_info),
        ))?;
        Ok(())
    }

    /// Send a signal using the given composite device handle that a new source
    /// device should be started.
    fn add_hidraw_device_to_composite_device(
        &self,
        device_info: &hidapi::DeviceInfo,
        handle: &Handle,
    ) -> Result<(), Box<dyn Error>> {
        let tx = &handle.tx;
        let device_info = device_info.clone();
        tx.send(composite_device::Command::SourceDeviceAdded(
            SourceDeviceInfo::HIDRawDeviceInfo(device_info),
        ))?;

        Ok(())
    }
}
