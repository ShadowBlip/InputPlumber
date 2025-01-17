use core::panic;
use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::time::Duration;

use ::procfs::CpuInfo;
use ::udev::MonitorBuilder;
use mio::{Events, Interest, Poll, Token};
use thiserror::Error;
use tokio::sync::mpsc;
use tokio::task;
use zbus::fdo::ManagedObjects;
use zbus::zvariant::ObjectPath;
use zbus::Connection;

use crate::bluetooth::device1::Device1Proxy;
use crate::config::path::get_capability_maps_paths;
use crate::config::path::get_devices_paths;
use crate::config::CapabilityMap;
use crate::config::CompositeDeviceConfig;
use crate::config::SourceDevice;
use crate::constants::BUS_PREFIX;
use crate::constants::BUS_SOURCES_PREFIX;
use crate::constants::BUS_TARGETS_PREFIX;
use crate::dbus::interface::composite_device::CompositeDeviceInterface;
use crate::dbus::interface::manager::ManagerInterface;
use crate::dbus::interface::source::evdev::SourceEventDeviceInterface;
use crate::dbus::interface::source::hidraw::SourceHIDRawInterface;
use crate::dbus::interface::source::iio_imu::SourceIioImuInterface;
use crate::dbus::interface::source::udev::SourceUdevDeviceInterface;
use crate::dmi::data::DMIData;
use crate::dmi::get_cpu_info;
use crate::dmi::get_dmi_data;
use crate::input::composite_device::CompositeDevice;
use crate::input::source::evdev;
use crate::input::source::hidraw;
use crate::input::source::iio;
use crate::input::target::TargetDevice;
use crate::input::target::TargetDeviceTypeId;
use crate::udev;
use crate::udev::device::AttributeGetter;
use crate::udev::device::UdevDevice;

use super::composite_device::client::CompositeDeviceClient;
use super::target::client::TargetDeviceClient;

use crate::watcher;
use crate::watcher::WatchEvent;

const DEV_PATH: &str = "/dev";
const INPUT_PATH: &str = "/dev/input";
const BUFFER_SIZE: usize = 20480;

#[derive(Error, Debug)]
pub enum ManagerError {
    #[error("failed to create target device")]
    CreateTargetDeviceFailed(String),
    #[error("failed to attach target device")]
    AttachTargetDeviceFailed(String),
}

/// Manager commands define all the different ways to interact with [Manager]
/// over a channel. These commands are processed in an asyncronous thread and
/// dispatched as they come in.
#[derive(Debug, Clone)]
pub enum ManagerCommand {
    DeviceAdded {
        device: UdevDevice,
    },
    DeviceRemoved {
        device: UdevDevice,
    },
    CreateCompositeDevice {
        config: CompositeDeviceConfig,
    },
    CreateTargetDevice {
        kind: String,
        sender: mpsc::Sender<Result<String, ManagerError>>,
    },
    StopTargetDevice {
        path: String,
    },
    AttachTargetDevice {
        target_path: String,
        composite_path: String,
        sender: mpsc::Sender<Result<(), ManagerError>>,
    },
    TargetDeviceStopped {
        path: String,
    },
    CompositeDeviceStopped(String),
    GetManageAllDevices {
        sender: mpsc::Sender<bool>,
    },
    SetManageAllDevices(bool),
    SystemSleep {
        sender: mpsc::Sender<()>,
    },
    SystemWake {
        sender: mpsc::Sender<()>,
    },
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
    /// System CPU info
    cpu_info: CpuInfo,
    /// The transmit side of the [rx] channel used to send [Command] messages.
    /// This can be cloned to allow child objects to communicate up to the
    /// manager.
    tx: mpsc::Sender<ManagerCommand>,
    /// The receive side of the channel used to listen for [Command] messages
    /// from other objects.
    rx: mpsc::Receiver<ManagerCommand>,
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
    composite_devices: HashMap<String, CompositeDeviceClient>,
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
    target_devices: HashMap<String, TargetDeviceClient>,
    /// Defines whether or not InputPlumber should try to automatically manage all
    /// input devices that have a [CompositeDeviceConfig] definition
    manage_all_devices: bool,
}

impl Manager {
    /// Returns a new instance of Gamepad Manager
    pub fn new(conn: Connection) -> Manager {
        let (tx, rx) = mpsc::channel(BUFFER_SIZE);

        log::debug!("Loading DMI data");
        let dmi_data = get_dmi_data();
        log::debug!("Got DMI data: {:?}", dmi_data);

        log::debug!("Loading CPU info");
        let cpu_info = match get_cpu_info() {
            Ok(info) => info,
            Err(e) => {
                log::error!("Failed to get CPU info: {e:?}");
                panic!("Unable to determine CPU info!");
            }
        };
        log::debug!("Got CPU info: {cpu_info:?}");

        Manager {
            dbus: conn,
            dmi_data,
            cpu_info,
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
            manage_all_devices: false,
        }
    }

