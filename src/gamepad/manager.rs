use evdev::Device;
use evdev::KeyCode;
use std::collections::HashMap;
use std::fs;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use zbus::fdo;
use zbus::zvariant::ObjectPath;
use zbus::Connection;
use zbus_macros::dbus_interface;

use crate::constants::PREFIX;
use crate::gamepad::source_device::SourceDevice;
use crate::input;

use super::managed_gamepad;
use super::watcher::WatchEvent;

const INPUT_PATH: &str = "/dev/input";

/// Manager commands define all the different ways to interact with [Manager]
/// over a channel. These commands are processed in an asyncronous thread and
/// dispatched as they come in.
#[derive(Debug)]
pub enum Command {
    WatchEvent { event: WatchEvent },
    GetInterceptMode { resp: Responder<Option<String>> },
    //SetInterceptMode { mode: String, resp: Responder<()> },
    ManagedGamepadRemoved { dbus_path: String },
    VirtualGamepadCreated { path: String },
}

/// Provided by the requester and used by the manager task to send
/// the command response back to the requester.
type Responder<T> = oneshot::Sender<Result<T, String>>;

/// Returns a new gamepad manager frontend and backend
pub fn new(tx: mpsc::Sender<Command>, rx: mpsc::Receiver<Command>) -> (DBusInterface, Manager) {
    let backend_tx = tx.clone();
    let frontend = DBusInterface::new(tx);
    let backend = Manager::new(rx, backend_tx);

    (frontend, backend)
}

/// Manages virtual controllers
///
/// The [Manager] discovers gamepads and interepts their input so
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
    /// The transmit side of the [rx] channel used to send [Command] messages.
    /// This can be cloned to allow child objects to communicate up to the
    /// manager.
    tx: mpsc::Sender<Command>,
    /// The receive side of the channel used to listen for [Command] messages
    /// from other objects.
    rx: mpsc::Receiver<Command>,
    /// List of all currently used gamepad paths
    /// E.g. ["/org/shadowblip/Gamepads/Gamepad0"]
    gamepad_dbus_paths: Vec<String>,
    /// List of all currently managed source devices.
    /// E.g. ["event0", "event2"]
    source_device_paths: Vec<String>,
}

impl Manager {
    /// Returns a new instance of Gamepad Manager
    pub fn new(rx: mpsc::Receiver<Command>, tx: mpsc::Sender<Command>) -> Manager {
        Manager {
            rx,
            tx,
            gamepad_dbus_paths: Vec::new(),
            source_device_paths: Vec::new(),
        }
    }

    /// Starts listening for [Command] messages to be sent from clients and
    /// dispatch those events.
    pub async fn run(&mut self, dbus_connection: Connection) {
        while let Some(cmd) = self.rx.recv().await {
            log::debug!("Received command: {:?}", cmd);
            match cmd {
                Command::WatchEvent { event } => {
                    self.on_device_change(&dbus_connection, event).await;
                }
                Command::GetInterceptMode { resp } => {
                    let res: Result<Option<String>, String> =
                        Ok(Some(String::from("No intercept")));
                    // Ignore errors
                    let _ = resp.send(res);
                }
                Command::ManagedGamepadRemoved { dbus_path } => {
                    let path = ObjectPath::from_string_unchecked(dbus_path.clone());
                    let res = dbus_connection
                        .object_server()
                        .remove::<managed_gamepad::DBusInterface, ObjectPath>(path)
                        .await;
                    match res {
                        Ok(_x) => {
                            self.gamepad_dbus_paths.retain(|path| path != &dbus_path);
                            log::debug!("Removed gamepad: {}", dbus_path);
                        }
                        Err(e) => log::error!("Failed to remove gamepad: {}", e),
                    }
                }
                Command::VirtualGamepadCreated { path: _ } => todo!(),
            }
        }
    }

