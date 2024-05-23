use std::{cmp::Ordering, error::Error, fs::File, thread, time};

use packed_struct::{
    types::{Integer, SizedInteger},
    PackedStruct,
};
use tokio::sync::mpsc::{self, error::TryRecvError};
use uhid_virt::{Bus, CreateParams, OutputEvent, StreamError, UHIDDevice};
use zbus::{fdo, Connection};
use zbus_macros::dbus_interface;

use crate::{
    dbus::interface::target::gamepad::TargetGamepadInterface,
    drivers::steam_deck::{
        driver::{PID, VID},
        hid_report::{PackedInputDataReport, STICK_X_MAX, STICK_X_MIN, STICK_Y_MAX, STICK_Y_MIN},
        report_descriptor::CONTROLLER_DESCRIPTOR,
    },
    input::{
        capability::{
            Capability, Gamepad, GamepadAxis, GamepadButton, Touch, TouchButton, Touchpad,
        },
        composite_device,
        event::{native::NativeEvent, value::InputValue},
        source::hidraw::steam_deck::CAPABILITIES,
    },
};

use super::TargetCommand;

const POLL_INTERVAL_MS: u64 = 4;
const BUFFER_SIZE: usize = 2048;

/// The [DBusInterface] provides a DBus interface that can be exposed for managing
/// a [SteamDeckDevice].
pub struct DBusInterface {}

impl DBusInterface {
    fn new() -> DBusInterface {
        DBusInterface {}
    }
}

#[dbus_interface(name = "org.shadowblip.Input.Gamepad")]
impl DBusInterface {
    /// Name of the DBus device
    #[dbus_interface(property)]
    async fn name(&self) -> fdo::Result<String> {
        Ok("Steam Deck Controller".into())
    }
}

#[derive(Debug)]
pub struct SteamDeckDevice {
    conn: Connection,
    dbus_path: Option<String>,
    tx: mpsc::Sender<TargetCommand>,
    rx: mpsc::Receiver<TargetCommand>,
    state: PackedInputDataReport,
    composite_tx: Option<mpsc::Sender<composite_device::Command>>,
}