    /// Starts listening for [Command] messages to be sent from clients and
    /// dispatch those events.
    pub async fn run(&mut self) -> Result<(), Box<dyn Error + Send + Sync>> {
        let dbus_for_listen_on_dbus = self.dbus.clone();

        let cmd_tx_all_devices = self.tx.clone();

        // Watch for hidraw/evdev inotify events.
        // TODO: when we reload the udev device it triggers the udev watcher. We do this to break
        // access to the file descriptor for processes that have already authenticated. Figure out
        // a way to do this only using the udev events.
        let (watcher_tx, mut watcher_rx) = mpsc::channel(BUFFER_SIZE);
        if std::path::Path::new(DEV_PATH).exists() {
            let tx = watcher_tx.clone();
            tokio::task::spawn_blocking(move || {
                log::info!("Started hidraw device discovery thread");
                watcher::watch(DEV_PATH.into(), tx)
            });
        }
        if std::path::Path::new(INPUT_PATH).exists() {
            let tx = watcher_tx.clone();
            tokio::task::spawn_blocking(move || {
                log::info!("Started evdev device discovery thread");
                watcher::watch(INPUT_PATH.into(), tx)
            });
        }

        log::debug!("Starting input manager task...");

        let _ = tokio::join!(
            Self::discover_all_devices(&cmd_tx_all_devices),
            Self::watch_iio_devices(self.tx.clone()),
            Self::watch_devnodes(self.tx.clone(), &mut watcher_rx),
            Self::listen_on_dbus(dbus_for_listen_on_dbus, self.tx.clone()),
            self.events_loop()
        );

        Ok(())
    }

