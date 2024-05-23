use std::{collections::HashMap, error::Error, time::Duration};

use evdev::{
    uinput::{VirtualDevice, VirtualDeviceBuilder},
    AbsInfo, AbsoluteAxisCode, AttributeSet, InputEvent, KeyCode, RelativeAxisCode,
    SynchronizationCode, SynchronizationEvent,
};
use tokio::{
    sync::mpsc::{self, error::TryRecvError},
    time::Instant,
};
use zbus::Connection;

use crate::{
    dbus::interface::target::mouse::TargetMouseInterface,
    input::{
        capability::{Capability, Mouse, MouseButton},
        composite_device,
        event::{evdev::EvdevEvent, native::NativeEvent, value::InputValue},
    },
};

use super::TargetCommand;

/// Size of the target command channel buffer for processing events
const BUFFER_SIZE: usize = 2048;

/// Poll rate that the virtual mouse uses to process translated mouse events
const STATE_POLL_RATE: Duration = Duration::from_millis(16);

/// [MouseDevice] is a target virtual mouse that can be used to send mouse input
#[derive(Debug)]
pub struct MouseDevice {
    conn: Connection,
    dbus_path: Option<String>,
    tx: mpsc::Sender<TargetCommand>,
    rx: mpsc::Receiver<TargetCommand>,
    composite_tx: Option<mpsc::Sender<composite_device::Command>>,
}

