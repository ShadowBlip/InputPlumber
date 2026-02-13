use core::panic;
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::error::Error;
use std::future::Future;
use std::path::PathBuf;
use std::time::Duration;

use ::procfs::CpuInfo;
use ::udev::MonitorBuilder;
use futures::future::BoxFuture;
use mio::{Events, Interest, Poll, Token};
use thiserror::Error;
use tokio::sync::mpsc;
use tokio::task;
use tokio::task::JoinHandle;
use zbus::fdo::ManagedObjects;
use zbus::zvariant::ObjectPath;
use zbus::Connection;

use crate::bluetooth::device1::Device1Proxy;
use crate::config::capability_map::load_capability_mappings;
use crate::config::path::get_devices_paths;
use crate::config::path::get_multidir_sorted_files;
use crate::config::CompositeDeviceConfig;
use crate::config::SourceDevice;
use crate::constants::BUS_PREFIX;
use crate::constants::BUS_SOURCES_PREFIX;
use crate::constants::BUS_TARGETS_PREFIX;
use crate::dbus::interface::manager::ManagerInterface;
use crate::dbus::interface::source::evdev::SourceEventDeviceInterface;
use crate::dbus::interface::source::hidraw::SourceHIDRawInterface;
use crate::dbus::interface::source::iio_imu::SourceIioImuInterface;
use crate::dbus::interface::source::led::SourceLedInterface;
use crate::dbus::interface::source::tty::SourceTtyInterface;
use crate::dbus::interface::source::udev::SourceUdevDeviceInterface;
use crate::dbus::interface::DBusInterfaceManager;
use crate::dmi::data::DMIData;
use crate::dmi::get_cpu_info;
use crate::dmi::get_dmi_data;
use crate::input::composite_device::CompositeDevice;
use crate::input::source::evdev;
use crate::input::source::hidraw;
use crate::input::source::iio;
use crate::input::source::led;
use crate::input::source::tty;
use crate::input::target::TargetDevice;
use crate::input::target::TargetDeviceTypeId;
use crate::udev;
use crate::udev::device::AttributeGetter;
use crate::udev::device::UdevDevice;

use super::composite_device::client::CompositeDeviceClient;
use super::info::DeviceInfo;
use super::target::client::TargetDeviceClient;
use super::target::TargetDeviceClass;

use crate::watcher;
use crate::watcher::WatchEvent;