    /// Manage events generated by various components
    async fn events_loop(&mut self) -> Result<(), Box<dyn Error>> {
        // Loop and listen for command events
        while let Some(cmd) = self.rx.recv().await {
            log::debug!("Received command: {:?}", cmd);
            match cmd {
                ManagerCommand::CreateCompositeDevice { config } => {
                    if let Err(e) = self.create_composite_device(config).await {
                        log::error!("Error creating composite device: {:?}", e);
                    }
                }
                ManagerCommand::CompositeDeviceStopped(path) => {
                    if let Err(e) = self.on_composite_device_stopped(path).await {
                        log::error!("Error handling stopped composite device: {:?}", e);
                    }
                }
                ManagerCommand::CreateTargetDevice { kind, sender } => {
                    // Create the target device
                    log::debug!("Got request to create target device: {kind}");
                    let device = match self.create_and_start_target_device(kind.as_str()).await {
                        Ok(device) => device,
                        Err(err) => {
                            if let Err(e) = sender.send(Err(err)).await {
                                log::error!("Failed to send response: {e:?}");
                            }
                            continue;
                        }
                    };
                    log::debug!("Created target device: {kind}");

                    // Get the DBus path to the target device
                    let path = device.keys().next().cloned();
                    let response = match path {
                        Some(path) => Ok(path),
                        None => Err(ManagerError::CreateTargetDeviceFailed(
                            "Unable to find device path".to_string(),
                        )),
                    };

                    if let Err(e) = sender.send(response).await {
                        log::error!("Failed to send response: {e:?}");
                    }
                    log::debug!("Finished creating target device: {kind}");
                }
                ManagerCommand::AttachTargetDevice {
                    target_path,
                    composite_path,
                    sender,
                } => {
                    log::debug!("Got request to attach target device {target_path} to device: {composite_path}");
                    let Some(target) = self.target_devices.get(&target_path) else {
                        let err = ManagerError::AttachTargetDeviceFailed(
                            "Failed to find target device".into(),
                        );
                        log::error!("{err}");
                        if let Err(e) = sender.send(Err(err)).await {
                            log::error!("Failed to send response: {e:?}");
                        }
                        continue;
                    };
                    let Some(device) = self.composite_devices.get(&composite_path) else {
                        let err = ManagerError::AttachTargetDeviceFailed(
                            "Failed to find composite device".into(),
                        );
                        log::error!("{err}");
                        if let Err(e) = sender.send(Err(err)).await {
                            log::error!("Failed to send response: {e:?}");
                        }
                        continue;
                    };

                    // Send the attach command to the composite device
                    let mut targets = HashMap::new();
                    targets.insert(target_path.clone(), target.clone());
                    if let Err(e) = device.attach_target_devices(targets).await {
                        log::error!("Failed to send attach command: {e:?}");
                    }
                    log::debug!("Finished handling attach request for: {target_path}");
                }
                ManagerCommand::StopTargetDevice { path } => {
                    log::debug!("Got request to stop target device: {path}");
                    let Some(target) = self.target_devices.get(&path) else {
                        log::error!("Failed to find target device: {path}");
                        continue;
                    };
                    if let Err(e) = target.stop().await {
                        log::error!("Failed to send stop command to target device {path}: {e:?}");
                    }
                    log::debug!("Finished handling stop target device request for {path}");
                }
                ManagerCommand::TargetDeviceStopped { path } => {
                    log::debug!("Target device stopped: {path}");
                    self.target_devices.remove(&path);
                }
                ManagerCommand::DeviceAdded { device } => {
                    let dev_name = device.name();
                    let dev_sysname = device.sysname();

                    if let Err(e) = self.on_device_added(device).await {
                        log::error!("Error adding device '{dev_name} ({dev_sysname})': {e}");
                    }
                }
                ManagerCommand::DeviceRemoved { device } => {
                    if let Err(e) = self.on_device_removed(device).await {
                        log::error!("Error removing device: {e}");
                    }
                }
                ManagerCommand::SetManageAllDevices(manage_all_devices) => {
                    log::debug!("Setting management of all devices to: {manage_all_devices}");
                    if self.manage_all_devices == manage_all_devices {
                        continue;
                    }
                    self.manage_all_devices = manage_all_devices;

                    // If management of all devices was enabled, trigger device discovery
                    if manage_all_devices {
                        let cmd_tx = self.tx.clone();
                        tokio::task::spawn(async move {
                            if let Err(e) = Manager::discover_all_devices(&cmd_tx).await {
                                log::error!("Failed to trigger device discovery: {e:?}");
                            }
                        });
                        continue;
                    }

                    // If management was disabled, stop any composite devices that
                    // are not auto-managed.
                    for (dbus_path, config) in self.used_configs.iter() {
                        if let Some(options) = config.options.as_ref() {
                            let auto_managed = options.auto_manage.unwrap_or(false);
                            if auto_managed {
                                continue;
                            }
                        }

                        log::debug!("Found composite device that should not be managed anymore: {dbus_path}");
                        let Some(device) = self.composite_devices.get(dbus_path) else {
                            continue;
                        };
                        if let Err(e) = device.stop().await {
                            log::error!("Failed to stop composite device: {e:?}");
                        }
                    }
                }
                ManagerCommand::GetManageAllDevices { sender } => {
                    if let Err(e) = sender.send(self.manage_all_devices).await {
                        log::error!("Failed to send response: {e:?}");
                    }
                }
                ManagerCommand::SystemSleep { sender } => {
                    log::info!("Preparing for system suspend");

                    // Call the suspend handler on each composite device and wait
                    // for a response.
                    let composite_devices = self.composite_devices.clone();
                    tokio::task::spawn(async move {
                        for device in composite_devices.values() {
                            if let Err(e) = device.suspend().await {
                                log::error!("Failed to call suspend handler on device: {e:?}");
                            }
                        }

                        // Respond to the sender to inform them that suspend tasks
                        // have completed.
                        if let Err(e) = sender.send(()).await {
                            log::error!("Failed to send response: {e:?}");
                        }

                        log::info!("Finished preparing for system suspend");
                    });
                }
                ManagerCommand::SystemWake { sender } => {
                    log::info!("Preparing for system resume");

                    // Call the resume handler on each composite device and wait
                    // for a response.
                    let composite_devices = self.composite_devices.clone();
                    tokio::task::spawn(async move {
                        for device in composite_devices.values() {
                            if let Err(e) = device.resume().await {
                                log::error!("Failed to call resume handler on device: {e:?}");
                            }
                        }

                        // Respond to the sender to inform them that resume tasks
                        // have completed.
                        if let Err(e) = sender.send(()).await {
                            log::error!("Failed to send response: {e:?}");
                        }

                        log::info!("Finished preparing for system resume");
                    });
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
        device: UdevDevice,
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
        log::info!("Found matching source device for: {:?}", config.name);
        let config = config.clone();
        let device = CompositeDevice::new(
            self.dbus.clone(),
            self.tx.clone(),
            config,
            device,
            capability_map,
        )?;

        // Check to see if there's already a CompositeDevice for
        // these source devices.
        // TODO: Should we allow multiple composite devices with the same source?
        let source_device_ids = device.get_source_devices_used();
        for (id, path) in self.source_devices_used.iter() {
            if !source_device_ids.contains(id) {
                continue;
            }
            return Err(format!("Source device '{id}' already in use by {path}").into());
        }

        Ok(device)
    }

    /// Create target input device to emulate based on the given device type.
    async fn create_target_device(&mut self, kind: &str) -> Result<TargetDevice, Box<dyn Error>> {
        log::trace!("Creating target device: {kind}");
        let Ok(target_id) = TargetDeviceTypeId::try_from(kind) else {
            return Err("Invalid target device ID".to_string().into());
        };

        // Create the target device to emulate based on the kind
        let device = TargetDevice::from_type_id(target_id, self.dbus.clone())?;

        Ok(device)
    }

    /// Start and run the given target devices. Returns a HashMap of transmitters
    /// to send events to the given targets.
    async fn start_target_devices(
        &mut self,
        targets: Vec<TargetDevice>,
    ) -> Result<HashMap<String, TargetDeviceClient>, Box<dyn Error>> {
        let mut target_devices = HashMap::new();
        for target in targets {
            // Get the target device class to determine the DBus path to use for
            // the device.
            let device_class = target.dbus_device_class();
            let path = self.next_target_path(device_class)?;

            // Get a client reference to communicate with the target device
            let Some(client) = target.client() else {
                log::trace!("No client implemented for target device");
                continue;
            };
            target_devices.insert(path.clone(), client.clone());
            self.target_devices.insert(path.clone(), client.clone());

            // Run the target device
            tokio::spawn(async move {
                if let Err(e) = target.run(path.clone()).await {
                    log::error!("Failed to run target device {path}: {e:?}");
                }
                log::debug!("Target device closed at: {path}");
            });
        }

        // Spawn tasks to cleanup target devices
        for (path, target) in target_devices.iter() {
            let tx = self.tx.clone();
            let path = path.clone();
            let target = target.clone();
            task::spawn(async move {
                target.closed().await;
                if let Err(e) = tx.send(ManagerCommand::TargetDeviceStopped { path }).await {
                    log::error!("Failed to target device stopped: {e:?}");
                }
            });
        }

        Ok(target_devices)
    }

    /// Create and start the given type of target device and return a mapping
    /// of the dbus path to the target device and sender to send messages to the
    /// device.
    async fn create_and_start_target_device(
        &mut self,
        kind: &str,
    ) -> Result<HashMap<String, TargetDeviceClient>, ManagerError> {
        // Create the target device
        let device = match self.create_target_device(kind).await {
            Ok(device) => device,
            Err(e) => {
                let err = format!("Error creating target device: {e:?}");
                log::error!("{err}");
                return Err(ManagerError::CreateTargetDeviceFailed(err));
            }
        };

        // Start the target device
        let paths = match self.start_target_devices(vec![device]).await {
            Ok(paths) => paths,
            Err(e) => {
                let err = format!("Error starting target device: {e:?}");
                log::error!("{err}");
                return Err(ManagerError::CreateTargetDeviceFailed(err));
            }
        };

        Ok(paths)
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
        log::debug!(
            "Starting CompositeDevice at {path} with the following sources: {source_device_ids:?}"
        );
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
        let client = device.client();

        // Keep track of target devices that this composite device is using
        let mut target_device_paths = Vec::new();

        // Create a DBus target device
        log::debug!("Creating target devices for {path}");
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
                log::error!("Error running {dbus_path}: {e}");
            }
            log::debug!("Composite device stopped running: {dbus_path}");
            if let Err(e) = tx
                .send(ManagerCommand::CompositeDeviceStopped(dbus_path))
                .await
            {
                log::error!("Error sending composite device stopped: {e}");
            }
        });
        let comp_path = path.clone();

        // Add the device to our maps
        self.composite_devices.insert(comp_path, client);
        log::trace!("Managed source devices: {:?}", self.source_devices_used);
        self.used_configs.insert(path, config);
        log::trace!("Used configs: {:?}", self.used_configs);
        self.composite_device_targets
            .insert(composite_path.clone(), target_device_paths);
        log::trace!("Used target devices: {:?}", self.composite_device_targets);

        Ok(())
    }