impl MouseDevice {
    pub fn new(conn: Connection) -> Self {
        let (tx, rx) = mpsc::channel(BUFFER_SIZE);
        Self {
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

    /// Creates a new instance of the device interface on DBus.
    pub async fn listen_on_dbus(&mut self, path: String) -> Result<(), Box<dyn Error>> {
        log::debug!("Starting dbus interface on {path}");
        let conn = self.conn.clone();
        self.dbus_path = Some(path.clone());
        tokio::spawn(async move {
            log::debug!("Starting dbus interface: {path}");
            let iface = TargetMouseInterface::new();
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
        log::debug!("Creating virtual mouse");
        let mut device = self.create_virtual_device()?;
        let axis_map = HashMap::new();

        // Create a thread with the virtual mouse state
        let tx = self.tx.clone();
        let mut state = MouseMotionState::new(tx);
        let state_tx = state.transmitter();
        tokio::spawn(async move {
            // Create the state
            let mut interval = tokio::time::interval(STATE_POLL_RATE);
            let mut current_time = Instant::now();
            loop {
                // Calculate the delta between each tick
                let last_time = current_time;
                current_time = Instant::now();
                let delta = current_time - last_time;

                // Process the current mouse state
                if let Err(e) = state.process(delta).await {
                    log::debug!("Channel disconnected for processing mouse state: {:?}", e);
                    break;
                }
                interval.tick().await;
            }
        });

        // Listen for send events
        log::debug!("Started listening for events to send");
        while let Some(command) = self.rx.recv().await {
            match command {
                TargetCommand::SetCompositeDevice(tx) => {
                    self.set_composite_device(tx);
                }
                TargetCommand::WriteEvent(event) => {
                    log::trace!("Got event to emit: {:?}", event);

                    // Check if this event needs to be processed by the
                    // mouse state.
                    if event.is_translated()
                        && matches!(event.as_capability(), Capability::Mouse(Mouse::Motion))
                    {
                        log::trace!("Got translated mouse motion event: {:?}", event);
                        if let Some(tx) = state_tx.as_ref() {
                            if let Err(e) = tx.send(event).await {
                                log::warn!(
                                    "Failed to send translated event to mouse state: {:?}",
                                    e
                                );
                                continue;
                            }
                        }
                        continue;
                    }

                    // Translate and emit the event(s)
                    let evdev_events = self.translate_event(event, axis_map.clone());
                    device.emit(evdev_events.as_slice())?;
                    device.emit(&[SynchronizationEvent::new(
                        SynchronizationCode::SYN_REPORT,
                        0,
                    )
                    .into()])?;
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
                    .remove::<TargetMouseInterface, String>(path.clone())
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

    /// Translate the given native event into an evdev event
    fn translate_event(
        &self,
        event: NativeEvent,
        axis_map: HashMap<AbsoluteAxisCode, AbsInfo>,
    ) -> Vec<InputEvent> {
        EvdevEvent::from_native_event(event, axis_map)
            .into_iter()
            .map(|event| event.as_input_event())
            .collect()
    }

    /// Create the virtual device to emulate
    fn create_virtual_device(&self) -> Result<VirtualDevice, Box<dyn Error>> {
        let mut buttons = AttributeSet::<KeyCode>::new();
        buttons.insert(KeyCode::BTN_LEFT);
        buttons.insert(KeyCode::BTN_RIGHT);
        buttons.insert(KeyCode::BTN_MIDDLE);
        buttons.insert(KeyCode::BTN_SIDE);
        buttons.insert(KeyCode::BTN_EXTRA);
        let device = VirtualDeviceBuilder::new()?
            .name("InputPlumber Mouse")
            .with_keys(&buttons)?
            .with_relative_axes(&AttributeSet::from_iter([
                RelativeAxisCode::REL_X,
                RelativeAxisCode::REL_Y,
                RelativeAxisCode::REL_WHEEL,
                RelativeAxisCode::REL_HWHEEL,
            ]))?
            .build()?;

        Ok(device)
    }

    fn get_capabilities(&self) -> Vec<Capability> {
        vec![
            Capability::Mouse(Mouse::Button(MouseButton::Left)),
            Capability::Mouse(Mouse::Button(MouseButton::Right)),
            Capability::Mouse(Mouse::Button(MouseButton::Middle)),
            Capability::Mouse(Mouse::Button(MouseButton::Side)),
            Capability::Mouse(Mouse::Button(MouseButton::Extra)),
            Capability::Mouse(Mouse::Button(MouseButton::WheelUp)),
            Capability::Mouse(Mouse::Button(MouseButton::WheelDown)),
            Capability::Mouse(Mouse::Motion),
        ]
    }
}

/// The [MouseMotionState] keeps track of the mouse velocity from translated
/// input events (like a joystick), and sends mouse motion events to the
/// [MouseDevice] based on the current velocity.
#[derive(Debug)]
pub struct MouseMotionState {
    rx: mpsc::Receiver<NativeEvent>,
    tx: Option<mpsc::Sender<NativeEvent>>,
    device_tx: mpsc::Sender<TargetCommand>,
    mouse_remainder: (f64, f64),
    mouse_velocity: (f64, f64),
}

impl MouseMotionState {
    /// Create a new mouse motion state to keep track of mouse velocity.
    pub fn new(device_tx: mpsc::Sender<TargetCommand>) -> Self {
        let (tx, rx) = mpsc::channel(BUFFER_SIZE);
        Self {
            rx,
            tx: Some(tx),
            device_tx,
            mouse_remainder: (0.0, 0.0),
            mouse_velocity: (0.0, 0.0),
        }
    }

    /// Returns a transmitter that can be used to send events to process.
    pub fn transmitter(&mut self) -> Option<mpsc::Sender<NativeEvent>> {
        let tx = self.tx.clone();
        self.tx = None;
        tx
    }

    /// Move the mouse based on the given input event translation
    pub async fn process(&mut self, delta: Duration) -> Result<(), TryRecvError> {
        // Process any events that come over the channel from the device
        loop {
            match self.rx.try_recv() {
                Ok(event) => self.update_state(event),
                Err(err) => match err {
                    TryRecvError::Empty => break,
                    TryRecvError::Disconnected => return Err(err),
                },
            }
        }

        // Calculate how much the mouse should move based on the current mouse velocity
        let mut pixels_to_move = (0.0, 0.0);
        pixels_to_move.0 = delta.as_secs_f64() * self.mouse_velocity.0;
        pixels_to_move.1 = delta.as_secs_f64() * self.mouse_velocity.1;

        // Get the fractional value of the position so we can accumulate them
        // in between invocations
        let mut x = pixels_to_move.0 as i32; // E.g. 3.14 -> 3
        let mut y = pixels_to_move.1 as i32;
        self.mouse_remainder.0 += pixels_to_move.0 - x as f64;
        self.mouse_remainder.1 += pixels_to_move.1 - y as f64;

        // Keep track of relative mouse movements to keep around fractional values
        if self.mouse_remainder.0 >= 1.0 {
            x += 1;
            self.mouse_remainder.0 -= 1.0;
        }
        if self.mouse_remainder.0 <= -1.0 {
            x -= 1;
            self.mouse_remainder.0 += 1.0;
        }
        if self.mouse_remainder.1 >= 1.0 {
            y += 1;
            self.mouse_remainder.1 -= 1.0;
        }
        if self.mouse_remainder.1 <= -1.0 {
            y -= 1;
            self.mouse_remainder.1 += 1.0;
        }

        // Send events to the device if the mouse state has changed
        if x != 0 {
            let value = InputValue::Vector2 {
                x: Some(x as f64),
                y: None,
            };
            let event = NativeEvent::new(Capability::Mouse(Mouse::Motion), value);
            if let Err(e) = self.device_tx.send(TargetCommand::WriteEvent(event)).await {
                log::warn!("Failed to send write event: {:?}", e);
                return Err(TryRecvError::Disconnected);
            }
        }
        if y != 0 {
            let value = InputValue::Vector2 {
                x: None,
                y: Some(y as f64),
            };
            let event = NativeEvent::new(Capability::Mouse(Mouse::Motion), value);
            if let Err(e) = self.device_tx.send(TargetCommand::WriteEvent(event)).await {
                log::warn!("Failed to send write event: {:?}", e);
                return Err(TryRecvError::Disconnected);
            }
        }

        Ok(())
    }

    /// Processes the given mouse motion or button input event.
    fn update_state(&mut self, event: NativeEvent) {
        // Get the mouse position from the event value
        let value = event.get_value();
        let (x, y) = match value {
            InputValue::Vector2 { x, y } => (x, y),
            InputValue::Vector3 { x, y, z: _ } => (x, y),
            _ => (None, None),
        };

        // Update the mouse velocity
        if let Some(x) = x {
            self.mouse_velocity.0 = x;
            log::trace!("Updating mouse state: {:?}", self.mouse_velocity);
        }
        if let Some(y) = y {
            self.mouse_velocity.1 = y;
            log::trace!("Updating mouse state: {:?}", self.mouse_velocity);
        }
    }
}