const DEV_PATH: &str = "/dev";
const INPUT_PATH: &str = "/dev/input";
const BUFFER_SIZE: usize = 20480;
const VIRT_DEVICE_WHITELIST: &[&str] = &[
    "Sunshine PS5 (virtual) pad",
    "Sunshine X-Box One (virtual) pad",
    "Sunshine gamepad (virtual) motion sensors",
    "Sunshine Nintendo (virtual) pad",
];

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
    ProcessTaskQueue,
    TaskDone,
    DeviceAdded {
        device: DeviceInfo,
    },
    DeviceRemoved {
        device: DeviceInfo,
    },
    CreateCompositeDevice {
        config: CompositeDeviceConfig,
    },
    CreateAndStartCompositeDevice {
        path: PathBuf,
        config: CompositeDeviceConfig,
        source_device: Box<SourceDevice>,
        device: DeviceInfo,
        completion: mpsc::Sender<()>,
    },
    AddDeviceToCompositeDevice {
        id: String,
        source_device: Box<SourceDevice>,
        device: DeviceInfo,
        client: CompositeDeviceClient,
        composite_device: String,
        completion: mpsc::Sender<()>,
    },
    CreateTargetDevice {
        kind: TargetDeviceTypeId,
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
    RemoveFromGamepadOrder {
        device_path: String,
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
    SetGamepadOrder {
        dbus_paths: Vec<String>,
    },
    GamepadReorderingFinished,
    GetCompositeDevices {
        #[allow(clippy::type_complexity)]
        sender: mpsc::Sender<(
            // Composite Devices
            HashMap<String, CompositeDeviceClient>,
            // Composite Device Sources
            HashMap<String, Vec<SourceDevice>>,
            // Used configs
            HashMap<String, CompositeDeviceConfig>,
        )>,
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
    /// Dbus interface
    dbus: DBusInterfaceManager,
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
    /// Task queue is used to run sequential tasks without blocking the event
    /// loop.
    task_queue: VecDeque<(BoxFuture<'static, ()>, mpsc::Receiver<()>)>,
    /// Whether or not a task from the task queue is currently executing.
    task_running: bool,
    /// Mapping of source devices to their SourceDevice objects.
    /// E.g. {"evdev://event0": <SourceDevice>}
    source_devices: HashMap<String, SourceDevice>,
    /// Mapping of source devices to their [DBusInterfaceManager] that contains
    /// its dbus path and interfaces used.
    /// E.g. {"evdev://event0": <DBusInterfaceManager>}
    source_device_dbus_paths: HashMap<String, DBusInterfaceManager>,
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
    composite_device_targets: HashMap<String, HashSet<String>>,
    /// Mapping of DBus path to its corresponding [CompositeDeviceConfig]
    /// E.g. {"/org/shadowblip/InputPlumber/CompositeDevice0": <CompositeDeviceConfig>}
    used_configs: HashMap<String, CompositeDeviceConfig>,
    /// Mapping of target devices to their respective handles
    /// E.g. {"/org/shadowblip/InputPlumber/devices/target/dbus0": <Handle>}
    target_devices: HashMap<String, TargetDeviceClient>,
    /// List of composite device dbus paths with gamepad devices in player order.
    /// E.g. ["/org/shadowblip/InputPlumber/CompositeDevice0"]
    target_gamepad_order: Vec<String>,
    /// Whether or not target gamepad reordering is changing.
    target_gamepad_order_changing: bool,
    /// Defines whether or not InputPlumber should try to automatically manage all
    /// input devices that have a [CompositeDeviceConfig] definition
    manage_all_devices: bool,
}

impl Manager {
    /// Returns a new instance of Gamepad Manager
    pub fn new(conn: Connection) -> Manager {
        let path = format!("{BUS_PREFIX}/Manager");
        let dbus = DBusInterfaceManager::new(conn, path).expect("Manager path should be valid");

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
            dbus,
            dmi_data,
            cpu_info,
            rx,
            tx,
            task_queue: VecDeque::new(),
            task_running: false,
            composite_devices: HashMap::new(),
            source_devices: HashMap::new(),
            source_device_dbus_paths: HashMap::new(),
            source_devices_used: HashMap::new(),
            target_devices: HashMap::new(),
            used_configs: HashMap::new(),
            composite_device_sources: HashMap::new(),
            composite_device_targets: HashMap::new(),
            manage_all_devices: false,
            target_gamepad_order: vec![],
            target_gamepad_order_changing: false,
        }
    }

    /// Starts listening for [Command] messages to be sent from clients and
    /// dispatch those events.
    pub async fn run(&mut self) -> Result<(), Box<dyn Error + Send + Sync>> {
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

        self.listen_on_dbus();
        let _ = tokio::join!(
            Self::discover_all_devices(&cmd_tx_all_devices),
            Self::watch_iio_devices(self.tx.clone()),
            Self::watch_devnodes(self.tx.clone(), &mut watcher_rx),
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
                ManagerCommand::ProcessTaskQueue => {
                    let Some((task, mut completion)) = self.task_queue.pop_front() else {
                        continue;
                    };
                    let tx = self.tx.clone();
                    self.task_running = true;
                    tokio::spawn(async move {
                        // Execute the task
                        task.await;

                        // Wait for a response
                        completion.recv().await;

                        // Process the next task in the queue
                        let _ = tx.send(ManagerCommand::TaskDone).await;
                    });
                }
                ManagerCommand::TaskDone => {
                    self.task_running = false;
                    if self.task_queue.is_empty() {
                        continue;
                    }
                    let tx = self.tx.clone();
                    tokio::spawn(async move { tx.send(ManagerCommand::ProcessTaskQueue).await });
                }
                ManagerCommand::CreateCompositeDevice { config } => {
                    if let Err(e) = self.create_composite_device(config).await {
                        log::error!("Error creating composite device: {:?}", e);
                    }
                }
                ManagerCommand::CreateAndStartCompositeDevice {
                    path,
                    config,
                    source_device,
                    device,
                    completion,
                } => {
                    // Create the composite device
                    let dev = match self
                        .create_composite_device_from_config(path, &config, device)
                        .await
                    {
                        Ok(dev) => dev,
                        Err(e) => {
                            log::error!("Failed to create composite device from config: {e}");
                            continue;
                        }
                    };

                    // Get the target input devices from the config
                    let target_devices_config = config.target_devices.clone();

                    // Create the composite deivce
                    let result = self
                        .start_composite_device(
                            dev,
                            config.clone(),
                            target_devices_config,
                            *source_device,
                        )
                        .await;
                    if let Err(e) = result {
                        log::error!("Failed to start composite device: {e}");
                    }
                    let _ = completion.send(()).await;
                }
                ManagerCommand::AddDeviceToCompositeDevice {
                    id,
                    source_device,
                    device,
                    client,
                    composite_device,
                    completion,
                } => {
                    if let Err(e) = self.add_device_to_composite_device(device, &client).await {
                        log::error!("Failed to add device to composite device: {e}");
                        continue;
                    }
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
                    sources.push(*source_device.clone());
                    self.source_devices.insert(id, *source_device);
                    let _ = completion.send(()).await;
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
                        _ => Err(ManagerError::CreateTargetDeviceFailed(
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
                    if let Err(err) = self
                        .attach_target_device(target_path.as_str(), composite_path.as_str())
                        .await
                    {
                        log::error!("Failed to attach {target_path} to {composite_path}: {err:?}");
                        if let Err(e) = sender.send(Err(err)).await {
                            log::error!("Failed to send response: {e:?}");
                        }
                        continue;
                    }
                    if let Err(e) = sender.send(Ok(())).await {
                        log::error!("Failed to send response: {e:?}");
                    }
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

                    // Lookup the compoiste device and see if it is suspended?
                    // TODO: Use a different hashmap to map target device to composite device
                    let mut device_path = None;
                    for (composite_path, target_device_paths) in
                        self.composite_device_targets.iter()
                    {
                        if target_device_paths.contains(&path) {
                            device_path = Some(composite_path.clone());
                            break;
                        }
                    }
                    let Some(device_path) = device_path else {
                        continue;
                    };

                    log::debug!("Found composite device for target device: {device_path}");
                    let Some(device) = self.composite_devices.get(&device_path) else {
                        continue;
                    };

                    self.composite_device_targets
                        .entry(device_path.clone())
                        .and_modify(|paths| {
                            paths.remove(&path);
                        });
                    log::debug!("Used target devices: {:?}", self.composite_device_targets);

                    // Spawn a new task to query the composite device for its
                    // suspended state in order to prevent deadlocks.
                    let tx = self.tx.clone();
                    let device = device.clone();
                    tokio::task::spawn(async move {
                        let is_suspended = match device.is_suspended().await {
                            Ok(suspended) => suspended,
                            Err(e) => {
                                log::debug!("Failed to check if device is suspended: {e:?}");
                                return;
                            }
                        };

                        // If the composite device is suspended, do not remove the
                        // target device from the gamepad ordering.
                        if is_suspended {
                            return;
                        }

                        // Notify the Manager to remove the gamepad from
                        // the gamepad order.
                        if let Err(e) = tx
                            .send(ManagerCommand::RemoveFromGamepadOrder { device_path })
                            .await
                        {
                            log::debug!("Failed to notify input manager to remove device from gamepad order: {e}");
                        }
                    });
                }
                ManagerCommand::RemoveFromGamepadOrder { device_path } => {
                    let new_order = self
                        .target_gamepad_order
                        .drain(..)
                        .filter(|paf| paf.as_str() != device_path.as_str())
                        .collect();

                    // Update gamepad order in dbus interface
                    self.update_and_emit_gamepad_order(new_order);

                    log::info!("Gamepad order: {:?}", self.target_gamepad_order);
                }
                ManagerCommand::DeviceAdded { device } => match device {
                    DeviceInfo::Udev(device) => {
                        let dev_name = device.name();
                        let dev_sysname = device.sysname();

                        if let Err(e) = self.on_udev_device_added(device).await {
                            log::error!("Error adding device '{dev_name} ({dev_sysname})': {e}");
                        }
                    }
                },
                ManagerCommand::DeviceRemoved { device } => match device {
                    DeviceInfo::Udev(device) => {
                        if let Err(e) = self.on_udev_device_removed(device).await {
                            log::error!("Error removing device: {e}");
                        }
                    }
                },
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
                    log::info!("Preparing to suspend all target devices");

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

                        log::info!("Finished preparing suspending all target devices");
                    });
                }
                ManagerCommand::SystemWake { sender } => {
                    log::info!("Preparing to resume all target devices");

                    // Call the resume handler on each composite device and wait
                    // for a response.
                    let composite_devices = self.composite_devices.clone();
                    let gamepad_order = self.target_gamepad_order.clone();
                    tokio::task::spawn(async move {
                        // Resume any composite devices in gamepad order first
                        for path in gamepad_order {
                            let Some(device) = composite_devices.get(&path) else {
                                continue;
                            };
                            log::info!("Resuming device: {path}");
                            if let Err(e) = device.resume().await {
                                log::error!("Failed to call resume handler on device: {e:?}");
                            }
                        }

                        // Resume any remaining composite devices
                        for (path, device) in composite_devices.iter() {
                            log::info!("Resuming device: {path}");
                            let is_suspended = match device.is_suspended().await {
                                Ok(suspended) => suspended,
                                Err(e) => {
                                    log::error!("Failed to check if device is suspended: {e:?}");
                                    continue;
                                }
                            };
                            if !is_suspended {
                                log::info!("Device `{path}` is not suspended. No need to resume.");
                                continue;
                            }
                            if let Err(e) = device.resume().await {
                                log::error!("Failed to call resume handler on device: {e:?}");
                            }
                        }

                        // Respond to the sender to inform them that resume tasks
                        // have completed.
                        if let Err(e) = sender.send(()).await {
                            log::error!("Failed to send response: {e:?}");
                        }

                        log::info!("Finished preparing resuming all target devices");
                    });
                }
                ManagerCommand::SetGamepadOrder { dbus_paths } => {
                    self.set_gamepad_order(dbus_paths).await;
                }
                ManagerCommand::GamepadReorderingFinished => {
                    log::info!("Finished reordering target devices");
                    self.target_gamepad_order_changing = false;
                }
                ManagerCommand::GetCompositeDevices { sender } => {
                    let composite_devices = self.composite_devices.clone();
                    let composite_device_sources = self.composite_device_sources.clone();
                    let used_configs = self.used_configs.clone();
                    let response = (composite_devices, composite_device_sources, used_configs);
                    if let Err(e) = sender.send(response).await {
                        log::warn!("Failed to send response to get composite devices: {e}");
                    }
                }
            }
        }

        log::info!("Stopped input manager");

        Ok(())
    }

    /// Add the given future to the task queue. Tasks will be executed in a
    /// first in, first out way. Tasks should send or drop the given completion
    /// channel when the task is done.
    fn queue_task(
        &mut self,
        task: impl Future<Output = ()> + Send + 'static,
        completion_rx: mpsc::Receiver<()>,
    ) {
        if !self.task_running {
            let tx = self.tx.clone();
            tokio::spawn(async move { tx.send(ManagerCommand::ProcessTaskQueue).await });
        }
        self.task_queue.push_back((Box::pin(task), completion_rx));
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
        path: PathBuf,
        config: &CompositeDeviceConfig,
        device: DeviceInfo,
    ) -> Result<CompositeDevice, Box<dyn Error>> {
        // Lookup the capability map associated with this config if it exists
        let capability_map = if let Some(map_id) = config.capability_map_id.clone() {
            log::debug!("Found capability mapping in config: {map_id}");
            let capability_map = load_capability_mappings();
            capability_map.get(&map_id).cloned()
        } else {
            None
        };

        // Create a composite device to manage these devices
        log::info!("Found matching source device for config {path:?}");
        let config = config.clone();
        let device = CompositeDevice::new(
            self.dbus.connection().clone(),
            self.tx.clone(),
            config,
            device,
            self.next_composite_dbus_path()?,
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
    /// Returns the DBus path for the device and the device itself.
    async fn create_target_device(
        &self,
        kind: &str,
    ) -> Result<(String, TargetDevice), Box<dyn Error>> {
        log::trace!("Creating target device: {kind}");
        let Ok(target_id) = TargetDeviceTypeId::try_from(kind) else {
            return Err("Invalid target device ID".to_string().into());
        };

        // Create the target device to emulate based on the kind
        let path = self.next_target_path(target_id.device_class())?;
        let dbus = DBusInterfaceManager::new(self.dbus.connection().clone(), path.clone())?;
        let device = TargetDevice::from_type_id(target_id, dbus)?;

        Ok((path, device))
    }

    /// Start and run the given target devices. Returns a HashMap of transmitters
    /// to send events to the given targets.
    async fn start_target_devices(
        &mut self,
        targets: Vec<(String, TargetDevice)>,
    ) -> Result<HashMap<String, TargetDeviceClient>, Box<dyn Error>> {
        let mut target_devices = HashMap::new();
        for (path, target) in targets {
            // Get a client reference to communicate with the target device
            let Some(client) = target.client() else {
                log::trace!("No client implemented for target device");
                continue;
            };
            target_devices.insert(path.clone(), client.clone());
            self.target_devices.insert(path.clone(), client.clone());

            // Run the target device
            tokio::spawn(async move {
                if let Err(e) = target.run().await {
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

    /// Attach the given target device to the given composite device
    async fn attach_target_device(
        &mut self,
        target_path: &str,
        composite_path: &str,
    ) -> Result<(), ManagerError> {
        // Check to see if the target device is a gamepad
        let is_gamepad = {
            let Some(target) = self.target_devices.get(target_path) else {
                let err =
                    ManagerError::AttachTargetDeviceFailed("Failed to find target device".into());
                return Err(err);
            };

            // Check the target device type
            let target_type = match target.get_type().await {
                Ok(kind) => kind,
                Err(e) => {
                    let err = ManagerError::AttachTargetDeviceFailed(format!(
                        "Failed to get target device type: {e:?}"
                    ));
                    return Err(err);
                }
            };
            let Some(target_type) = TargetDeviceTypeId::try_from(target_type.as_str()).ok() else {
                let err = ManagerError::AttachTargetDeviceFailed(
                    "Target device returned an invalid device type!".into(),
                );
                return Err(err);
            };

            target_type.is_gamepad()
        };

        // If the target device is a gamepad, maintain the order in which it
        // was connected.
        if is_gamepad
            && !self
                .target_gamepad_order
                .contains(&composite_path.to_owned())
        {
            let mut new_order = self.target_gamepad_order.clone();
            new_order.push(composite_path.to_string());

            // Update gamepad order in dbus interface
            self.update_and_emit_gamepad_order(new_order);

            log::info!("Gamepad order: {:?}", self.target_gamepad_order);
        }

        let Some(target) = self.target_devices.get(target_path) else {
            let err = ManagerError::AttachTargetDeviceFailed("Failed to find target device".into());
            return Err(err);
        };

        let Some(device) = self.composite_devices.get(composite_path) else {
            let err =
                ManagerError::AttachTargetDeviceFailed("Failed to find composite device".into());
            return Err(err);
        };

        // Send the attach command to the composite device
        let mut targets = HashMap::new();
        targets.insert(target_path.to_string(), target.clone());
        if let Err(e) = device.attach_target_devices(targets).await {
            let err = ManagerError::AttachTargetDeviceFailed(format!(
                "Failed to send attach command: {e:?}"
            ));
            return Err(err);
        }

        // Track the composite device and target device
        self.composite_device_targets
            .entry(composite_path.to_string())
            .and_modify(|paths| {
                paths.insert(target_path.to_string());
            })
            .or_insert({
                let mut paths = HashSet::new();
                paths.insert(target_path.to_string());
                paths
            });
        log::debug!("Used target devices: {:?}", self.composite_device_targets);

        log::debug!("Finished handling attach request for: {target_path}");

        Ok(())
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
    ) -> Result<JoinHandle<()>, Box<dyn Error>> {
        // Keep track of the source devices that this composite device is
        // using.
        let source_device_ids = device.get_source_devices_used();
        let composite_path = String::from(device.dbus_path());
        log::debug!(
            "Starting CompositeDevice at {composite_path} with the following sources: {source_device_ids:?}"
        );
        for id in source_device_ids {
            self.source_devices_used
                .insert(id.clone(), composite_path.clone());
            self.source_devices.insert(id, source_device.clone());
        }

        if !self.composite_device_sources.contains_key(&composite_path) {
            self.composite_device_sources
                .insert(composite_path.clone(), Vec::new());
        }
        let sources = self
            .composite_device_sources
            .get_mut(&composite_path)
            .unwrap();
        sources.push(source_device);

        // Get a handle to the device
        let client = device.client();

        // Keep track of target devices that this composite device is using
        let mut target_device_paths = Vec::new();

        // Queue target devices based on the configuration
        let mut target_devices = Vec::new();
        if let Some(target_devices_config) = target_types {
            for kind in target_devices_config {
                let Ok(target_id) = TargetDeviceTypeId::try_from(kind.as_str()) else {
                    return Err("Invalid target device ID".to_string().into());
                };
                target_devices.push(target_id);
            }
        }
        client.set_target_devices(target_devices).await?;

        // Create a DBus target device
        log::debug!("Creating target devices for {composite_path}");
        let dbus_device = self.create_target_device("dbus").await?;
        let dbus_devices = self.start_target_devices(vec![dbus_device]).await?;
        let dbus_paths = dbus_devices.keys();
        for dbus_path in dbus_paths {
            target_device_paths.push(dbus_path.clone());
        }
        device.set_dbus_devices(dbus_devices);
        device.listen_on_dbus().await?;

        // Add the device to our maps
        self.composite_devices
            .insert(composite_path.clone(), client);
        log::trace!("Managed source devices: {:?}", self.source_devices_used);
        self.used_configs.insert(composite_path.clone(), config);
        log::trace!("Used configs: {:?}", self.used_configs);
        self.composite_device_targets.insert(
            composite_path.to_string(),
            HashSet::with_capacity(target_device_paths.len()),
        );

        // Run the device
        let composite_path = String::from(device.dbus_path());
        let tx = self.tx.clone();
        let task = tokio::spawn(async move {
            if let Err(e) = device.run().await {
                log::error!("Error running {composite_path}: {e}");
            }
            log::debug!("Composite device stopped running: {composite_path}");
            if let Err(e) = tx
                .send(ManagerCommand::CompositeDeviceStopped(
                    composite_path.clone(),
                ))
                .await
            {
                log::error!(
                    "Error sending to composite device {composite_path} the stopped signal: {e}"
                );
            }
        });

        Ok(task)
    }

    /// Called when a composite device stops running
    async fn on_composite_device_stopped(&mut self, path: String) -> Result<(), Box<dyn Error>> {
        log::debug!("Removing composite device: {}", path);

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

        // Remove the device from gamepad order
        let new_order = self
            .target_gamepad_order
            .drain(..)
            .filter(|paf| paf.as_str() != path.as_str())
            .collect();

        // Update gamepad order in dbus interface
        self.update_and_emit_gamepad_order(new_order);

        log::info!("Gamepad order: {:?}", self.target_gamepad_order);

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
        device: DeviceInfo,
    ) -> Result<(), Box<dyn Error>> {
        // Ignore the device if it's already in use.
        if let Some(device_path) = self.source_devices_used.get(&id) {
            log::debug!("Source device {id} already in use by {device_path}. Skipping.");
            return Ok(());
        }

        // NOTE: reading udev devices may be slow and blocking, so this is done
        // in a task to allow the input manager to continue processing requests.
        let configs = self.load_device_configs().await;
        let dmi_data = self.dmi_data.clone();
        let cpu_info = self.cpu_info.clone();
        let tx = self.tx.clone();
        let (complete_tx, complete_rx) = mpsc::channel(1);
        let task = async move {
            // Query the manager for the current state of composite devices
            let (manage_devices_tx, mut manage_devices_rx) = mpsc::channel(1);
            let cmd = ManagerCommand::GetManageAllDevices {
                sender: manage_devices_tx,
            };
            if let Err(e) = tx.send(cmd).await {
                log::error!("Failed to send get manage all devices command: {e}");
                return;
            }
            let Some(manage_all_devices) = manage_devices_rx.recv().await else {
                log::error!("Failed to receive response for get manage all devices");
                return;
            };
            let (composite_devices_tx, mut composite_devices_rx) = mpsc::channel(1);
            let cmd = ManagerCommand::GetCompositeDevices {
                sender: composite_devices_tx,
            };
            if let Err(e) = tx.send(cmd).await {
                log::error!("Failed to send get composite devices command: {e}");
                return;
            }
            let Some(response) = composite_devices_rx.recv().await else {
                log::error!("Failed to receive response for get composite devices");
                return;
            };
            let (composite_devices, composite_device_sources, used_configs) = response;

            // Check all existing composite devices to see if this device is part of
            // their config
            'start: for composite_device in composite_devices.keys() {
                let Some(config) = used_configs.get(composite_device) else {
                    continue;
                };
                log::debug!("Checking if existing composite device {composite_device:?} with config {:?} is missing device: {id:?}", config.name);

                log::trace!(
                    "Composite device has {} source devices defined",
                    config.source_devices.len()
                );

                let max_sources = config.maximum_sources.unwrap_or_else(|| {
                    if config.single_source.unwrap_or(false) {
                        1
                    } else {
                        0
                    }
                });

                // If the CompositeDevice only allows a maximum number of source devices,
                // check to see if that limit has been reached. If that limit is reached,
                // then a new CompositeDevice will be created for the source device.
                // If maximum_sources is less than 1 (e.g. 0, -1) then consider
                // the maximum to be 'unlimited'.
                if max_sources > 0 {
                    // Check to see how many source devices this composite device is
                    // currently managing.
                    if composite_device_sources
                        .get(composite_device)
                        .is_some_and(|sources| (sources.len() as i32) >= max_sources)
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
                    log::debug!(
                        "Device {id} does not match existing device: {:?}",
                        config.name
                    );

                    continue;
                };

                // Check if the device has already been used in this config or not,
                // stop here if the device must be unique.
                if let Some(sources) = composite_device_sources.get(composite_device) {
                    for source in sources {
                        if *source != source_device {
                            continue;
                        }

                        if source_device.ignore.is_some_and(|ignored| ignored) {
                            log::debug!(
                            "Ignoring device {:?}, not adding to composite device: {composite_device}",
                            source_device
                        );
                            break 'start;
                        }

                        // Check if the composite device has to be unique (default to being unique)
                        if source_device.unique.unwrap_or(true) {
                            log::trace!(
                            "Found unique device {:?}, not adding to composite device {composite_device}",
                            source_device
                        );
                            break 'start;
                        }
                    }
                }

                log::info!("Found missing {} device, adding source device {id} to existing composite device: {composite_device:?}", device.kind());
                let Some(client) = composite_devices.get(composite_device.as_str()).cloned() else {
                    log::error!("No existing composite device found for key {composite_device:?}");
                    continue;
                };

                // Send the command to the input manager to add the device
                let cmd = ManagerCommand::AddDeviceToCompositeDevice {
                    id,
                    source_device: Box::new(source_device),
                    device,
                    client,
                    composite_device: composite_device.clone(),
                    completion: complete_tx,
                };
                if let Err(e) = tx.send(cmd).await {
                    log::error!(
                        "Failed to send manager command to add device to composite device: {e}"
                    );
                }

                return;
            }

            log::debug!("No existing composite device matches device {id}.");

            log::debug!("Checking unused configs");

            // Check all CompositeDevice configs to see if this device creates
            // a match that will automatically create a CompositeDevice.
            for (path, config) in configs {
                log::trace!("Checking if config {path:?} matches device",);

                // Check to see if 'auto_manage' is enabled for this config.
                let auto_manage = config
                    .options
                    .as_ref()
                    .map(|options| options.auto_manage.unwrap_or(false))
                    .unwrap_or(false);
                if !manage_all_devices && !auto_manage {
                    log::trace!(
                        "Config {path:?} does not have 'auto_manage' option enabled. Skipping.",
                    );
                    continue;
                }

                // Check to see if this configuration matches the system
                if !config.has_valid_matches(&dmi_data, &cpu_info) {
                    log::trace!("Configuration {path:?} does not match system");
                    continue;
                }

                // Check if this device matches any source configs
                let Some(source_device) = config.get_matching_device(&device) else {
                    log::trace!("Device does not match config: {:?}", config.name);
                    continue;
                };
                if let Some(ignored) = source_device.ignore {
                    if ignored {
                        log::trace!("Event device configured to ignore: {:?}", device);
                        return;
                    }
                }
                log::info!(
                    "Found a matching {} device {id} in config {path:?}, creating CompositeDevice",
                    device.kind()
                );

                // Send request to manager to create and start composite device
                let cmd = ManagerCommand::CreateAndStartCompositeDevice {
                    path,
                    config,
                    source_device: Box::new(source_device),
                    device,
                    completion: complete_tx,
                };
                if let Err(e) = tx.send(cmd).await {
                    log::warn!("Failed to send create and start device to manager: {e}");
                }

                return;
            }
            log::debug!("No unused configs found for device.");
        };
        self.queue_task(task, complete_rx);

        Ok(())
    }

    /// Called when any source device is removed
    async fn on_source_device_removed(
        &mut self,
        device: DeviceInfo,
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
        self.source_devices_used.remove(&id);

        Ok(())
    }

    /// Called when a new device is detected by udev
    async fn on_udev_device_added(&mut self, device: UdevDevice) -> Result<(), Box<dyn Error>> {
        let dev_path = device.devpath();
        let dev_name = device.name();
        let dev_sysname = device.sysname();
        let sys_name = device.sysname();
        if sys_name.is_empty() {
            log::debug!("Device discarded for missing sysname: {dev_name} at {dev_path}");
            return Ok(());
        }
        let dev = device.clone();

        log::debug!("Device added: {dev_name} ({dev_sysname}): {dev_path}");

        // Get the device subsystem
        let subsystem = device.subsystem();

        // Get the device id
        let id = device.get_id();

        // Create a DBus interface manager for the source device if one does not exist
        if !self.source_device_dbus_paths.contains_key(&id) {
            // Get the DBus path based on the device subsystem
            let path = match subsystem.as_str() {
                "input" => evdev::get_dbus_path(sys_name),
                "hidraw" => hidraw::get_dbus_path(sys_name),
                "iio" => iio::get_dbus_path(sys_name),
                "leds" => led::get_dbus_path(sys_name),
                "tty" => tty::get_dbus_path(sys_name),
                _ => return Err(format!("Device subsystem not supported: {subsystem:?}").into()),
            };
            let conn = self.dbus.connection().clone();
            let mut dbus = DBusInterfaceManager::new(conn, path)?;

            // Register subsystem-specific DBus interfaces
            match subsystem.as_str() {
                "input" => {
                    let evdev_iface = SourceEventDeviceInterface::new(dev);
                    dbus.register(evdev_iface);
                }
                "hidraw" => {
                    let hidraw_iface = SourceHIDRawInterface::new(dev);
                    dbus.register(hidraw_iface);
                }
                "iio" => {
                    let iio_iface = SourceIioImuInterface::new(dev);
                    dbus.register(iio_iface);
                }
                "leds" => {
                    let led_iface = SourceLedInterface::new(dev);
                    dbus.register(led_iface);
                }
                "tty" => {
                    let tty_iface = SourceTtyInterface::new(dev);
                    dbus.register(tty_iface);
                }
                _ => (),
            }

            // Register the generic udev dbus interface for the device. The
            // [DBusInterfaceManager] will unregister all interfaces automatically
            // if it goes out of scope.
            let udev_iface = SourceUdevDeviceInterface::new(device.clone());
            dbus.register(udev_iface);

            // Track the lifetime of the source device to keep the dbus interface(s) up
            self.source_device_dbus_paths.insert(id.clone(), dbus);
        }

        // Check to see if the device should be managed by inputplumber or not.
        let mut notify_device_added = true;
        match subsystem.as_str() {
            "input" => {
                if device.devnode().is_empty() {
                    log::debug!("Event device discarded for missing devnode: {dev_name} ({dev_sysname}) at {dev_path}");
                    return Ok(());
                }

                log::debug!("event device added: {dev_name} ({dev_sysname})");

                // Check to see if the device should be managed or not
                'check_manage: {
                    if !device.is_virtual() {
                        log::trace!("{dev_name} ({dev_sysname}) is a real device - {dev_path}");
                        break 'check_manage;
                    }

                    // Look up the connected device using udev
                    let device_info = udev::get_device(dev_path.clone()).await?;

                    // Check if the virtual device is using the bluetooth bus
                    let bus_id = device.get_attribute_from_tree("id/bustype");
                    let id_bus = device_info.properties.get("ID_BUS");

                    log::debug!("Bus ID for {dev_path}: udev: {id_bus:?}, UdevDevice: {bus_id:?}");
                    let is_bluetooth = {
                        if let Some(bus) = id_bus {
                            bus == "bluetooth"
                        } else if let Some(bus) = bus_id {
                            bus == "0005"
                        } else {
                            false
                        }
                    };

                    // Some virtual gamepads we DO want to manage
                    let device_name = device.get_attribute_from_tree("name").unwrap_or_default();
                    let is_whitelisted = VIRT_DEVICE_WHITELIST.contains(&device_name.as_str());

                    if !is_bluetooth && !is_whitelisted {
                        log::debug!("{dev_name} ({dev_sysname}) is virtual, skipping consideration for {dev_path}");
                        notify_device_added = false;
                    }
                    if is_bluetooth {
                        log::debug!("{dev_name} ({dev_sysname}) is a virtual device node for a bluetooth device. Treating as real - {dev_path}");
                    }
                    if is_whitelisted {
                        log::debug!("{dev_name} ({dev_sysname}) is a virtual device node for a whitelisted device. Treating as real - {dev_path}")
                    }
                }
            }

            "hidraw" => {
                if device.devnode().is_empty() {
                    log::debug!("hidraw device discarded for missing devnode: {dev_name} ({dev_sysname}) at {dev_path}");
                    return Ok(());
                }

                log::debug!("hidraw device added: {dev_name} ({dev_sysname})");

                // Check to see if the device should be managed or not
                'check_manage: {
                    if !device.is_virtual() {
                        log::trace!("{dev_name} ({dev_sysname})  is a real device -{dev_path}");
                        break 'check_manage;
                    }

                    // Check to see if this virtual device is a bluetooth device
                    let uniq = device.uniq();
                    if uniq.is_empty() {
                        log::debug!("{dev_name} ({dev_sysname}) is virtual, skipping consideration for {dev_path}.");
                        notify_device_added = false;
                        break 'check_manage;
                    };

                    // Check bluez to see if that uniq is a bluetooth device
                    let object_manager =
                        zbus::fdo::ObjectManagerProxy::builder(self.dbus.connection())
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
                        let bt_device = Device1Proxy::builder(self.dbus.connection())
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
                        notify_device_added = false;
                        break 'check_manage;
                    }
                    log::debug!("{dev_name} ({dev_sysname}) is a virtual device node for a bluetooth device. Treating as real - {dev_path}");
                }
            }

            "iio" => {
                if device.devnode().is_empty() {
                    log::warn!("iio device discarded for missing devnode: {dev_name} ({dev_sysname}) at {dev_path}");
                    return Ok(());
                }

                log::debug!("iio device added: {} ({})", device.name(), device.sysname());

                // Check to see if the device is virtual
                if device.is_virtual() {
                    log::debug!("{dev_name} ({dev_sysname}) is virtual, skipping consideration for {dev_path}");
                    notify_device_added = false;
                } else {
                    log::trace!("Device {dev_name} ({dev_sysname}) is real - {dev_path}");
                }
            }

            "leds" => {
                log::debug!("LED device added: {} ({})", device.name(), device.sysname());

                // Check to see if the device is virtual
                if device.is_virtual() {
                    log::debug!("{dev_name} ({dev_sysname}) is virtual, skipping consideration for {dev_path}");
                    notify_device_added = false;
                } else {
                    log::trace!("Device {dev_name} ({dev_sysname}) is real - {dev_path}");
                }
            }

            "tty" => {
                log::debug!("TTY device added: {} ({})", device.name(), device.sysname());

                // Check to see if the device is virtual
                if device.is_virtual() {
                    log::debug!("{dev_name} ({dev_sysname}) is virtual, skipping consideration for {dev_path}");
                    notify_device_added = false;
                } else {
                    log::trace!("Device {dev_name} ({dev_sysname}) is real - {dev_path}");
                }
            }

            _ => {
                return Err(format!("Device subsystem not supported: {subsystem:?}").into());
            }
        };

        // Signal that a source device was added
        if notify_device_added {
            log::debug!("Spawning task to add source device: {id}");
            self.on_source_device_added(id.clone(), device.into())
                .await?;
            log::debug!("Finished adding {id}");
        }

        Ok(())
    }

    async fn on_udev_device_removed(&mut self, device: UdevDevice) -> Result<(), Box<dyn Error>> {
        let dev_name = device.name();
        let sys_name = device.sysname();
        log::debug!("Device removed: {dev_name} ({sys_name})");
        let path = ObjectPath::from_string_unchecked(format!("{BUS_SOURCES_PREFIX}/{sys_name}"));
        log::debug!("Device dbus path: {path}");

        let id = device.get_id();

        if id.is_empty() {
            log::warn!("Removed device had an empty id: {device:?}");
            return Ok(());
        }
        log::debug!("Device ID: {id}");

        // Signal that a source device was removed
        self.source_device_dbus_paths.remove(&id);
        self.on_source_device_removed(device.into(), id).await?;

        Ok(())
    }

    /// Returns the next available target device dbus path
    fn next_target_path(&self, kind: TargetDeviceClass) -> Result<String, Box<dyn Error>> {
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
    fn next_composite_dbus_path(&self) -> Result<String, Box<dyn Error>> {
        for i in 0u64.. {
            let path = format!("{}/CompositeDevice{}", BUS_PREFIX, i);
            if !self.composite_devices.contains_key(&path) {
                return Ok(path);
            }
        }

        Err(Box::from("No available dbus path left"))
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

                    // Ensure the path is a valid devnode
                    let full_path = PathBuf::from(format!("{base_path}/{name}"));
                    if full_path.is_dir() {
                        log::trace!("Devnode path {base_path}/{name} is a directory. Skipping.");
                        continue;
                    }

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
                    let result = cmd_tx
                        .send(ManagerCommand::DeviceRemoved {
                            device: device.into(),
                        })
                        .await;
                    if let Err(e) = result {
                        log::error!("Unable to send command: {:?}", e);
                    }
                }
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
        let led_devices = udev::discover_devices("leds")?;
        let led_devices = led_devices.into_iter().map(|dev| dev.into()).collect();
        Manager::discover_devices(cmd_tx, led_devices).await?;
        let tty_devices = udev::discover_devices("tty")?;
        let tty_devices = tty_devices.into_iter().map(|dev| dev.into()).collect();
        Manager::discover_devices(cmd_tx, tty_devices).await?;

        Ok(())
    }

    async fn discover_devices(
        manager_tx: &mpsc::Sender<ManagerCommand>,
        devices: Vec<UdevDevice>,
    ) -> Result<(), Box<dyn Error>> {
        for device in devices {
            manager_tx
                .send(ManagerCommand::DeviceAdded {
                    device: device.into(),
                })
                .await?;
        }

        Ok(())
    }

    /// Looks in all default locations for [CompositeDeviceConfig] definitions and
    /// load/parse them. Returns an array of these configs which can be used
    /// to automatically create a [CompositeDevice].
    pub async fn load_device_configs(&self) -> Vec<(PathBuf, CompositeDeviceConfig)> {
        let task = task::spawn_blocking(move || {
            log::trace!("Loading device configurations");
            let mut devices: Vec<(PathBuf, CompositeDeviceConfig)> = Vec::new();
            let paths = get_devices_paths();
            let files = get_multidir_sorted_files(paths.as_slice(), |entry| {
                entry.path().extension().unwrap_or_default() == "yaml"
            });

            // Look at each file in the directory and try to load them
            for file in files {
                // Try to load the composite device profile
                log::trace!("Found file: {}", file.display());
                let device = CompositeDeviceConfig::from_yaml_file(file.display().to_string());
                let device = match device {
                    Ok(dev) => dev,
                    Err(e) => {
                        log::warn!(
                            "Failed to parse composite device config '{}': {e}",
                            file.display()
                        );
                        continue;
                    }
                };
                devices.push((file, device));
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
    fn listen_on_dbus(&mut self) {
        let iface = ManagerInterface::new(self.tx.clone());
        self.dbus.register(iface);
    }

    async fn add_device_to_composite_device(
        &self,
        device: DeviceInfo,
        client: &CompositeDeviceClient,
    ) -> Result<(), Box<dyn Error>> {
        client.add_source_device(device).await?;
        Ok(())
    }

    /// Update the gamepad order and emit dbus signal about order change
    fn update_and_emit_gamepad_order(&mut self, order: Vec<String>) {
        // Set the new order
        self.target_gamepad_order = order.clone();

        // Update the gamepad order on the dbus interface
        let conn = self.dbus.connection().clone();
        tokio::task::spawn(async move {
            if let Err(e) = ManagerInterface::update_target_gamepad_order(&conn, order).await {
                log::warn!("Failed to emit gamepad order changed signal: {e}");
            }
        });
    }

    /// Set the player order of the given composite device paths. Each device
    /// will be suspended and resumed in player order.
    async fn set_gamepad_order(&mut self, order: Vec<String>) {
        // If gamepad reordering is already in progress, requeue the request
        let tx = self.tx.clone();
        if self.target_gamepad_order_changing {
            log::debug!("Gamepad reordering in progress. Requeuing reordering request.");
            tokio::task::spawn(async move {
                let _ = tx
                    .send(ManagerCommand::SetGamepadOrder { dbus_paths: order })
                    .await;
            });
            return;
        }
        log::info!("Setting player order to: {order:?}");

        // Ensure the given paths are valid composite device paths
        let new_order: Vec<String> = order
            .into_iter()
            .filter(|path| {
                let is_valid = self.composite_devices.contains_key(path);
                if !is_valid {
                    log::error!("Invalid composite device path to set gamepad order: {path}");
                }
                is_valid
            })
            .collect();

        // Get each device to resume
        let devices: Vec<CompositeDeviceClient> = new_order
            .into_iter()
            .map(|path| self.composite_devices.get(&path).unwrap().clone())
            .collect();

        self.target_gamepad_order_changing = true;

        tokio::task::spawn(async move {
            // Suspend all composite devices
            for device in devices.iter() {
                if let Err(e) = device.suspend().await {
                    log::warn!("Failed to suspend device: {e}");
                }
            }

            // Sleep a little bit before resuming target devices
            tokio::time::sleep(Duration::from_millis(100)).await;

            // Resume all composite devices in order
            for device in devices.iter() {
                if let Err(e) = device.resume().await {
                    log::warn!("Failed to resume device: {e}");
                    continue;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }

            // Notify the manager that gamepad reordering has completed
            if let Err(e) = tx.send(ManagerCommand::GamepadReorderingFinished).await {
                log::error!("Failed to signal gamepad reordering finished. This is bad: {e:?}");
            }
        });
    }
}