    /// Called when a composite device stops running
    async fn on_composite_device_stopped(&mut self, path: String) -> Result<(), Box<dyn Error>> {
        log::debug!("Removing composite device: {}", path);

        // Remove the DBus interface
        let dbus_path = ObjectPath::from_string_unchecked(path.clone());
        let conn = self.dbus.clone();
        task::spawn(async move {
            log::debug!("Stopping dbus interface: {dbus_path}");
            let result = conn
                .object_server()
                .remove::<CompositeDeviceInterface, ObjectPath>(dbus_path.clone())
                .await;
            if let Err(e) = result {
                log::error!("Failed to remove dbus interface {dbus_path}: {e:?}");
            } else {
                log::debug!("Stopped dbus interface: {dbus_path}");
            }
        });

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
        device: UdevDevice,
    ) -> Result<(), Box<dyn Error>> {
        // Check all existing composite devices to see if this device is part of
        // their config
        'start: for composite_device in self.composite_devices.keys() {
            let Some(config) = self.used_configs.get(composite_device) else {
                continue;
            };
            log::debug!("Checking if existing composite device {composite_device:?} with config {:?} is missing device: {id:?}", config.name);

            // If the CompositeDevice only allows a single source device, skip its
            // consideration.
            if config.single_source.unwrap_or(false) {
                log::trace!("{:?} is a single source device. Skipping.", config.name);
                continue;
            }
            if config.maximum_sources.unwrap_or(0) == 1 {
                log::trace!("{:?} is a single source device. Skipping.", config.name);
                continue;
            }
            log::trace!(
                "Composite device has {} source devices defined",
                config.source_devices.len()
            );

            // If the CompositeDevice only allows a maximum number of source devices,
            // check to see if that limit has been reached. If that limit is reached,
            // then a new CompositeDevice will be created for the source device.
            // If maximum_sources is less than 1 (e.g. 0, -1) then consider
            // the maximum to be 'unlimited'.
            if let Some(max_sources) = config
                .maximum_sources
                .filter(|max_sources| *max_sources > 0)
            {
                // Check to see how many source devices this composite device is
                // currently managing.
                if self
                    .composite_device_sources
                    .get(composite_device)
                    .map_or(false, |sources| (sources.len() as i32) >= max_sources)
                {
                    log::trace!(
                        "{composite_device:?} maximum source devices reached: {max_sources}. Skipping."
                    );
                    continue;
                }
            }

            // Check if this device matches any source udev configs of the running
            // CompositeDevice.
            let Some(source_device) = config.get_matching_device(&device) else {
                log::trace!(
                    "Device {id} does not match existing device: {:?}",
                    config.name
                );

                continue;
            };

            // Check if the device has already been used in this config or not,
            // stop here if the device must be unique.
            if let Some(sources) = self.composite_device_sources.get(composite_device) {
                for source in sources {
                    if *source != source_device {
                        continue;
                    }

                    if source_device.ignore.map_or(false, |ignored| ignored) {
                        log::debug!(
                            "Ignoring device {:?}, not adding to composite device: {composite_device}",
                            source_device
                        );
                        break 'start;
                    }

                    // Check if the composite device has to be unique (default to being unique)
                    if source_device.unique.map_or(true, |unique| unique) {
                        log::trace!(
                            "Found unique device {:?}, not adding to composite device {composite_device}",
                            source_device
                        );
                        break 'start;
                    }
                }
            }

            log::info!("Found missing {} device, adding source device {id} to existing composite device: {composite_device:?}", device.subsystem());
            let Some(client) = self.composite_devices.get(composite_device.as_str()) else {
                log::error!("No existing composite device found for key {composite_device:?}");
                continue;
            };

            self.add_device_to_composite_device(device, client).await?;
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

        log::debug!("No existing composite device matches device {id}.");

        // Check all CompositeDevice configs to see if this device creates
        // a match that will automatically create a CompositeDevice.
        let configs = self.load_device_configs().await;
        log::debug!("Checking unused configs");
        for config in configs {
            log::trace!("Checking config {:?} for device", config.name);

            // Check to see if 'auto_manage' is enabled for this config.
            let auto_manage = config
                .options
                .as_ref()
                .and_then(|options| Some(options.auto_manage.unwrap_or(false)))
                .unwrap_or(false);
            if !self.manage_all_devices && !auto_manage {
                log::trace!(
                    "Config {:?} does not have 'auto_manage' option enabled. Skipping.",
                    config.name
                );
                continue;
            }

            // Check to see if this configuration matches the system
            if !config.has_valid_matches(&self.dmi_data, &self.cpu_info) {
                log::trace!("Configuration does not match system");
                continue;
            }

            // Check if this device matches any source configs
            if let Some(source_device) = config.get_matching_device(&device) {
                if let Some(ignored) = source_device.ignore {
                    if ignored {
                        log::trace!("Event device configured to ignore: {:?}", device);
                        return Ok(());
                    }
                }
                log::info!(
                    "Found a matching {} device {id}, creating CompositeDevice",
                    device.subsystem()
                );
                let dev = self
                    .create_composite_device_from_config(&config, device)
                    .await?;

                // Get the target input devices from the config
                let target_devices_config = config.target_devices.clone();

                // Create the composite deivce
                self.start_composite_device(
                    dev,
                    config.clone(),
                    target_devices_config,
                    source_device.clone(),
                )
                .await?;

                return Ok(());
            }

            log::trace!("Device does not match config: {:?}", config.name);
        }
        log::debug!("No unused configs found for device.");

        Ok(())
    }

