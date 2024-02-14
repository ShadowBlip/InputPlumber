use std::collections::HashMap;
use std::error::Error;
use std::fs;

use tokio::sync::broadcast;
use tokio::sync::mpsc;
use zbus::fdo;
use zbus::zvariant::ObjectPath;
use zbus::Connection;
use zbus_macros::dbus_interface;

use crate::config::CompositeDeviceConfig;
use crate::constants::BUS_PREFIX;
use crate::dmi::data::DMIData;
use crate::dmi::get_dmi_data;
use crate::input::composite_device;
use crate::input::composite_device::CompositeDevice;
use crate::input::source;
use crate::input::source::hidraw;
use crate::procfs;
use crate::watcher;

const DEV_PATH: &str = "/dev";
const INPUT_PATH: &str = "/dev/input";
const BUFFER_SIZE: usize = 1024;

/// Manager commands define all the different ways to interact with [Manager]
/// over a channel. These commands are processed in an asyncronous thread and
/// dispatched as they come in.
#[derive(Debug, Clone)]
pub enum Command {
    SourceDeviceAdded,
    SourceDeviceRemoved,
    EventDeviceAdded { name: String },
    EventDeviceRemoved { name: String },
    HIDRawAdded { name: String },
    HIDRawRemoved { name: String },
    CreateCompositeDevice { config: CompositeDeviceConfig },
    CompositeDeviceStopped(String),
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
    /// Mapping of source devices to their DBus path
    /// E.g. {"evdev://event0": "/org/shadowblip/InputPlumber/devices/source/event0"}
    source_devices: HashMap<String, String>,
    /// Map of source devices being used by a [CompositeDevice].
    /// E.g. {"evdev://event0": "/org/shadowblip/InputPlumber/CompositeDevice0"}
    source_devices_used: HashMap<String, String>,
    /// Mapping of DBus path to its corresponding [CompositeDevice] handle
    /// E.g. {"/org/shadowblip/InputPlumber/CompositeDevice0": <Handle>}
    composite_devices: HashMap<String, composite_device::Handle>,
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
            source_devices: HashMap::new(),
            source_devices_used: HashMap::new(),
            composite_devices: HashMap::new(),
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
                Command::SourceDeviceAdded => {
                    if let Err(e) = self.on_source_device_added().await {
                        log::error!("Error handling added source device: {:?}", e);
                    }
                }
                Command::SourceDeviceRemoved => {
                    if let Err(e) = self.on_source_device_removed().await {
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

        Ok(())
    }

    /// Create a new [CompositeDevice] from the given [CompositeDeviceConfig]
    async fn create_composite_device(
        &mut self,
        _config: CompositeDeviceConfig,
    ) -> Result<(), Box<dyn Error>> {
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

        // Remove the composite device from our list
        self.composite_devices.remove::<String>(&path);
        log::debug!("Composite device removed: {}", path);

        Ok(())
    }

    /// Called when any source device is added. This method will load all
    /// device configurations to check and see if any configuration matches
    /// the input devices on the system. If a match is found, a [CompositeDevice]
    /// will be created and started.
    async fn on_source_device_added(&mut self) -> Result<(), Box<dyn Error>> {
        // Check all CompositeDevice configs to see if this device creates
        // a match that will automatically create a CompositeDevice.
        let configs = self.load_device_configs().await;
        log::debug!("Checking configs");
        for config in configs {
            log::debug!("Got config: {:?}", config.name);

            // Check to see if this configuration matches the system
            if !config.has_valid_matches(self.dmi_data.clone()) {
                log::debug!("Configuration does not match system");
                continue;
            }

            // Skip configs where the required source devices don't exist.
            if !config.sources_exist()? {
                continue;
            }

            // TODO: Check to see if there's already a composite device running
            // but without all its source devices

            // Create a composite device to manage these devices
            log::info!("Found matching source devices: {:?}", config.name);
            let mut device = CompositeDevice::new(self.dbus.clone(), config)?;

            // Check to see if there's already a CompositeDevice for
            // these source devices.
            // TODO: Should we allow multiple composite devices with the same source?
            let mut devices_in_use = false;
            let source_device_ids = device.get_source_device_ids();
            for (id, path) in self.source_devices_used.iter() {
                if !source_device_ids.contains(id) {
                    continue;
                }
                log::debug!("Source device '{}' already in use by: {}", id, path);
                devices_in_use = true;
                break;
            }
            if devices_in_use {
                continue;
            }

            // Generate the DBus tree path for this composite device
            let path = self.next_composite_dbus_path();

            // Keep track of the source devices that this composite device is
            // using.
            for id in source_device_ids {
                self.source_devices_used.insert(id, path.clone());
            }

            // Create a DBus interface for the device
            log::info!("Creating composite device");
            device.listen_on_dbus(path.clone()).await?;

            // Get a handle to the device
            let handle = device.handle();

            // Run the device
            let dbus_path = path.clone();
            let tx = self.tx.clone();
            tokio::spawn(async move {
                if let Err(e) = device.run().await {
                    log::error!("Error running device: {:?}", e);
                }
                log::debug!("Composite device stopped running: {:?}", dbus_path);
                if let Err(e) = tx.send(Command::CompositeDeviceStopped(dbus_path)) {
                    log::error!("Error sending composite device stopped: {:?}", e);
                }
            });

            // Add the device to our map
            self.composite_devices.insert(path, handle);
            log::debug!("Managed source devices: {:?}", self.source_devices_used);
        }

        Ok(())
    }

    /// Called when any source device is removed
    async fn on_source_device_removed(&mut self) -> Result<(), Box<dyn Error>> {
        log::debug!("Source device removed");
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
        self.tx.send(Command::SourceDeviceAdded)?;

        // Add the device as a source device
        let path = source::evdev::get_dbus_path(handler.clone());
        let id = format!("evdev://{}", handler);
        self.source_devices.insert(id, path);

        Ok(())
    }

    /// Called when an event device (e.g. /dev/input/event5) is removed
    async fn on_event_device_removed(&mut self, handler: String) -> Result<(), Box<dyn Error>> {
        log::debug!("Event device removed: {}", handler);

        // Remove the device from our hashmap
        let id = format!("evdev://{}", handler);
        self.source_devices.remove(&id);

        // Remove the DBus interface
        let path = source::evdev::get_dbus_path(handler);
        let path = ObjectPath::from_string_unchecked(path.clone());
        self.dbus
            .object_server()
            .remove::<source::evdev::DBusInterface, ObjectPath>(path.clone())
            .await?;

        // Signal that a source device was removed
        self.tx.send(Command::SourceDeviceRemoved)?;

        Ok(())
    }

    /// Called when a hidraw device (e.g. /dev/hidraw0) is added
    async fn on_hidraw_added(&mut self, name: String) -> Result<(), Box<dyn Error>> {
        log::debug!("HIDRaw added: {}", name);
        let path = format!("/dev/{}", name);

        // Signal that a source device was added
        self.tx.send(Command::SourceDeviceAdded)?;

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
        self.tx.send(Command::SourceDeviceAdded)?;

        // Add the device as a source device
        let path = source::hidraw::get_dbus_path(path);
        let id = format!("hidraw://{}", name);
        self.source_devices.insert(id, path);

        Ok(())
    }

    /// Called when a hidraw device (e.g. /dev/hidraw0) is removed
    async fn on_hidraw_removed(&self, name: String) -> Result<(), Box<dyn Error>> {
        log::debug!("HIDRaw removed: {}", name);

        // Signal that a source device was removed
        self.tx.send(Command::SourceDeviceRemoved)?;

        Ok(())
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

    /// Looks in all default locations for [CompositeDeviceConfig] definitions and
    /// load/parse them. Returns an array of these configs which can be used
    /// to automatically create a [CompositeDevice].
    pub async fn load_device_configs(&self) -> Vec<CompositeDeviceConfig> {
        let task = tokio::task::spawn_blocking(move || {
            let mut devices: Vec<CompositeDeviceConfig> = Vec::new();
            let paths = vec![
                "/usr/share/inputplumber/devices",
                "/etc/inputplumber/devices.d",
                "./rootfs/usr/share/inputplumber/devices",
            ];

            // Look for composite device profiles in all known locations
            for path in paths {
                let files = fs::read_dir(path);
                if files.is_err() {
                    log::debug!("Failed to load directory {}: {}", path, files.unwrap_err());
                    continue;
                }
                let files = files.unwrap();

                // Look at each file in the directory and try to load them
                for file in files {
                    if file.is_err() {
                        log::debug!("Failed read directory entry: {}", file.unwrap_err());
                        continue;
                    }
                    let file = file.unwrap();
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

        task.await.unwrap_or_default()
    }

    /// Creates a DBus object
    async fn listen_on_dbus(&self) -> Result<(), Box<dyn Error>> {
        let iface = DBusInterface::new(self.tx.clone());
        let manager_path = format!("{}/Manager", BUS_PREFIX);
        self.dbus.object_server().at(manager_path, iface).await?;
        Ok(())
    }
}
