use std::collections::HashMap;
use std::fs;
use evdev::Device;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use zbus::fdo;
use zbus::Connection;
use zbus_macros::dbus_interface;

use super::watcher::WatchEvent;

const INPUT_PATH: &str = "/dev/input";

#[derive(Debug)]
pub enum Command {
    WatchEvent { event: WatchEvent },
    GetInterceptMode { resp: Responder<Option<String>> },
    //SetInterceptMode { mode: String, resp: Responder<()> },
}

/// Provided by the requester and used by the manager task to send
/// the command response back to the requester.
type Responder<T> = oneshot::Sender<Result<T, String>>;

/// Returns a new gamepad manager frontend and backend
pub fn new(tx: mpsc::Sender<Command>, rx: mpsc::Receiver<Command>) -> (Frontend, Backend) {
    let frontend = Frontend::new(tx);
    let backend = Backend::new(rx);

    return (frontend, backend);
}

/// Manages virtual controllers
///
/// The [Backend] discovers gamepads and interepts their input so
/// it can control what inputs should get passed on to the game and
/// what only an overlay should process. This works by grabbing exclusive
/// access to the physical gamepads and creating a virtual
/// gamepad that games can see.
///
/// SteamInput does this differently. It instead sets the 'SDL_GAMECONTROLLER_IGNORE_DEVICES'
/// environment variable whenever it launches a game to make the game ignore all
/// physical gamepads EXCEPT for Steam virtual gamepads.
/// https://github.com/godotengine/godot/pull/76045
pub struct Backend {
    rx: mpsc::Receiver<Command>,
}

impl Backend {
    /// Returns a new instance of Gamepad Manager
    pub fn new(rx: mpsc::Receiver<Command>) -> Backend {
        Backend { rx }
    }

    /// Start the backend and listen for command messages from the frontend
    /// and other clients.
    pub async fn run(&mut self, _connection: Connection) {
        while let Some(cmd) = self.rx.recv().await {
            log::debug!("Received command: {:?}", cmd);
            match cmd {
                Command::WatchEvent { event } => {
                    self.on_device_change(event);
                }
                Command::GetInterceptMode { resp } => {
                    let res: Result<Option<String>, String> =
                        Ok(Some(String::from("No intercept")));
                    // Ignore errors
                    let _ = resp.send(res);
                }
                //Command::SetInterceptMode { resp, mode } => {
                //    let _ = resp.send(Ok(()));
                //    //let res = client.set(&key, val).await;
                //    //// Ignore errors
                //    //let _ = resp.send(res);
                //}
            }
        }
    }

    /// Triggers whenever we detect any input connect/disconnect events
    pub fn on_device_change(&self, event: WatchEvent) {
        log::debug!("Got watch event: {:?}", event);
        match event {
            WatchEvent::Create { name, mask: _ } => {
                log::info!("Got create event: {}", name);
            }
            WatchEvent::Modify { name, mask: _ } => {
                log::info!("Got create event: {}", name);
            }
            WatchEvent::Delete { name, mask: _ } => {
                log::info!("Got create event: {}", name);
            }
            WatchEvent::Other {} => (),
        }
    }

    /// Returns an array of input devices discovered under '/dev/input'
    pub fn discover_devices(&self) -> HashMap<String, Device> {
        let mut devices: HashMap<String, Device> = HashMap::new();
        let paths = fs::read_dir(INPUT_PATH).unwrap();
        for path in paths {
            let path = path.unwrap();
            let filename = path.file_name();
            let filename = filename.as_os_str().to_str().unwrap();
            if !filename.starts_with("event") {
                log::debug!("Ignoring device: {}", path.path().display());
                continue;
            }
            let device = Device::open(path.path());
            if device.is_err() {
                let err = device.err().unwrap().to_string();
                log::debug!("Unable to open event device {}: {}", filename, err);
                continue;
            }
            let device = device.unwrap();

            log::info!("Discovered device: {}", device);
            devices.insert(path.path().display().to_string(), device);
        }

        return devices;
    }
}

/// The [Frontend] provides a DBus interface that can be exposed for managing
/// a [Backend]. It works by sending command messages to a channel that the
/// [Backend] is listening on.
pub struct Frontend {
    tx: mpsc::Sender<Command>,
}

impl Frontend {
    pub fn new(tx: mpsc::Sender<Command>) -> Frontend {
        Frontend { tx }
    }
}

#[dbus_interface(name = "org.shadowblip.GamepadManager")]
impl Frontend {
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