    /// Called when any source device is removed
    async fn on_source_device_removed(
        &mut self,
        device: UdevDevice,
        id: String,
    ) -> Result<(), Box<dyn Error>> {
        let dev_name = device.name();
        log::debug!("Source device removed: {dev_name}");
        let Some(composite_device_path) = self.source_devices_used.get(&id) else {
            log::debug!("Source device not being managed by a composite device");
            return Ok(());
        };

        let Some(client) = self.composite_devices.get(composite_device_path) else {
            return Err(format!("CompostiteDevice {} not found", composite_device_path).into());
        };

        client.remove_source_device(device).await?;

        let Some(device) = self.source_devices.get(&id) else {
            return Err(format!("Device {} not found in source devices", id).into());
        };

        let Some(sources) = self.composite_device_sources.get_mut(composite_device_path) else {
            return Err(format!("CompostiteDevice {} not found", composite_device_path).into());
        };

        let idx = sources.iter().position(|item| item == device);
        if idx.is_none() {
            self.source_devices.remove(&id);
            return Err(format!("Device {} not found in composite device sources", id).into());
        }
        sources.remove(idx.unwrap());
        self.source_devices.remove(&id);
        self.source_device_dbus_paths.remove(&id);
        self.source_devices_used.remove(&id);

        Ok(())
    }