impl SteamDeckDevice {
    pub fn new(conn: Connection) -> Self {
        let (tx, rx) = mpsc::channel(BUFFER_SIZE);
        Self {
            conn,
            dbus_path: None,
            tx,
            rx,
            state: PackedInputDataReport::new(),
            composite_tx: None,
        }
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
            let iface = DBusInterface::new();
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
        log::debug!("Creating virtual deck controller");
        let (device_tx, mut device_rx) = mpsc::channel::<PackedInputDataReport>(BUFFER_SIZE);
        let mut device = self.create_virtual_device()?;

        // Spawn the device in its own blocking thread
        tokio::task::spawn_blocking(move || {
            let mut frame: u32 = 0;
            let mut state = PackedInputDataReport::new();
            loop {
                // Handle reading from the device
                // https://www.kernel.org/doc/html/latest/hid/uhid.html#read
                let result = device.read();
                match result {
                    Ok(event) => {
                        match event {
                            OutputEvent::Start { dev_flags: _ } => {
                                log::debug!("Start event received");
                            }
                            OutputEvent::Stop => {
                                log::debug!("Stop event received");
                            }
                            OutputEvent::Open => {
                                log::debug!("Open event received");
                            }
                            OutputEvent::Close => {
                                log::debug!("Close event received");
                            }
                            OutputEvent::Output { data } => {
                                log::debug!("Got output data: {:?}", data);
                            }
                            OutputEvent::GetReport {
                                id,
                                report_number,
                                report_type,
                            } => {
                                log::debug!("Received GetReport event: id: {id}, num: {report_number}, type: {:?}", report_type);
                                let _ = device.write_get_report_reply(
                                    id,
                                    0,
                                    CONTROLLER_DESCRIPTOR.to_vec(),
                                );
                            }
                            OutputEvent::SetReport {
                                id,
                                report_number,
                                report_type,
                                data,
                            } => {
                                log::debug!("Received SetReport event: id: {id}, num: {report_number}, type: {:?}, data: {:?}", report_type, data);
                                let _ = device.write_set_report_reply(id, 0);
                            }
                        };
                    }
                    Err(err) => match err {
                        StreamError::Io(_e) => (),
                        StreamError::UnknownEventType(e) => {
                            log::debug!("Unknown event type: {:?}", e);
                        }
                    },
                };

                // Try to receive input events from the channel
                match device_rx.try_recv() {
                    Ok(new_state) => {
                        state = new_state;
                    }
                    Err(e) => match e {
                        TryRecvError::Empty => (),
                        TryRecvError::Disconnected => break,
                    },
                };

                // Update the frame counter every iteration
                frame += 1;
                state.frame = Integer::from_primitive(frame);

                // Pack the state into a binary array
                let data = state.pack();
                if let Err(e) = data {
                    log::debug!("Failed to pack input report: {:?}", e);
                    continue;
                }
                let data = data.unwrap();

                // Write the state to the virtual HID
                if let Err(e) = device.write(&data) {
                    log::error!("Failed to write input data report: {:?}", e);
                    break;
                }

                let duration = time::Duration::from_millis(POLL_INTERVAL_MS);
                thread::sleep(duration);
            }

            log::debug!("Destroying HID device");
            if let Err(e) = device.destroy() {
                log::error!("Failed to destroy device: {:?}", e);
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
                    // Update internal state
                    self.update_state(event);

                    // Send the state to the device
                    device_tx.send(self.state).await?;
                }
                TargetCommand::GetCapabilities(tx) => {
                    let caps = CAPABILITIES.to_vec();
                    if let Err(e) = tx.send(caps).await {
                        log::error!("Failed to send target capabilities: {e:?}");
                    }
                }
                TargetCommand::Stop => break,
            };
        }
        log::debug!("Stopped listening for events");

        // Remove the DBus interface
        if let Some(path) = self.dbus_path.clone() {
            let conn = self.conn.clone();
            let path = path.clone();
            tokio::task::spawn(async move {
                log::debug!("Stopping dbus interface for {path}");
                let result = conn
                    .object_server()
                    .remove::<TargetGamepadInterface, String>(path.clone())
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

    /// Create the virtual device to emulate
    fn create_virtual_device(&self) -> Result<UHIDDevice<File>, Box<dyn Error>> {
        let device = UHIDDevice::create(CreateParams {
            name: String::from("Valve Software Steam Controller"),
            phys: String::from(""),
            uniq: String::from(""),
            bus: Bus::USB,
            vendor: VID as u32,
            product: PID as u32,
            version: 0,
            country: 0,
            rd_data: CONTROLLER_DESCRIPTOR.to_vec(),
        })?;

        Ok(device)
    }

    /// Update the internal controller state when events are emitted.
    fn update_state(&mut self, event: NativeEvent) {
        let value = event.get_value();
        let capability = event.as_capability();
        match capability {
            Capability::None => (),
            Capability::NotImplemented => (),
            Capability::Sync => (),
            Capability::DBus(_) => (),
            Capability::Gamepad(gamepad) => match gamepad {
                Gamepad::Button(btn) => match btn {
                    GamepadButton::South => self.state.a = event.pressed(),
                    GamepadButton::East => self.state.b = event.pressed(),
                    GamepadButton::North => self.state.x = event.pressed(),
                    GamepadButton::West => self.state.y = event.pressed(),
                    GamepadButton::Start => self.state.menu = event.pressed(),
                    GamepadButton::Select => self.state.options = event.pressed(),
                    GamepadButton::Guide => self.state.steam = event.pressed(),
                    GamepadButton::QuickAccess => self.state.quick_access = event.pressed(),
                    GamepadButton::DPadUp => self.state.up = event.pressed(),
                    GamepadButton::DPadDown => self.state.down = event.pressed(),
                    GamepadButton::DPadLeft => self.state.left = event.pressed(),
                    GamepadButton::DPadRight => self.state.right = event.pressed(),
                    GamepadButton::LeftBumper => self.state.l1 = event.pressed(),
                    GamepadButton::LeftTrigger => self.state.l2 = event.pressed(),
                    GamepadButton::LeftPaddle1 => self.state.l4 = event.pressed(),
                    GamepadButton::LeftPaddle2 => self.state.l5 = event.pressed(),
                    GamepadButton::LeftStick => self.state.l3 = event.pressed(),
                    GamepadButton::LeftStickTouch => self.state.l_stick_touch = event.pressed(),
                    GamepadButton::RightBumper => self.state.r1 = event.pressed(),
                    GamepadButton::RightTrigger => self.state.r2 = event.pressed(),
                    GamepadButton::RightPaddle1 => self.state.r4 = event.pressed(),
                    GamepadButton::RightPaddle2 => self.state.r5 = event.pressed(),
                    GamepadButton::RightStick => self.state.r3 = event.pressed(),
                    GamepadButton::RightStickTouch => self.state.r_stick_touch = event.pressed(),
                    GamepadButton::LeftPaddle3 => (),
                    GamepadButton::RightPaddle3 => (),
                    _ => (),
                },
                Gamepad::Axis(axis) => match axis {
                    GamepadAxis::LeftStick => match value {
                        InputValue::None => (),
                        InputValue::Bool(_) => (),
                        InputValue::Float(_) => (),
                        InputValue::Vector2 { x, y } => {
                            if let Some(x) = x {
                                let value = denormalize_signed_value(x, STICK_X_MIN, STICK_X_MAX);
                                self.state.l_stick_x = Integer::from_primitive(value);
                            }
                            if let Some(y) = y {
                                let value = denormalize_signed_value(y, STICK_Y_MIN, STICK_Y_MAX);
                                self.state.l_stick_y = Integer::from_primitive(-value);
                            }
                        }
                        InputValue::Vector3 { x, y, z } => (),
                        InputValue::Touch {
                            index,
                            is_touching: pressed,
                            x,
                            y,
                        } => todo!(),
                    },
                    GamepadAxis::RightStick => match value {
                        InputValue::None => (),
                        InputValue::Bool(_) => (),
                        InputValue::Float(_) => (),
                        InputValue::Vector2 { x, y } => {
                            if let Some(x) = x {
                                let value = denormalize_signed_value(x, STICK_X_MIN, STICK_X_MAX);
                                self.state.r_stick_x = Integer::from_primitive(value);
                            }
                            if let Some(y) = y {
                                let value = denormalize_signed_value(y, STICK_Y_MIN, STICK_Y_MAX);
                                self.state.r_stick_y = Integer::from_primitive(-value);
                            }
                        }
                        InputValue::Vector3 { x, y, z } => (),
                        InputValue::Touch {
                            index,
                            is_touching: pressed,
                            x,
                            y,
                        } => (),
                    },
                    GamepadAxis::Hat1 => match value {
                        InputValue::None => (),
                        InputValue::Bool(_) => (),
                        InputValue::Float(_) => (),
                        InputValue::Vector2 { x, y } => {
                            if let Some(x) = x {
                                let value = denormalize_signed_value(x, -1.0, 1.0);
                                match value.cmp(&0) {
                                    Ordering::Less => {
                                        self.state.left = true;
                                        self.state.right = false;
                                    }
                                    Ordering::Equal => {
                                        self.state.left = false;
                                        self.state.right = false;
                                    }
                                    Ordering::Greater => {
                                        self.state.right = true;
                                        self.state.left = false;
                                    }
                                }
                            }
                            if let Some(y) = y {
                                let value = denormalize_signed_value(y, -1.0, 1.0);
                                match value.cmp(&0) {
                                    Ordering::Less => {
                                        self.state.up = true;
                                        self.state.down = false;
                                    }
                                    Ordering::Equal => {
                                        self.state.down = false;
                                        self.state.up = false;
                                    }
                                    Ordering::Greater => {
                                        self.state.down = true;
                                        self.state.up = false;
                                    }
                                }
                            }
                        }
                        InputValue::Vector3 { x: _, y: _, z: _ } => (),
                        InputValue::Touch {
                            index: _,
                            is_touching: _,
                            x: _,
                            y: _,
                        } => (),
                    },
                    GamepadAxis::Hat2 => (),
                    GamepadAxis::Hat3 => (),
                    GamepadAxis::Buttons(_, _) => (),
                },
                Gamepad::Trigger(_) => (),
                Gamepad::Accelerometer => (),
                Gamepad::Gyro => (),
            },
            Capability::Mouse(_) => (),
            Capability::Keyboard(_) => (),
            Capability::Touchpad(touch) => match touch {
                Touchpad::LeftPad(touch_event) => match touch_event {
                    Touch::Motion => match value {
                        InputValue::None => (),
                        InputValue::Bool(_) => (),
                        InputValue::Float(_) => (),
                        InputValue::Vector2 { x: _, y: _ } => (),
                        InputValue::Vector3 { x: _, y: _, z: _ } => (),
                        InputValue::Touch {
                            index: _,
                            is_touching: _,
                            x,
                            y,
                        } => {
                            if let Some(x) = x {
                                let value = denormalize_unsigned_value(x, 1.0);
                                let value = value as i16;
                                self.state.l_pad_x = Integer::from_primitive(value);
                            };
                            if let Some(y) = y {
                                let value = denormalize_unsigned_value(y, 1.0);
                                let value = value as i16;
                                self.state.l_pad_y = Integer::from_primitive(value);
                            };
                        }
                    },
                    Touch::Button(button) => match button {
                        TouchButton::Touch => self.state.l_pad_touch = event.pressed(),
                        TouchButton::Press => self.state.l_pad_press = event.pressed(),
                    },
                },
                Touchpad::RightPad(touch_event) => match touch_event {
                    Touch::Motion => match value {
                        InputValue::None => (),
                        InputValue::Bool(_) => (),
                        InputValue::Float(_) => (),
                        InputValue::Vector2 { x: _, y: _ } => (),
                        InputValue::Vector3 { x: _, y: _, z: _ } => (),
                        InputValue::Touch {
                            index: _,
                            is_touching: _,
                            x,
                            y,
                        } => {
                            if let Some(x) = x {
                                let value = denormalize_signed_value(x, 0.0, 1.0);
                                self.state.r_pad_x = Integer::from_primitive(value);
                            };
                            if let Some(y) = y {
                                let value = denormalize_signed_value(y, 0.0, 1.0);
                                self.state.r_pad_y = Integer::from_primitive(value);
                            };
                        }
                    },
                    Touch::Button(button) => match button {
                        TouchButton::Touch => self.state.r_pad_touch = event.pressed(),
                        TouchButton::Press => self.state.r_pad_press = event.pressed(),
                    },
                },
                // Treat center pad as a right pad
                Touchpad::CenterPad(_) => (),
            },
        };
    }
}

/// Convert the given normalized value to the real value based on the given
/// minimum and maximum axis range.
fn denormalize_signed_value(normal_value: f64, min: f64, max: f64) -> i16 {
    let mid = (max + min) / 2.0;
    let normal_value_abs = normal_value.abs();
    if normal_value >= 0.0 {
        let maximum = max - mid;
        let value = normal_value * maximum + mid;
        value as i16
    } else {
        let minimum = min - mid;
        let value = normal_value_abs * minimum + mid;
        value as i16
    }
}

/// De-normalizes the given value from 0.0 - 1.0 into a real value based on
/// the maximum axis range.
fn denormalize_unsigned_value(normal_value: f64, max: f64) -> u16 {
    (normal_value * max).round() as u16
}
