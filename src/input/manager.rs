use std::error::Error;

use tokio::fs;
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use zbus::fdo;
use zbus::Connection;
use zbus_macros::dbus_interface;

use crate::constants::BUS_PREFIX;
use crate::watcher;

const DEV_PATH: &str = "/dev";
const INPUT_PATH: &str = "/dev/input";

/// Manager commands define all the different ways to interact with [Manager]
/// over a channel. These commands are processed in an asyncronous thread and
/// dispatched as they come in.
#[derive(Debug, Clone)]
pub enum Command {
    EventDeviceAdded { name: String },
    EventDeviceRemoved { name: String },
    HIDRawAdded { name: String },
    HIDRawRemoved { name: String },
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
        return Ok("Woo".to_string());
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
    /// List of all currently used gamepad paths
    /// E.g. ["/org/shadowblip/Gamepads/Gamepad0"]
    gamepad_dbus_paths: Vec<String>,
    /// List of all currently managed source devices.
    /// E.g. ["event0", "event2"]
    source_device_paths: Vec<String>,
}

impl Manager {
    /// Returns a new instance of Gamepad Manager
    pub fn new(conn: Connection) -> Manager {
        let (tx, rx) = broadcast::channel(32);
        Manager {
            dbus: conn,
            rx,
            tx,
            gamepad_dbus_paths: Vec::new(),
            source_device_paths: Vec::new(),
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
                Command::EventDeviceAdded { name } => self.on_event_device_added(name).await,
                Command::EventDeviceRemoved { name } => self.on_event_device_removed(name).await,
                Command::HIDRawAdded { name } => self.on_hidraw_added(name).await,
                Command::HIDRawRemoved { name } => self.on_hidraw_removed(name).await,
            }
        }

        Ok(())
    }

    /// Called when an event device (e.g. /dev/input/event5) is added
    async fn on_event_device_added(&self, name: String) {
        log::debug!("Event device added: {}", name);
    }

    /// Called when an event device (e.g. /dev/input/event5) is removed
    async fn on_event_device_removed(&self, name: String) {
        log::debug!("Event device removed: {}", name);
    }

    /// Called when a hidraw device (e.g. /dev/hidraw0) is added
    async fn on_hidraw_added(&self, name: String) {
        log::debug!("HIDRaw added: {}", name);
    }

    /// Called when a hidraw device (e.g. /dev/hidraw0) is removed
    async fn on_hidraw_removed(&self, name: String) {
        log::debug!("HIDRaw removed: {}", name);
    }

    /// Starts watching for input devices that are added and removed.
    async fn watch_input_devices(&self) -> Result<(), Box<dyn Error>> {
        // Create a channel to handle watch events
        let (watcher_tx, mut watcher_rx) = mpsc::channel(32);

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
        let mut paths = fs::read_dir(DEV_PATH).await?;
        while let Ok(Some(entry)) = paths.next_entry().await {
            let path = entry.file_name();
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
        let mut paths = fs::read_dir(INPUT_PATH).await?;
        while let Ok(Some(entry)) = paths.next_entry().await {
            let path = entry.file_name();
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
                        if base_path == INPUT_PATH {
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
                        if base_path == INPUT_PATH {
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

    /// Creates a DBus object
    async fn listen_on_dbus(&self) -> Result<(), Box<dyn Error>> {
        let iface = DBusInterface::new(self.tx.clone());
        let manager_path = format!("{}/Manager", BUS_PREFIX);
        self.dbus.object_server().at(manager_path, iface).await?;
        Ok(())
    }
}