    /// Called when a new device is detected by udev
    async fn on_device_added(&mut self, device: UdevDevice) -> Result<(), Box<dyn Error>> {
        let dev_path = device.devpath();
        let dev_name = device.name();
        let dev_sysname = device.sysname();
        let sys_name = device.sysname();
        if sys_name.is_empty() {
            log::debug!("Device discarded for missing sysname: {dev_name} at {dev_path}");
            return Ok(());
        }
        let sysname = sys_name.clone();
        let dev = device.clone();

        log::debug!("Device added: {dev_name} ({dev_sysname}): {dev_path}");

        // Get the device subsystem
        let subsystem = device.subsystem();

        // Get the device id
        let id = device.get_id();

        // Create a DBus interface depending on the device subsystem
        match subsystem.as_str() {
            "input" => {
                if device.devnode().is_empty() {
                    log::debug!("Event device discarded for missing devnode: {dev_name} ({dev_sysname}) at {dev_path}");
                    return Ok(());
                }

                log::debug!("Event device added: {dev_name} ({dev_sysname})");

                // Create a DBus interface for the event device
                let conn = self.dbus.clone();
                let path = evdev::get_dbus_path(sys_name.clone());
                log::debug!(
                    "Attempting to listen on dbus for {dev_path} | {dev_name} ({dev_sysname})"
                );

                let dbus_path = path.clone();
                task::spawn(async move {
                    let result = SourceUdevDeviceInterface::listen_on_dbus(
                        conn.clone(),
                        dbus_path.as_str(),
                        sysname.as_str(),
                        dev.clone(),
                    )
                    .await;
                    if let Err(e) = result {
                        log::error!("Error creating source udev dbus interface: {e:?}");
                    }
                    let result =
                        SourceEventDeviceInterface::listen_on_dbus(conn, sysname, dev).await;
                    if let Err(e) = result {
                        log::error!("Error creating source evdev dbus interface: {e:?}");
                    }
                    log::debug!("Finished adding source device on dbus");
                });

                // Add the device as a source device
                self.source_device_dbus_paths.insert(id.clone(), path);

                // Check to see if the device is virtual
                if device.is_virtual() {
                    // Look up the connected device using udev
                    let device_info = udev::get_device(dev_path.clone()).await?;

                    // Check if the virtual device is using the bluetooth bus
                    // TODO: Can we get properties from UdevDevice::get_attribute_from_tree?
                    let id_bus = device_info.properties.get("ID_BUS");

                    log::debug!("Bus ID for {dev_path}: {id_bus:?}");
                    let is_bluetooth = {
                        if let Some(bus) = id_bus {
                            bus == "bluetooth"
                        } else {
                            false
                        }
                    };

                    if !is_bluetooth {
                        log::debug!("{dev_name} ({dev_sysname}) is virtual, skipping consideration for {dev_path}");
                        return Ok(());
                    }
                    log::debug!("{dev_name} ({dev_sysname}) is a virtual device node for a bluetooth device. Treating as real - {dev_path}")
                } else {
                    log::trace!("{dev_name} ({dev_sysname}) is a real device - {dev_path}");
                }

                // Signal that a source device was added
                log::debug!("Spawning task to add source device: {id}");
                self.on_source_device_added(id.clone(), device).await?;
                log::debug!("Finished adding {id}");
            }
            "hidraw" => {
                if device.devnode().is_empty() {
                    log::debug!("hidraw device discarded for missing devnode: {dev_name} ({dev_sysname}) at {dev_path}");
                    return Ok(());
                }

                log::debug!("hidraw device added: {dev_name} ({dev_sysname})");

                // Create a DBus interface for the event device
                let conn = self.dbus.clone();
                let path = hidraw::get_dbus_path(sys_name.clone());

                log::debug!("Attempting to listen on dbus for {dev_path} | {dev_sysname}");
                let dbus_path = path.clone();
                task::spawn(async move {
                    let result = SourceUdevDeviceInterface::listen_on_dbus(
                        conn.clone(),
                        dbus_path.as_str(),
                        sysname.as_str(),
                        dev.clone(),
                    )
                    .await;
                    if let Err(e) = result {
                        log::error!("Error creating source udev dbus interface: {e:?}");
                    }
                    let result = SourceHIDRawInterface::listen_on_dbus(conn, sysname, dev).await;
                    if let Err(e) = result {
                        log::error!("Error creating source evdev dbus interface: {e:?}");
                    }
                    log::debug!("Finished adding source device on dbus");
                });

                // Add the device as a source device
                self.source_device_dbus_paths.insert(id.clone(), path);

                // Check to see if the device is virtual
                if device.is_virtual() {
                    // Check to see if this virtual device is a bluetooth device
                    let uniq = device.uniq();
                    if uniq.is_empty() {
                        log::debug!("{dev_name} ({dev_sysname}) is virtual, skipping consideration for {dev_path}.");
                        return Ok(());
                    };

                    // Check bluez to see if that uniq is a bluetooth device
                    let object_manager = zbus::fdo::ObjectManagerProxy::builder(&self.dbus)
                        .destination("org.bluez")?
                        .path("/")?
                        .build()
                        .await?;
                    let objects: ManagedObjects = object_manager.get_managed_objects().await?;

                    // Check each dbus object for a connected device
                    let mut matches_bluetooth = false;
                    for (path, obj) in objects.iter() {
                        // Only consider device objects
                        if !obj.contains_key("org.bluez.Device1") {
                            log::trace!("{path} does not have org.bluez.Device1 interface");
                            continue;
                        }

                        // Get a reference to the device
                        let bt_device = Device1Proxy::builder(&self.dbus)
                            .destination("org.bluez")?
                            .path(path)?
                            .build()
                            .await?;

                        // Only consider connected bluetooth devices
                        if !bt_device.connected().await? {
                            continue;
                        }

                        // Check to see if the 'uniq' field matches the bluetooth addr
                        let address = bt_device.address().await?;
                        log::debug!(
                            "Checking if virtual device {uniq} is bluetooth device: {address}"
                        );
                        if uniq.to_lowercase() == address.to_lowercase() {
                            matches_bluetooth = true;
                            break;
                        }
                    }

                    if !matches_bluetooth {
                        log::debug!("{dev_name} ({dev_sysname}) is virtual, skipping consideration for {dev_path}.");
                        return Ok(());
                    }
                    log::debug!("{dev_name} ({dev_sysname}) is a virtual device node for a bluetooth device. Treating as real - {dev_path}");
                } else {
                    log::trace!("{dev_name} ({dev_sysname})  is a real device -{dev_path}");
                }

                // Signal that a source device was added
                log::debug!("Spawing task to add source device: {id}");
                self.on_source_device_added(id.clone(), device).await?;
                log::debug!("Finished adding hidraw device {id}");
            }

            "iio" => {
                if device.devnode().is_empty() {
                    log::warn!("iio device discarded for missing devnode: {dev_name} ({dev_sysname}) at {dev_path}");
                    return Ok(());
                }

                log::debug!("iio device added: {} ({})", device.name(), device.sysname());

                // Create a DBus interface for the event device
                let conn = self.dbus.clone();
                let path = iio::get_dbus_path(sys_name.clone());

                log::debug!("Attempting to listen on dbus for device {dev_name} ({dev_sysname}) | {dev_path}");
                let dbus_path = path.clone();
                task::spawn(async move {
                    let result = SourceUdevDeviceInterface::listen_on_dbus(
                        conn.clone(),
                        dbus_path.as_str(),
                        sysname.as_str(),
                        dev.clone(),
                    )
                    .await;
                    if let Err(e) = result {
                        log::error!("Error creating source udev dbus interface: {e:?}");
                    }

                    let result = SourceIioImuInterface::listen_on_dbus(conn, dev).await;
                    if let Err(e) = result {
                        log::error!("Error creating source evdev dbus interface: {e:?}");
                    }
                    log::debug!("Finished adding source device on dbus");
                });

                // Add the device as a source device
                self.source_device_dbus_paths.insert(id.clone(), path);

                // Check to see if the device is virtual
                if device.is_virtual() {
                    log::debug!("{dev_name} ({dev_sysname}) is virtual, skipping consideration for {dev_path}");
                    return Ok(());
                } else {
                    log::trace!("Device {dev_name} ({dev_sysname}) is real - {dev_path}");
                }

                // Signal that a source device was added
                log::debug!("Spawing task to add source device: {id}");
                self.on_source_device_added(id.clone(), device).await?;
                log::debug!("Finished adding event device {id}");
            }

            _ => {
                return Err(format!("Device subsystem not supported: {subsystem:?}").into());
            }
        };

        Ok(())
    }

