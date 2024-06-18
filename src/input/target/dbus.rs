use std::error::Error;

use tokio::sync::mpsc;
use zbus::Connection;

use crate::{
    dbus::interface::target::dbus::TargetDBusInterface,
    input::{
        capability::Capability,
        composite_device,
        event::{
            dbus::{Action, DBusEvent},
            native::NativeEvent,
        },
    },
};

use super::TargetCommand;

/// Size of the channel buffer for events
const BUFFER_SIZE: usize = 2048;
/// The threshold for axis inputs to be considered "pressed"
const AXIS_THRESHOLD: f64 = 0.35;

/// The internal emulated device state for tracking analog input
#[derive(Debug, Clone, Default)]
struct State {
    pressed_left: bool,
    pressed_right: bool,
    pressed_up: bool,
    pressed_down: bool,
}

/// The [DBusDevice] is a virtual input device that can emit input events. It
/// is primarily used when a [CompositeDevice] is using input interception to
/// divert inputs to an overlay over DBus.
#[derive(Debug)]
pub struct DBusDevice {
    state: State,
    conn: Connection,
    dbus_path: Option<String>,
    tx: mpsc::Sender<TargetCommand>,
    rx: mpsc::Receiver<TargetCommand>,
    composite_tx: Option<mpsc::Sender<composite_device::Command>>,
}

impl DBusDevice {
    // Create a new [DBusDevice] instance.
    pub fn new(conn: Connection) -> Self {
        let (tx, rx) = mpsc::channel(BUFFER_SIZE);
        Self {
            state: State::default(),
            conn,
            dbus_path: None,
            composite_tx: None,
            tx,
            rx,
        }
    }

    /// Returns the DBus path of this device
    pub fn get_dbus_path(&self) -> Option<String> {
        self.dbus_path.clone()
    }

    /// Returns a transmitter channel that can be used to send events to this device
    pub fn transmitter(&self) -> mpsc::Sender<TargetCommand> {
        self.tx.clone()
    }

    /// Configures the device to send output events to the given composite device
    /// channel.
    pub fn set_composite_device(&mut self, tx: mpsc::Sender<composite_device::Command>) {
        self.composite_tx = Some(tx);
    }

    /// Creates a new instance of the dbus device interface on DBus.
    pub async fn listen_on_dbus(&mut self, path: String) -> Result<(), Box<dyn Error>> {
        let conn = self.conn.clone();
        self.dbus_path = Some(path.clone());

        tokio::spawn(async move {
            log::debug!("Starting dbus interface: {path}");
            let iface = TargetDBusInterface::new();
            if let Err(e) = conn.object_server().at(path.clone(), iface).await {
                log::debug!("Failed to start dbus interface {path}: {e:?}");
            } else {
                log::debug!("Started dbus interface on {path}");
            }
        });

        Ok(())
    }

    /// Creates and runs the target device
    pub async fn run(&mut self) -> Result<(), Box<dyn Error>> {
        log::debug!("Creating virtual dbus device");

        // Listen for send events
        log::debug!("Started listening for events to send");
        while let Some(command) = self.rx.recv().await {
            match command {
                TargetCommand::SetCompositeDevice(tx) => {
                    self.set_composite_device(tx);
                }
                TargetCommand::WriteEvent(event) => {
                    //log::debug!("Got event to emit: {:?}", event);
                    let dbus_events = self.translate_event(event);
                    for dbus_event in dbus_events {
                        self.write_dbus_event(dbus_event).await?;
                    }
                }
                TargetCommand::GetCapabilities(tx) => {
                    let caps = self.get_capabilities();
                    if let Err(e) = tx.send(caps).await {
                        log::error!("Failed to send target capabilities: {e:?}");
                    }
                }
                TargetCommand::Stop => break,
            };
        }
        log::debug!("Stopping device");

        // Remove the DBus interface
        if let Some(path) = self.dbus_path.clone() {
            let conn = self.conn.clone();
            let path = path.clone();
            tokio::task::spawn(async move {
                log::debug!("Stopping dbus interface for {path}");
                let result = conn
                    .object_server()
                    .remove::<TargetDBusInterface, String>(path.clone())
                    .await;
                if let Err(e) = result {
                    log::error!("Failed to stop dbus interface {path}: {e:?}");
                } else {
                    log::debug!("Stopped dbus interface for {path}");
                }
            });
        }

        Ok(())
    }

