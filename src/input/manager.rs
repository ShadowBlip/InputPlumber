use std::collections::HashMap;
use std::error::Error;

use tokio::fs;
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use zbus::fdo;
use zbus::zvariant::ObjectPath;
use zbus::Connection;
use zbus_macros::dbus_interface;

use crate::config::CompositeDeviceConfig;
use crate::constants::BUS_PREFIX;
use crate::input::composite_device;
use crate::input::composite_device::CompositeDevice;
use crate::input::source;
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
    EventDeviceAdded { name: String },
    EventDeviceRemoved { name: String },
    HIDRawAdded { name: String },
    HIDRawRemoved { name: String },
    CreateCompositeDevice { config: CompositeDeviceConfig },
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
    /// The transmit side of the [rx] channel used to send [Command] messages.
    /// This can be cloned to allow child objects to communicate up to the
    /// manager.
    tx: broadcast::Sender<Command>,
    /// The receive side of the channel used to listen for [Command] messages
    /// from other objects.
    rx: broadcast::Receiver<Command>,
    /// Mapping of all currently managed source devices.
    /// E.g. {"/org/shadowblip/InputPlumber/devices/source/event0": <SourceDevice>}
    source_devices: HashMap<String, source::SourceDevice>,
    composite_devices: HashMap<String, composite_device::Handle>,
}

impl Manager {
    /// Returns a new instance of Gamepad Manager
    pub fn new(conn: Connection) -> Manager {
        let (tx, rx) = broadcast::channel(BUFFER_SIZE);
        Manager {
            dbus: conn,
            rx,
            tx,
            source_devices: HashMap::new(),
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
                Command::HIDRawRemoved { name } => self.on_hidraw_removed(name).await,
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
            }
        }

        Ok(())
    }

    /// Create a new [CompositeDevice] from the given [CompositeDeviceConfig]
    async fn create_composite_device(
        &mut self,
        config: CompositeDeviceConfig,
    ) -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    /// Called when any source device is added
    async fn on_source_device_added(&mut self) -> Result<(), Box<dyn Error>> {
        // Check all CompositeDevice configs to see if this device creates
        // a match that will automatically create a CompositeDevice.
        let configs = self.load_device_configs().await;
        log::debug!("Checking configs");
        for config in configs {
            log::debug!("Got config: {:?}", config.name);

            // TODO: Check if DMI matches

            // Skip configs where the required source devices don't exist.
            if !config.sources_exist()? {
                continue;
            }

            log::info!("Found matching source!: {:?}", config.name);

            // TODO: Check to see if there's already a CompositeDevice for
            // these source devices

            // Create a composite device to manage these devices
            log::info!("Creating composite device");
            let device = CompositeDevice::new(config);

            // Create a DBus interface for the device
            let path = self.next_composite_dbus_path();
            device
                .listen_on_dbus(self.dbus.clone(), path.clone())
                .await?;

            // Get a handle to the device
            let handle = device.handle();

            // Run the device
            tokio::spawn(async move {
                if let Err(e) = device.run().await {
                    log::error!("Error running device: {:?}", e);
                }
            });

            // Add the device to our map
            self.composite_devices.insert(path, handle);
        }

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
            return Err("Failed to find device information".into());
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

        Ok(())
    }

    /// Called when an event device (e.g. /dev/input/event5) is removed
    async fn on_event_device_removed(&mut self, name: String) -> Result<(), Box<dyn Error>> {
        log::debug!("Event device removed: {}", name);

        // Remove the device from our hashmap
        let path = source::evdev::get_dbus_path(name);
        self.source_devices.remove(&path);

        // Remove the DBus interface
        let path = ObjectPath::from_string_unchecked(path.clone());
        self.dbus
            .object_server()
            .remove::<source::evdev::DBusInterface, ObjectPath>(path.clone())
            .await?;

        Ok(())
    }

    /// Called when a hidraw device (e.g. /dev/hidraw0) is added
    async fn on_hidraw_added(&self, name: String) -> Result<(), Box<dyn Error>> {
        log::debug!("HIDRaw added: {}", name);

        // Signal that a source device was added
        self.tx.send(Command::SourceDeviceAdded)?;

        Ok(())
    }

    /// Called when a hidraw device (e.g. /dev/hidraw0) is removed
    async fn on_hidraw_removed(&self, name: String) {
        log::debug!("HIDRaw removed: {}", name);
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
        let mut devices: Vec<CompositeDeviceConfig> = Vec::new();
        let paths = vec![
            "/usr/share/inputplumber/devices",
            "/etc/inputplumber/devices.d",
            "./rootfs/usr/share/inputplumber/devices",
        ];

        // Look for composite device profiles in all known locations
        for path in paths {
            let files = fs::read_dir(path).await;
            if files.is_err() {
                log::debug!("Failed to load directory {}: {}", path, files.unwrap_err());
                continue;
            }
            let mut files = files.unwrap();

            // Look at each file in the directory and try to load them
            while let Ok(Some(file)) = files.next_entry().await {
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
    }

    /// Creates a DBus object
    async fn listen_on_dbus(&self) -> Result<(), Box<dyn Error>> {
        let iface = DBusInterface::new(self.tx.clone());
        let manager_path = format!("{}/Manager", BUS_PREFIX);
        self.dbus.object_server().at(manager_path, iface).await?;
        Ok(())
    }
}