    async fn on_device_removed(&mut self, device: UdevDevice) -> Result<(), Box<dyn Error>> {
        let dev_name = device.name();
        let sys_name = device.sysname();
        let subsystem = device.subsystem();
        log::debug!("Device removed: {dev_name} ({sys_name})");
        let path = ObjectPath::from_string_unchecked(format!("{BUS_SOURCES_PREFIX}/{sys_name}"));
        log::debug!("Device dbus path: {path}");
        let conn = self.dbus.clone();
        task::spawn(async move {
            log::debug!("Stopping dbus interfaces: {path}");

            // Stop generic interfaces
            let result = conn
                .object_server()
                .remove::<SourceUdevDeviceInterface, ObjectPath>(path.clone())
                .await;
            if let Err(e) = result {
                log::error!("Failed to remove udev dbus interface {path}: {e:?}");
            } else {
                log::debug!("Stopped udev dbus interface: {path}");
            }

            // Stop subsystem-specific interfaces
            let result = match subsystem.as_str() {
                "input" => {
                    conn.object_server()
                        .remove::<SourceEventDeviceInterface, ObjectPath>(path.clone())
                        .await
                }
                "hidraw" => {
                    conn.object_server()
                        .remove::<SourceHIDRawInterface, ObjectPath>(path.clone())
                        .await
                }
                "iio" => {
                    conn.object_server()
                        .remove::<SourceIioImuInterface, ObjectPath>(path.clone())
                        .await
                }
                _ => Err(zbus::Error::Failure(format!(
                    "Invalid subsystem: '{subsystem}'"
                ))),
            };
            if let Err(e) = result {
                log::error!("Failed to remove dbus interface {path}: {e:?}");
            } else {
                log::debug!("Stopped dbus interface: {path}");
            }
        });

        let id = device.get_id();

        if id.is_empty() {
            return Ok(());
        }
        log::debug!("Device ID: {id}");

        // Signal that a source device was removed
        self.on_source_device_removed(device, id).await?;

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
            if self.target_devices.contains_key(&path) {
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
            if self.composite_devices.contains_key(&path) {
                i += 1;
                continue;
            }
            return path;
        }
    }