    /// Triggers whenever we detect any input connect/disconnect events
    pub async fn on_device_change(&mut self, dbus_connection: &Connection, event: WatchEvent) {
        log::debug!("Got watch event: {:?}", event);
        // Don't update our state if this was a non-event change
        match event {
            WatchEvent::Create { name, mask: _ } => {
                log::info!("Got create event: {}", name);
                if !name.starts_with("event") {
                    return;
                }
            }
            WatchEvent::Modify { name, mask: _ } => {
                log::info!("Got modify event: {}", name);
                if !name.starts_with("event") {
                    return;
                }
            }
            WatchEvent::Delete { name, mask: _ } => {
                log::info!("Got delete event: {}", name);
                if !name.starts_with("event") {
                    return;
                }
            }
            WatchEvent::Other {} => (),
        }

        // Get all currently detected input devices from procfs
        let procfs_devices = input::device::get_all();
        if procfs_devices.is_err() {
            log::error!("Unable to read procfs input devices");
            return;
        }
        let procfs_devices = procfs_devices.unwrap();

        // Get a list of all currently detected event devices (e.g. ["event1", "event2"])
        let mut detected_handlers: Vec<String> = Vec::new();
        for device in procfs_devices {
            log::debug!("Detected device: {:?}", device);
            for handler in device.handlers {
                if !handler.starts_with("event") {
                    continue;
                }
                let path = format!("/dev/input/{}", handler);
                detected_handlers.push(path);
            }
        }
        log::debug!("Detected device handlers: {:?}", detected_handlers);

        // Delete any event handlers that were removed
        let mut to_remove: Vec<String> = Vec::new();
        for source_device_path in &self.source_device_paths {
            if detected_handlers.contains(source_device_path) {
                continue;
            }
            to_remove.push(source_device_path.to_string());
        }
        for item in to_remove.into_iter() {
            self.source_device_paths.retain(|p| p != &item);
            log::debug!("Source device removed: {}", item);
            log::debug!("Current source devices: {:?}", self.source_device_paths);
        }

        // Discover all connected input devices
        let mut discovered_devices = self.discover_devices();
        let mut discovered_gamepads: HashMap<String, SourceDevice> = HashMap::new();

        // Sort the discovered devices by their kind
        for (path, device) in discovered_devices.drain() {
            // Ignore devices that are already managed
            if self.source_device_paths.contains(&path) {
                log::debug!("Ignoring already managed device: {}", path);
                continue;
            }
            let source_device = SourceDevice::new(path.clone(), device);
            if source_device.is_virtual() {
                log::debug!("Ignoring device that apprears virtual: {}", path);
                continue;
            }
            if source_device.is_gamepad() {
                log::debug!("Considering gamepad: {}", path);
                discovered_gamepads.insert(path, source_device);
                continue;
            }
        }

        // Add any newly found gamepads
        for (path, gamepad) in discovered_gamepads.drain() {
            let mut devices: Vec<SourceDevice> = Vec::new();
            devices.push(gamepad);
            self.add_managed_gamepad(devices, dbus_connection).await;
        }
    }