    /// Translate the given native event into one or more dbus events
    fn translate_event(&mut self, event: NativeEvent) -> Vec<DBusEvent> {
        let mut translated = vec![];
        let events = DBusEvent::from_native_event(event);
        for mut event in events {
            // Axis input is a special case, where we need to keep track of the
            // current state of the axis, and only emit events whenever the axis
            // passes or falls below the defined threshold.
            let include_event = match event.action {
                Action::Left => {
                    if self.state.pressed_left && event.value < AXIS_THRESHOLD {
                        event.value = 0.0;
                        self.state.pressed_left = false;
                        true
                    } else if !self.state.pressed_left && event.value > AXIS_THRESHOLD {
                        event.value = 1.0;
                        self.state.pressed_left = true;
                        true
                    } else {
                        false
                    }
                }
                Action::Right => {
                    if self.state.pressed_right && event.value < AXIS_THRESHOLD {
                        event.value = 0.0;
                        self.state.pressed_right = false;
                        true
                    } else if !self.state.pressed_right && event.value > AXIS_THRESHOLD {
                        event.value = 1.0;
                        self.state.pressed_right = true;
                        true
                    } else {
                        false
                    }
                }
                Action::Up => {
                    if self.state.pressed_up && event.value < AXIS_THRESHOLD {
                        event.value = 0.0;
                        self.state.pressed_up = false;
                        true
                    } else if !self.state.pressed_up && event.value > AXIS_THRESHOLD {
                        event.value = 1.0;
                        self.state.pressed_up = true;
                        true
                    } else {
                        false
                    }
                }
                Action::Down => {
                    if self.state.pressed_down && event.value < AXIS_THRESHOLD {
                        event.value = 0.0;
                        self.state.pressed_down = false;
                        true
                    } else if !self.state.pressed_down && event.value > AXIS_THRESHOLD {
                        event.value = 1.0;
                        self.state.pressed_down = true;
                        true
                    } else {
                        false
                    }
                }
                _ => true,
            };

            if include_event {
                translated.push(event);
            }
        }

        translated
    }

    /// Writes the given event to DBus
    async fn write_dbus_event(&self, event: DBusEvent) -> Result<(), Box<dyn Error>> {
        // Only send valid events
        let valid = !matches!(event.action, Action::None);
        if !valid {
            return Ok(());
        }

        // DBus events can only be written if there is a DBus path reference.
        let Some(path) = self.dbus_path.clone() else {
            return Err("No dbus path exists to send events to".into());
        };

        let conn = self.conn.clone();
        // Get the object instance at the given path so we can send DBus signal
        // updates
        let iface_ref = conn
            .object_server()
            .interface::<_, TargetDBusInterface>(path.as_str())
            .await?;

        // Send the input event signal
        TargetDBusInterface::input_event(
            iface_ref.signal_context(),
            event.action.as_string(),
            event.value,
        )
        .await?;

        Ok(())
    }

    /// Returns capabilities of the target device
    fn get_capabilities(&self) -> Vec<Capability> {
        vec![
            Capability::DBus(Action::Guide),
            Capability::DBus(Action::Quick),
            Capability::DBus(Action::Quick2),
            Capability::DBus(Action::Context),
            Capability::DBus(Action::Option),
            Capability::DBus(Action::Select),
            Capability::DBus(Action::Accept),
            Capability::DBus(Action::Back),
            Capability::DBus(Action::ActOn),
            Capability::DBus(Action::Left),
            Capability::DBus(Action::Right),
            Capability::DBus(Action::Up),
            Capability::DBus(Action::Down),
            Capability::DBus(Action::L1),
            Capability::DBus(Action::L2),
            Capability::DBus(Action::L3),
            Capability::DBus(Action::R1),
            Capability::DBus(Action::R2),
            Capability::DBus(Action::R3),
            Capability::DBus(Action::VolumeUp),
            Capability::DBus(Action::VolumeDown),
            Capability::DBus(Action::VolumeMute),
            Capability::DBus(Action::Keyboard),
            Capability::DBus(Action::Screenshot),
        ]
    }
}