    /// Watch for IIO device events
    fn watch_iio_devices(
        cmd_tx: mpsc::Sender<ManagerCommand>,
    ) -> tokio::task::JoinHandle<Result<(), Box<dyn Error + std::marker::Send + Sync>>> {
        task::spawn_blocking(move || {
            let mut monitor = MonitorBuilder::new()?.match_subsystem("iio")?.listen()?;

            let mut poll = Poll::new()?;
            let mut events = Events::with_capacity(1024);
            poll.registry()
                .register(&mut monitor, Token(0), Interest::READABLE)?;

            loop {
                if poll.poll(&mut events, None).is_err() {
                    std::thread::sleep(Duration::from_millis(10));
                    continue;
                }
                for event in monitor.iter() {
                    let action = event.action().unwrap_or_default();
                    let device = event.device();
                    let dev_name = device.name();
                    let dev_sysname = device.sysname().to_string_lossy();

                    match action.to_string_lossy().trim() {
                        "add" => {
                            log::debug!(
                                "Got udev add action for iio device {dev_name} ({dev_sysname})"
                            );
                            cmd_tx.blocking_send(ManagerCommand::DeviceAdded {
                                device: device.into(),
                            })?;
                        }
                        "remove" => {
                            log::debug!(
                                "Got udev remove action for iio device {dev_name} ({dev_sysname})"
                            );
                            cmd_tx.blocking_send(ManagerCommand::DeviceRemoved {
                                device: device.into(),
                            })?;
                        }
                        unhandled_action => {
                            log::trace!("Unhandled udev action for iio device {dev_name} ({dev_sysname}: {unhandled_action}");
                        }
                    }
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        })
    }

    /// Watch for appearance and disappearence of devices is /dev and associate the corresponding udev device
    async fn watch_devnodes(
        cmd_tx: mpsc::Sender<ManagerCommand>,
        watcher_rx: &mut mpsc::Receiver<WatchEvent>,
    ) {
        'outer: while let Some(event) = watcher_rx.recv().await {
            match event {
                WatchEvent::Create { name, base_path } => {
                    let subsystem = {
                        match base_path.as_str() {
                            "/dev" => {
                                if !name.starts_with("hidraw") {
                                    None
                                } else {
                                    Some("hidraw")
                                }
                            }
                            "/dev/input" => Some("input"),

                            _ => None,
                        }
                    };
                    let Some(subsystem) = subsystem else {
                        log::trace!("No supported subsystem detected for {base_path}/{name}");
                        continue;
                    };

                    // Wait until the device has initialized with udev
                    const MAX_TRIES: u8 = 80;
                    let mut attempt: u8 = 0;
                    loop {
                        // Break after max attempts reached
                        if attempt > MAX_TRIES {
                            log::warn!("Unable to create initialized UdevDevice for {base_path}/{name} after {MAX_TRIES} attempts.");
                            continue 'outer;
                        }

                        // Try to get the device from udev to check its initialization state
                        {
                            let Ok(device) = ::udev::Device::from_subsystem_sysname(
                                subsystem.to_string(),
                                name.clone(),
                            ) else {
                                log::debug!(
                                    "Unable to create UdevDevice from {base_path}/{name} to check initialization"
                                );
                                attempt += 1;
                                tokio::time::sleep(Duration::from_millis(50)).await;
                                continue;
                            };

                            if device.is_initialized() {
                                break;
                            }
                        };
                        log::trace!("{base_path}/{name} is not yet initialized by udev");

                        tokio::time::sleep(Duration::from_millis(50)).await;
                        attempt += 1;
                    }

                    // Create a udev device for the device
                    let Ok(device) =
                        ::udev::Device::from_subsystem_sysname(subsystem.to_string(), name.clone())
                    else {
                        log::warn!("Unable to create UdevDevice from {base_path}/{name}");
                        continue;
                    };

                    // Notify the manager that a device was added
                    log::debug!("Got inotify add action for {base_path}/{name}");
                    let result = cmd_tx
                        .send(ManagerCommand::DeviceAdded {
                            device: device.into(),
                        })
                        .await;
                    if let Err(e) = result {
                        log::error!("Unable to send command: {:?}", e);
                    }
                }
                WatchEvent::Delete { name, base_path } => {
                    let device = UdevDevice::from_devnode(base_path.as_str(), name.as_str());
                    log::debug!("Got inotify remove action for {base_path}/{name}");
                    let result = cmd_tx.send(ManagerCommand::DeviceRemoved { device }).await;
                    if let Err(e) = result {
                        log::error!("Unable to send command: {:?}", e);
                    }
                }
                WatchEvent::Modify {
                    name: _,
                    base_path: _,
                } => (),
            }
        }
    }

    /// Performs initial input device discovery of all supported subsystems
    async fn discover_all_devices(
        cmd_tx: &mpsc::Sender<ManagerCommand>,
    ) -> Result<(), Box<dyn Error>> {
        let hidraw_devices = udev::discover_devices("hidraw")?;
        let hidraw_devices = hidraw_devices.into_iter().map(|dev| dev.into()).collect();
        Manager::discover_devices(cmd_tx, hidraw_devices).await?;
        let event_devices = udev::discover_devices("input")?;
        let event_devices = event_devices.into_iter().map(|dev| dev.into()).collect();
        Manager::discover_devices(cmd_tx, event_devices).await?;
        let iio_devices = udev::discover_devices("iio")?;
        let iio_devices = iio_devices.into_iter().map(|dev| dev.into()).collect();
        Manager::discover_devices(cmd_tx, iio_devices).await?;

        Ok(())
    }

    async fn discover_devices(
        manager_tx: &mpsc::Sender<ManagerCommand>,
        devices: Vec<UdevDevice>,
    ) -> Result<(), Box<dyn Error>> {
        for device in devices {
            manager_tx
                .send(ManagerCommand::DeviceAdded { device })
                .await?;
        }

        Ok(())
    }

    /// Loads all capability mappings in all default locations and returns a hashmap
    /// of the CapabilityMap ID and the [CapabilityMap].
    pub async fn load_capability_mappings(&self) -> HashMap<String, CapabilityMap> {
        let mut mappings = HashMap::new();
        let paths = get_capability_maps_paths();

        // Look for capability mappings in all known locations
        for path in paths.iter() {
            let files = fs::read_dir(path);
            if files.is_err() {
                log::trace!("Failed to load directory {path:?}: {}", files.unwrap_err());
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
                log::trace!("Found file: {}", file.path().display());
                let mapping = CapabilityMap::from_yaml_file(file.path().display().to_string());
                if mapping.is_err() {
                    log::warn!(
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
        let task = task::spawn_blocking(move || {
            let mut devices: Vec<CompositeDeviceConfig> = Vec::new();
            let paths = get_devices_paths();

            // Look for composite device profiles in all known locations
            for path in paths.iter() {
                log::trace!("Checking {path:?} for composite device configs");
                let files = fs::read_dir(path);
                if files.is_err() {
                    log::debug!("Failed to load directory {path:?}: {}", files.unwrap_err());
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
                    log::trace!("Found file: {}", file.path().display());
                    let device =
                        CompositeDeviceConfig::from_yaml_file(file.path().display().to_string());
                    if device.is_err() {
                        log::warn!(
                            "Failed to parse composite device config '{}': {}",
                            file.path().display(),
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

    /// Creates a DBus object and return the (active) handle to the listener
    async fn listen_on_dbus(
        dbus: Connection,
        tx: mpsc::Sender<ManagerCommand>,
    ) -> tokio::task::JoinHandle<()> {
        let iface = ManagerInterface::new(tx);
        let manager_path = format!("{}/Manager", BUS_PREFIX);
        task::spawn(async move {
            if let Err(e) = dbus.object_server().at(manager_path, iface).await {
                log::error!("Failed create manager dbus interface: {e:?}");
            }
        })
    }

    async fn add_device_to_composite_device(
        &self,
        device: UdevDevice,
        client: &CompositeDeviceClient,
    ) -> Result<(), Box<dyn Error>> {
        client.add_source_device(device).await?;
        Ok(())
    }
}