    /// Creates and starts a managed gamepad from the given [SourceDevice] objects.
    async fn add_managed_gamepad(&mut self, devices: Vec<SourceDevice>, dbus: &Connection) {
        // Create the managed gamepad objects
        let dbus_path = self.next_gamepad_dbus_path();
        let (tx, rx) = mpsc::channel(1024);
        let gamepad_tx = tx.clone();
        let (gamepad_dbus, mut managed_gamepad) = managed_gamepad::new(tx, rx, self.tx.clone());

        // Serve the gamepad interfaces on DBus
        match dbus
            .object_server()
            .at(dbus_path.clone(), gamepad_dbus)
            .await
        {
            Ok(_x) => log::debug!("Served gamepad at path: {}", dbus_path),
            Err(e) => log::error!("Failed to serve gamepad: {}", e),
        };
        self.gamepad_dbus_paths.push(dbus_path.clone());

        // For each source device, start a thread to listen for input events to
        // send to the managed gamepad
        for device in devices.into_iter() {
            // Add the source device path to keep track of which devices are
            // being managed
            self.source_device_paths.push(device.path.clone());

            // Add the device as a source device for the managed gamepad
            let cmd = managed_gamepad::Command::AddSourceDevice { device };
            if gamepad_tx.send(cmd).await.is_err() {
                log::error!("Unable to add source device");
            };
        }

        // Create a virtual device from the managed gamepad
        let (resp_tx, resp_rx) = oneshot::channel();
        let cmd = managed_gamepad::Command::CreateVirtualDevice { resp: resp_tx };
        gamepad_tx.send(cmd).await;

        // Create a transmit channel so we can communicate events up to the
        // gamepad manager.
        let manager_tx = self.tx.clone();

        // Run the gamepad in its own thread to process events and commands
        let handle = tokio::spawn(async move {
            log::debug!("Starting managed gamepad");
            managed_gamepad.run().await;

            // Notify the gamepad manager that the managed gamepad has finished
            // running.
            let cmd = Command::ManagedGamepadRemoved { dbus_path };
            manager_tx.send(cmd).await;
        });

        // Collect the virtual device that was created by the managed gamepad
        // TODO: Do we even need this?
        let virtual_device_path = resp_rx.await;
        match virtual_device_path {
            Ok(path) => {
                let path = path.unwrap();
                log::debug!("Got virtual device path: {}", path);
                // TODO: Add this to a different list?
                self.source_device_paths.push(path);
            }
            Err(e) => log::debug!("Failed to get virtual device path: {}", e),
        }

        // Emit a signal about gamepad change
        //dbus_connection.emit_signal(destination, path, interface, signal_name, body)
    }

    /// Returns the next available gamepad dbus path
    fn next_gamepad_dbus_path(&self) -> String {
        let max = 1024;
        let mut i = 0;
        loop {
            if i > max {
                return "Devices exceeded".to_string();
            }
            let path = String::from(format!("{}/Gamepad{}", PREFIX, i));
            if self.gamepad_dbus_paths.contains(&path) {
                i += 1;
                continue;
            }
            return path;
        }
    }

    /// Returns true if the given evdev device is a gamepad
    fn is_gamepad(&self, device: &Device) -> bool {
        let supported = device
            .supported_keys()
            .map_or(false, |keys| keys.contains(KeyCode::BTN_MODE));
        return supported;
    }

    /// Returns an array of input devices discovered under '/dev/input'
    pub fn discover_devices(&self) -> HashMap<String, Device> {
        let mut devices: HashMap<String, Device> = HashMap::new();
        let paths = fs::read_dir(INPUT_PATH).unwrap();
        for path in paths {
            let path = path.unwrap();
            let filename = path.file_name();
            let filename = filename.as_os_str().to_str().unwrap();

            // Ignore non event devices
            if !filename.starts_with("event") {
                log::debug!("Ignoring device: {}", path.path().display());
                continue;
            }

            // Open the device
            let device = Device::open(path.path());
            if device.is_err() {
                let err = device.err().unwrap().to_string();
                log::debug!("Unable to open event device {}: {}", filename, err);
                continue;
            }
            let device = device.unwrap();

            //log::info!("Discovered device: {}", device);
            devices.insert(path.path().display().to_string(), device);
        }

        return devices;
    }
}

/// The [DBusInterface] provides a DBus interface that can be exposed for managing
/// a [Manager]. It works by sending command messages to a channel that the
/// [Manager] is listening on.
pub struct DBusInterface {
    tx: mpsc::Sender<Command>,
}

impl DBusInterface {
    pub fn new(tx: mpsc::Sender<Command>) -> DBusInterface {
        DBusInterface { tx }
    }
}

#[dbus_interface(name = "org.shadowblip.GamepadManager")]
impl DBusInterface {
    #[dbus_interface(property)]
    async fn intercept_mode(&self) -> fdo::Result<String> {
        let (resp_tx, resp_rx) = oneshot::channel();
        let cmd = Command::GetInterceptMode { resp: resp_tx };

        // Send the GET request to the backend
        self.tx.send(cmd).await.unwrap();

        // Await the response
        let res = resp_rx.await;
        println!("GOT = {:?}", res);

        return Ok("Woo".to_string());
    }
}
