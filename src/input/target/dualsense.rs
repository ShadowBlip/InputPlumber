//! Emulates a Sony DualSense gamepad as a target input device.
//! The DualSense implementation is based on the great work done by NeroReflex
//! and the ROGueENEMY project:
//! https://github.com/NeroReflex/ROGueENEMY/
use std::{cmp::Ordering, error::Error, fs::File, thread, time};

use packed_struct::prelude::*;
use tokio::sync::{
    broadcast,
    mpsc::{self, error::TryRecvError},
};
use uhid_virt::{Bus, CreateParams, OutputEvent, StreamError, UHIDDevice};
use zbus::{fdo, Connection};
use zbus_macros::dbus_interface;

use crate::{
    drivers::dualsense::{
        driver::{
            DS5_ACC_RES_PER_G, DS5_EDGE_NAME, DS5_EDGE_PID, DS5_EDGE_VERSION, DS5_EDGE_VID,
            DS5_NAME, DS5_PID, DS5_VERSION, DS5_VID, FEATURE_REPORT_CALIBRATION,
            FEATURE_REPORT_FIRMWARE_INFO, FEATURE_REPORT_PAIRING_INFO, OUTPUT_REPORT_BT_SIZE,
            OUTPUT_REPORT_USB_SIZE, STICK_X_MAX, STICK_X_MIN, STICK_Y_MAX, STICK_Y_MIN,
            TRIGGER_MAX,
        },
        hid_report::{
            BluetoothPackedInputDataReport, Direction, PackedInputDataReport,
            USBPackedInputDataReport,
        },
        report_descriptor::{
            DS_BT_DESCRIPTOR, DS_EDGE_BT_DESCRIPTOR, DS_EDGE_USB_DESCRIPTOR, DS_USB_DESCRIPTOR,
        },
    },
    input::{
        capability::{Capability, Gamepad, GamepadAxis, GamepadButton, GamepadTrigger},
        composite_device::Command,
        event::{native::NativeEvent, value::InputValue},
    },
};

use super::TargetCommand;

const POLL_INTERVAL_MS: u64 = 4;
const BUFFER_SIZE: usize = 2048;

/// The type of DualSense device to emulate. Currently two models are supported:
/// DualSense and DualSense Edge.
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum ModelType {
    Normal,
    Edge,
}

/// The DualSense device can be emulated using either the USB or Bluetooth buses
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum BusType {
    Usb,
    Bluetooth,
}

/// The [DualSenseHardware] defines the kind of DualSense controller to emulate
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct DualSenseHardware {
    model: ModelType,
    bus_type: BusType,
    mac_addr: [u8; 6],
}

impl DualSenseHardware {
    pub fn new(model: ModelType, bus_type: BusType) -> Self {
        // "e8:47:3a:d6:e7:74"
        let mac_addr = [0x74, 0xe7, 0xd6, 0x3a, 0x47, 0xe8];
        Self {
            model,
            bus_type,
            mac_addr,
        }
    }
}

impl Default for DualSenseHardware {
    fn default() -> Self {
        Self {
            model: ModelType::Normal,
            bus_type: BusType::Usb,
            mac_addr: [0x74, 0xe7, 0xd6, 0x3a, 0x47, 0xe8],
        }
    }
}

/// The [DBusInterface] provides a DBus interface that can be exposed for managing
/// a [DualSenseDevice].
pub struct DBusInterface {
    hardware: DualSenseHardware,
}

impl DBusInterface {
    fn new(hardware: DualSenseHardware) -> DBusInterface {
        DBusInterface { hardware }
    }
}

#[dbus_interface(name = "org.shadowblip.Input.Gamepad")]
impl DBusInterface {
    /// Name of the DBus device
    #[dbus_interface(property)]
    async fn name(&self) -> fdo::Result<String> {
        match self.hardware.model {
            ModelType::Edge => match self.hardware.bus_type {
                BusType::Bluetooth => Ok("DualSense Edge (bluetooth)".into()),
                BusType::Usb => Ok("DualSense Edge".into()),
            },
            ModelType::Normal => match self.hardware.bus_type {
                BusType::Bluetooth => Ok("DualSense (bluetooth)".into()),
                BusType::Usb => Ok("DualSense".into()),
            },
        }
    }
}

/// The [DualSenseDevice] is a target input device implementation that emulates
/// a Playstation DualSense controller using uhid.
#[derive(Debug)]
pub struct DualSenseDevice {
    conn: Connection,
    dbus_path: Option<String>,
    tx: mpsc::Sender<TargetCommand>,
    rx: mpsc::Receiver<TargetCommand>,
    state: PackedInputDataReport,
    composite_tx: Option<broadcast::Sender<Command>>,
    hardware: DualSenseHardware,
}

impl DualSenseDevice {
    pub fn new(conn: Connection, hardware: DualSenseHardware) -> Self {
        let (tx, rx) = mpsc::channel(BUFFER_SIZE);
        Self {
            conn,
            dbus_path: None,
            tx,
            rx,
            state: PackedInputDataReport::Usb(USBPackedInputDataReport::new()),
            composite_tx: None,
            hardware,
        }
    }

    /// Returns a transmitter channel that can be used to send events to this device
    pub fn transmitter(&self) -> mpsc::Sender<TargetCommand> {
        self.tx.clone()
    }

    /// Configures the device to send output events to the given composite device
    /// channel.
    pub fn set_composite_device(&mut self, tx: broadcast::Sender<Command>) {
        self.composite_tx = Some(tx);
    }

    /// Creates a new instance of the dbus device interface on DBus.
    pub async fn listen_on_dbus(&mut self, path: String) -> Result<(), Box<dyn Error>> {
        let conn = self.conn.clone();
        self.dbus_path = Some(path.clone());
        let hw_config = self.hardware;
        tokio::spawn(async move {
            let iface = DBusInterface::new(hw_config);
            if let Err(e) = conn.object_server().at(path, iface).await {
                log::error!("Failed to setup DBus interface for Gamepad device: {:?}", e);
            }
        });
        Ok(())
    }

    /// Creates and runs the target device
    pub async fn run(&mut self) -> Result<(), Box<dyn Error>> {
        log::debug!("Creating virtual dualsense controller");
        let device = self.create_virtual_device()?;
        log::debug!("Spawning device thread for virtual controller");
        let device_tx = self.spawn_device_thread(device)?;

        // Listen for send events from source devices as dispatch those events
        // to the device thread.
        log::debug!("Started listening for events to send");
        while let Some(command) = self.rx.recv().await {
            match command {
                TargetCommand::SetCompositeDevice(tx) => {
                    self.set_composite_device(tx.clone());
                }
                TargetCommand::WriteEvent(event) => {
                    // Update internal state
                    self.update_state(event);

                    // Send the state to the virtual device
                    device_tx.send(self.state).await?;
                }
                TargetCommand::Stop => break,
            };
        }

        log::debug!("Stopped listening for events");

        Ok(())
    }

    /// Create the virtual device to emulate
    fn create_virtual_device(&self) -> Result<UHIDDevice<File>, Box<dyn Error>> {
        let device = UHIDDevice::create(CreateParams {
            name: match self.hardware.model {
                ModelType::Edge => String::from(DS5_EDGE_NAME),
                ModelType::Normal => String::from(DS5_NAME),
            },
            phys: String::from(""),
            uniq: format!(
                "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
                self.hardware.mac_addr[5],
                self.hardware.mac_addr[4],
                self.hardware.mac_addr[3],
                self.hardware.mac_addr[2],
                self.hardware.mac_addr[1],
                self.hardware.mac_addr[0],
            ),
            bus: match self.hardware.bus_type {
                BusType::Bluetooth => Bus::BLUETOOTH,
                BusType::Usb => Bus::USB,
            },
            vendor: match self.hardware.model {
                ModelType::Edge => DS5_EDGE_VID as u32,
                ModelType::Normal => DS5_VID as u32,
            },
            product: match self.hardware.model {
                ModelType::Edge => DS5_EDGE_PID as u32,
                ModelType::Normal => DS5_PID as u32,
            },
            version: match self.hardware.model {
                ModelType::Edge => DS5_EDGE_VERSION as u32,
                ModelType::Normal => DS5_VERSION as u32,
            },
            country: 0,
            rd_data: match self.hardware.model {
                ModelType::Edge => match self.hardware.bus_type {
                    BusType::Bluetooth => DS_EDGE_BT_DESCRIPTOR.to_vec(),
                    BusType::Usb => DS_EDGE_USB_DESCRIPTOR.to_vec(),
                },
                ModelType::Normal => match self.hardware.bus_type {
                    BusType::Bluetooth => DS_BT_DESCRIPTOR.to_vec(),
                    BusType::Usb => DS_USB_DESCRIPTOR.to_vec(),
                },
            },
        })?;

        Ok(device)
    }

    /// Spawn the device thread to handle reading and writing from/to the virtual
    /// hidraw device. Returns the sender side of the channel so input events
    /// from source devices can be sent to the device for processing.
    fn spawn_device_thread(
        &self,
        device: UHIDDevice<File>,
    ) -> Result<mpsc::Sender<PackedInputDataReport>, Box<dyn Error>> {
        log::debug!("Creating virtual dualsense controller");
        let (device_tx, device_rx) = mpsc::channel::<PackedInputDataReport>(BUFFER_SIZE);
        let hw_config = self.hardware;

        // Spawn the device in its own blocking thread
        tokio::task::spawn_blocking(move || {
            let mut hidraw_device = HidRawDevice::new(device, hw_config, device_rx);
            loop {
                if let Err(e) = hidraw_device.poll() {
                    log::warn!("Failed polling hidraw device: {:?}", e);
                    break;
                }

                let duration = time::Duration::from_millis(POLL_INTERVAL_MS);
                thread::sleep(duration);
            }

            log::debug!("Destroying HID device");
            if let Err(e) = hidraw_device.destroy() {
                log::error!("Failed to destroy device: {:?}", e);
            }
        });

        Ok(device_tx)
    }

    /// Update the internal controller state when events are emitted.
    fn update_state(&mut self, event: NativeEvent) {
        let value = event.get_value();
        let capability = event.as_capability();
        match capability {
            Capability::None => (),
            Capability::NotImplemented => (),
            Capability::Sync => (),
            Capability::Gamepad(gamepad) => match gamepad {
                Gamepad::Button(btn) => match btn {
                    GamepadButton::South => match self.state {
                        PackedInputDataReport::Usb(ref mut state) => state.cross = event.pressed(),
                        PackedInputDataReport::Bluetooth(ref mut state) => {
                            state.cross = event.pressed()
                        }
                    },
                    GamepadButton::East => match self.state {
                        PackedInputDataReport::Usb(ref mut state) => state.circle = event.pressed(),
                        PackedInputDataReport::Bluetooth(ref mut state) => {
                            state.circle = event.pressed()
                        }
                    },
                    GamepadButton::North => match self.state {
                        PackedInputDataReport::Usb(ref mut state) => state.square = event.pressed(),
                        PackedInputDataReport::Bluetooth(ref mut state) => {
                            state.square = event.pressed()
                        }
                    },
                    GamepadButton::West => match self.state {
                        PackedInputDataReport::Usb(ref mut state) => {
                            state.triangle = event.pressed()
                        }
                        PackedInputDataReport::Bluetooth(ref mut state) => {
                            state.triangle = event.pressed()
                        }
                    },
                    GamepadButton::Start => match self.state {
                        PackedInputDataReport::Usb(ref mut state) => {
                            state.options = event.pressed()
                        }
                        PackedInputDataReport::Bluetooth(ref mut state) => {
                            state.options = event.pressed()
                        }
                    },
                    GamepadButton::Select => match self.state {
                        PackedInputDataReport::Usb(ref mut state) => state.create = event.pressed(),
                        PackedInputDataReport::Bluetooth(ref mut state) => {
                            state.create = event.pressed()
                        }
                    },
                    GamepadButton::Guide => match self.state {
                        PackedInputDataReport::Usb(ref mut state) => state.ps = event.pressed(),
                        PackedInputDataReport::Bluetooth(ref mut state) => {
                            state.ps = event.pressed()
                        }
                    },
                    GamepadButton::QuickAccess => (),
                    GamepadButton::DPadUp => match self.state {
                        PackedInputDataReport::Usb(ref mut state) => match state.dpad {
                            Direction::North => {
                                if !event.pressed() {
                                    state.dpad = Direction::None
                                }
                            }
                            Direction::NorthEast => {
                                if !event.pressed() {
                                    state.dpad = Direction::East
                                }
                            }
                            Direction::East => {
                                if event.pressed() {
                                    state.dpad = Direction::NorthEast
                                }
                            }
                            Direction::SouthEast => {
                                if event.pressed() {
                                    state.dpad = Direction::NorthEast
                                }
                            }
                            Direction::South => {
                                if event.pressed() {
                                    state.dpad = Direction::North
                                }
                            }
                            Direction::SouthWest => {
                                if event.pressed() {
                                    state.dpad = Direction::NorthWest
                                }
                            }
                            Direction::West => {
                                if event.pressed() {
                                    state.dpad = Direction::NorthWest
                                }
                            }
                            Direction::NorthWest => {
                                if !event.pressed() {
                                    state.dpad = Direction::West
                                }
                            }
                            Direction::None => {
                                if event.pressed() {
                                    state.dpad = Direction::North
                                }
                            }
                        },
                        PackedInputDataReport::Bluetooth(ref mut state) => {
                            state.up = event.pressed()
                        }
                    },
                    GamepadButton::DPadDown => match self.state {
                        PackedInputDataReport::Usb(ref mut state) => match state.dpad {
                            Direction::North => {
                                if event.pressed() {
                                    state.dpad = Direction::South
                                }
                            }
                            Direction::NorthEast => {
                                if event.pressed() {
                                    state.dpad = Direction::SouthEast
                                }
                            }
                            Direction::East => {
                                if event.pressed() {
                                    state.dpad = Direction::SouthEast
                                }
                            }
                            Direction::SouthEast => {
                                if !event.pressed() {
                                    state.dpad = Direction::East
                                }
                            }
                            Direction::South => {
                                if !event.pressed() {
                                    state.dpad = Direction::None
                                }
                            }
                            Direction::SouthWest => {
                                if !event.pressed() {
                                    state.dpad = Direction::West
                                }
                            }
                            Direction::West => {
                                if event.pressed() {
                                    state.dpad = Direction::SouthWest
                                }
                            }
                            Direction::NorthWest => {
                                if event.pressed() {
                                    state.dpad = Direction::SouthWest
                                }
                            }
                            Direction::None => {
                                if event.pressed() {
                                    state.dpad = Direction::South
                                }
                            }
                        },
                        PackedInputDataReport::Bluetooth(ref mut state) => {
                            state.down = event.pressed()
                        }
                    },
                    GamepadButton::DPadLeft => match self.state {
                        PackedInputDataReport::Usb(ref mut state) => match state.dpad {
                            Direction::North => {
                                if event.pressed() {
                                    state.dpad = Direction::NorthWest
                                }
                            }
                            Direction::NorthEast => {
                                if event.pressed() {
                                    state.dpad = Direction::NorthWest
                                }
                            }
                            Direction::East => {
                                if event.pressed() {
                                    state.dpad = Direction::West
                                }
                            }
                            Direction::SouthEast => {
                                if event.pressed() {
                                    state.dpad = Direction::SouthWest
                                }
                            }
                            Direction::South => {
                                if event.pressed() {
                                    state.dpad = Direction::SouthWest
                                }
                            }
                            Direction::SouthWest => {
                                if !event.pressed() {
                                    state.dpad = Direction::South
                                }
                            }
                            Direction::West => {
                                if !event.pressed() {
                                    state.dpad = Direction::None
                                }
                            }
                            Direction::NorthWest => {
                                if !event.pressed() {
                                    state.dpad = Direction::North
                                }
                            }
                            Direction::None => {
                                if event.pressed() {
                                    state.dpad = Direction::West
                                }
                            }
                        },
                        PackedInputDataReport::Bluetooth(ref mut state) => {
                            state.left = event.pressed()
                        }
                    },
                    GamepadButton::DPadRight => match self.state {
                        PackedInputDataReport::Usb(ref mut state) => match state.dpad {
                            Direction::North => {
                                if event.pressed() {
                                    state.dpad = Direction::NorthEast
                                }
                            }
                            Direction::NorthEast => {
                                if !event.pressed() {
                                    state.dpad = Direction::North
                                }
                            }
                            Direction::East => {
                                if !event.pressed() {
                                    state.dpad = Direction::None
                                }
                            }
                            Direction::SouthEast => {
                                if !event.pressed() {
                                    state.dpad = Direction::South
                                }
                            }
                            Direction::South => {
                                if event.pressed() {
                                    state.dpad = Direction::SouthEast
                                }
                            }
                            Direction::SouthWest => {
                                if event.pressed() {
                                    state.dpad = Direction::SouthEast
                                }
                            }
                            Direction::West => {
                                if event.pressed() {
                                    state.dpad = Direction::East
                                }
                            }
                            Direction::NorthWest => {
                                if event.pressed() {
                                    state.dpad = Direction::NorthEast
                                }
                            }
                            Direction::None => {
                                if event.pressed() {
                                    state.dpad = Direction::East
                                }
                            }
                        },
                        PackedInputDataReport::Bluetooth(ref mut state) => {
                            state.right = event.pressed()
                        }
                    },
                    GamepadButton::LeftBumper => match self.state {
                        PackedInputDataReport::Usb(ref mut state) => state.l1 = event.pressed(),
                        PackedInputDataReport::Bluetooth(ref mut state) => {
                            state.l1 = event.pressed()
                        }
                    },
                    GamepadButton::LeftTrigger => match self.state {
                        PackedInputDataReport::Usb(ref mut state) => state.l2 = event.pressed(),
                        PackedInputDataReport::Bluetooth(ref mut state) => {
                            state.l2 = event.pressed()
                        }
                    },
                    GamepadButton::LeftPaddle1 => match self.state {
                        PackedInputDataReport::Usb(ref mut state) => {
                            state.left_paddle = event.pressed()
                        }
                        PackedInputDataReport::Bluetooth(_) => (),
                    },
                    GamepadButton::LeftPaddle2 => match self.state {
                        PackedInputDataReport::Usb(ref mut state) => {
                            state.left_fn = event.pressed()
                        }
                        PackedInputDataReport::Bluetooth(_) => (),
                    },
                    GamepadButton::LeftStick => match self.state {
                        PackedInputDataReport::Usb(ref mut state) => state.l3 = event.pressed(),
                        PackedInputDataReport::Bluetooth(ref mut state) => {
                            state.l3 = event.pressed()
                        }
                    },
                    GamepadButton::LeftStickTouch => (),
                    GamepadButton::LeftTouchpadTouch => (),
                    GamepadButton::LeftTouchpadPress => match self.state {
                        PackedInputDataReport::Usb(ref mut state) => {
                            state.touchpad = event.pressed()
                        }
                        PackedInputDataReport::Bluetooth(ref mut state) => {
                            state.touchpad = event.pressed()
                        }
                    },
                    GamepadButton::RightBumper => match self.state {
                        PackedInputDataReport::Usb(ref mut state) => state.r1 = event.pressed(),
                        PackedInputDataReport::Bluetooth(ref mut state) => {
                            state.r1 = event.pressed()
                        }
                    },
                    GamepadButton::RightTrigger => match self.state {
                        PackedInputDataReport::Usb(ref mut state) => state.r2 = event.pressed(),
                        PackedInputDataReport::Bluetooth(ref mut state) => {
                            state.r2 = event.pressed()
                        }
                    },
                    GamepadButton::RightPaddle1 => match self.state {
                        PackedInputDataReport::Usb(ref mut state) => {
                            state.right_paddle = event.pressed()
                        }
                        PackedInputDataReport::Bluetooth(_) => (),
                    },
                    GamepadButton::RightPaddle2 => match self.state {
                        PackedInputDataReport::Usb(ref mut state) => {
                            state.right_fn = event.pressed()
                        }
                        PackedInputDataReport::Bluetooth(_) => (),
                    },
                    GamepadButton::RightStick => match self.state {
                        PackedInputDataReport::Usb(ref mut state) => state.r3 = event.pressed(),
                        PackedInputDataReport::Bluetooth(ref mut state) => {
                            state.r3 = event.pressed()
                        }
                    },
                    GamepadButton::RightStickTouch => (),
                    GamepadButton::RightTouchpadTouch => (),
                    GamepadButton::RightTouchpadPress => match self.state {
                        PackedInputDataReport::Usb(ref mut state) => {
                            state.touchpad = event.pressed()
                        }
                        PackedInputDataReport::Bluetooth(ref mut state) => {
                            state.touchpad = event.pressed()
                        }
                    },
                    GamepadButton::LeftPaddle3 => (),
                    GamepadButton::RightPaddle3 => (),
                    _ => (),
                },
                Gamepad::Axis(axis) => match axis {
                    GamepadAxis::LeftStick => {
                        if let InputValue::Vector2 { x, y } = value {
                            if let Some(x) = x {
                                let value = denormalize_signed_value(x, STICK_X_MIN, STICK_X_MAX);
                                match self.state {
                                    PackedInputDataReport::Usb(ref mut state) => {
                                        state.joystick_l_x = value
                                    }
                                    PackedInputDataReport::Bluetooth(ref mut state) => {
                                        state.joystick_l_x = value
                                    }
                                }
                            }
                            if let Some(y) = y {
                                let value = denormalize_signed_value(y, STICK_Y_MIN, STICK_Y_MAX);
                                match self.state {
                                    PackedInputDataReport::Usb(ref mut state) => {
                                        state.joystick_l_y = value
                                    }
                                    PackedInputDataReport::Bluetooth(ref mut state) => {
                                        state.joystick_l_y = value
                                    }
                                }
                            }
                        }
                    }
                    GamepadAxis::RightStick => {
                        if let InputValue::Vector2 { x, y } = value {
                            if let Some(x) = x {
                                let value = denormalize_signed_value(x, STICK_X_MIN, STICK_X_MAX);
                                match self.state {
                                    PackedInputDataReport::Usb(ref mut state) => {
                                        state.joystick_r_x = value
                                    }
                                    PackedInputDataReport::Bluetooth(ref mut state) => {
                                        state.joystick_r_x = value
                                    }
                                }
                            }
                            if let Some(y) = y {
                                let value = denormalize_signed_value(y, STICK_Y_MIN, STICK_Y_MAX);
                                match self.state {
                                    PackedInputDataReport::Usb(ref mut state) => {
                                        state.joystick_r_y = value
                                    }
                                    PackedInputDataReport::Bluetooth(ref mut state) => {
                                        state.joystick_r_y = value
                                    }
                                }
                            }
                        }
                    }
                    GamepadAxis::Hat1 => {
                        if let InputValue::Vector2 { x, y } = value {
                            if let Some(x) = x {
                                let value = denormalize_signed_value(x, -1.0, 1.0);
                                match value.cmp(&0) {
                                    Ordering::Less => match self.state {
                                        PackedInputDataReport::Usb(ref mut state) => {
                                            match state.dpad {
                                                Direction::North => {
                                                    state.dpad = Direction::NorthWest
                                                }
                                                Direction::South => {
                                                    state.dpad = Direction::SouthWest
                                                }
                                                _ => state.dpad = Direction::West,
                                            }
                                        }
                                        PackedInputDataReport::Bluetooth(ref mut state) => {
                                            state.left = true;
                                            state.right = false;
                                        }
                                    },
                                    Ordering::Equal => match self.state {
                                        PackedInputDataReport::Usb(ref mut state) => {
                                            match state.dpad {
                                                Direction::NorthWest => {
                                                    state.dpad = Direction::North
                                                }
                                                Direction::SouthWest => {
                                                    state.dpad = Direction::South
                                                }
                                                Direction::NorthEast => {
                                                    state.dpad = Direction::North
                                                }
                                                Direction::SouthEast => {
                                                    state.dpad = Direction::South
                                                }
                                                Direction::East => state.dpad = Direction::None,
                                                Direction::West => state.dpad = Direction::None,
                                                _ => (),
                                            }
                                        }
                                        PackedInputDataReport::Bluetooth(ref mut state) => {
                                            state.left = false;
                                            state.right = false;
                                        }
                                    },
                                    Ordering::Greater => match self.state {
                                        PackedInputDataReport::Usb(ref mut state) => {
                                            match state.dpad {
                                                Direction::North => {
                                                    state.dpad = Direction::NorthEast
                                                }
                                                Direction::South => {
                                                    state.dpad = Direction::SouthEast
                                                }
                                                _ => state.dpad = Direction::East,
                                            }
                                        }
                                        PackedInputDataReport::Bluetooth(ref mut state) => {
                                            state.right = true;
                                            state.left = false;
                                        }
                                    },
                                }
                            }
                            if let Some(y) = y {
                                let value = denormalize_signed_value(y, -1.0, 1.0);
                                match value.cmp(&0) {
                                    Ordering::Less => match self.state {
                                        PackedInputDataReport::Usb(ref mut state) => {
                                            match state.dpad {
                                                Direction::East => {
                                                    state.dpad = Direction::NorthEast
                                                }
                                                Direction::West => {
                                                    state.dpad = Direction::NorthWest
                                                }
                                                _ => state.dpad = Direction::North,
                                            }
                                        }
                                        PackedInputDataReport::Bluetooth(ref mut state) => {
                                            state.up = true;
                                            state.down = false;
                                        }
                                    },
                                    Ordering::Equal => match self.state {
                                        PackedInputDataReport::Usb(ref mut state) => {
                                            match state.dpad {
                                                Direction::NorthWest => {
                                                    state.dpad = Direction::West
                                                }
                                                Direction::SouthWest => {
                                                    state.dpad = Direction::West
                                                }
                                                Direction::NorthEast => {
                                                    state.dpad = Direction::East
                                                }
                                                Direction::SouthEast => {
                                                    state.dpad = Direction::East
                                                }
                                                Direction::North => state.dpad = Direction::None,
                                                Direction::South => state.dpad = Direction::None,
                                                _ => (),
                                            }
                                        }
                                        PackedInputDataReport::Bluetooth(ref mut state) => {
                                            state.down = false;
                                            state.up = false;
                                        }
                                    },
                                    Ordering::Greater => match self.state {
                                        PackedInputDataReport::Usb(ref mut state) => {
                                            match state.dpad {
                                                Direction::East => {
                                                    state.dpad = Direction::SouthEast
                                                }
                                                Direction::West => {
                                                    state.dpad = Direction::SouthWest
                                                }
                                                _ => state.dpad = Direction::South,
                                            }
                                        }
                                        PackedInputDataReport::Bluetooth(ref mut state) => {
                                            state.down = true;
                                            state.up = false;
                                        }
                                    },
                                }
                            }
                        }
                    }
                    GamepadAxis::Hat2 => (),
                    GamepadAxis::Hat3 => (),
                    // TODO: Remove GamepadAxis::Buttons
                    // NativeEvent { capability: Gamepad(Axis(Buttons(DPadLeft, DPadRight))), source_capability: None, value: Vector2 { x: Some(1.0), y: None } }
                    GamepadAxis::Buttons(neg, _) => match neg {
                        GamepadButton::DPadUp =>
                        {
                            #[allow(clippy::collapsible_match)]
                            if let InputValue::Vector2 { x: _, y } = value {
                                if let Some(y) = y {
                                    let value = y as i8;
                                    match value.cmp(&0) {
                                        Ordering::Less => match self.state {
                                            PackedInputDataReport::Usb(ref mut state) => {
                                                match state.dpad {
                                                    Direction::East => {
                                                        state.dpad = Direction::NorthEast
                                                    }
                                                    Direction::West => {
                                                        state.dpad = Direction::NorthWest
                                                    }
                                                    _ => state.dpad = Direction::North,
                                                }
                                            }
                                            PackedInputDataReport::Bluetooth(ref mut state) => {
                                                state.up = true;
                                                state.down = false;
                                            }
                                        },
                                        Ordering::Equal => match self.state {
                                            PackedInputDataReport::Usb(ref mut state) => {
                                                match state.dpad {
                                                    Direction::NorthWest => {
                                                        state.dpad = Direction::West
                                                    }
                                                    Direction::SouthWest => {
                                                        state.dpad = Direction::West
                                                    }
                                                    Direction::NorthEast => {
                                                        state.dpad = Direction::East
                                                    }
                                                    Direction::SouthEast => {
                                                        state.dpad = Direction::East
                                                    }
                                                    Direction::North => {
                                                        state.dpad = Direction::None
                                                    }
                                                    Direction::South => {
                                                        state.dpad = Direction::None
                                                    }
                                                    _ => (),
                                                }
                                            }
                                            PackedInputDataReport::Bluetooth(ref mut state) => {
                                                state.down = false;
                                                state.up = false;
                                            }
                                        },
                                        Ordering::Greater => match self.state {
                                            PackedInputDataReport::Usb(ref mut state) => {
                                                match state.dpad {
                                                    Direction::East => {
                                                        state.dpad = Direction::SouthEast
                                                    }
                                                    Direction::West => {
                                                        state.dpad = Direction::SouthWest
                                                    }
                                                    _ => state.dpad = Direction::South,
                                                }
                                            }
                                            PackedInputDataReport::Bluetooth(ref mut state) => {
                                                state.down = true;
                                                state.up = false;
                                            }
                                        },
                                    }
                                }
                            }
                        }
                        GamepadButton::DPadLeft =>
                        {
                            #[allow(clippy::collapsible_match)]
                            if let InputValue::Vector2 { x, y: _ } = value {
                                if let Some(x) = x {
                                    let value = x as i8;
                                    match value.cmp(&0) {
                                        Ordering::Less => match self.state {
                                            PackedInputDataReport::Usb(ref mut state) => {
                                                match state.dpad {
                                                    Direction::North => {
                                                        state.dpad = Direction::NorthWest
                                                    }
                                                    Direction::South => {
                                                        state.dpad = Direction::SouthWest
                                                    }
                                                    _ => state.dpad = Direction::West,
                                                }
                                            }
                                            PackedInputDataReport::Bluetooth(ref mut state) => {
                                                state.left = true;
                                                state.right = false;
                                            }
                                        },
                                        Ordering::Equal => match self.state {
                                            PackedInputDataReport::Usb(ref mut state) => {
                                                match state.dpad {
                                                    Direction::NorthWest => {
                                                        state.dpad = Direction::North
                                                    }
                                                    Direction::SouthWest => {
                                                        state.dpad = Direction::South
                                                    }
                                                    Direction::NorthEast => {
                                                        state.dpad = Direction::North
                                                    }
                                                    Direction::SouthEast => {
                                                        state.dpad = Direction::South
                                                    }
                                                    Direction::East => state.dpad = Direction::None,
                                                    Direction::West => state.dpad = Direction::None,
                                                    _ => (),
                                                }
                                            }
                                            PackedInputDataReport::Bluetooth(ref mut state) => {
                                                state.left = false;
                                                state.right = false;
                                            }
                                        },
                                        Ordering::Greater => match self.state {
                                            PackedInputDataReport::Usb(ref mut state) => {
                                                match state.dpad {
                                                    Direction::North => {
                                                        state.dpad = Direction::NorthEast
                                                    }
                                                    Direction::South => {
                                                        state.dpad = Direction::SouthEast
                                                    }
                                                    _ => state.dpad = Direction::East,
                                                }
                                            }
                                            PackedInputDataReport::Bluetooth(ref mut state) => {
                                                state.right = true;
                                                state.left = false;
                                            }
                                        },
                                    }
                                }
                            }
                        }
                        _ => (),
                    },
                },
                Gamepad::Trigger(trigger) => match trigger {
                    GamepadTrigger::LeftTrigger => {
                        if let InputValue::Float(normal_value) = value {
                            let value = denormalize_unsigned_value(normal_value, TRIGGER_MAX);
                            match self.state {
                                PackedInputDataReport::Usb(ref mut state) => {
                                    state.l2_trigger = value
                                }
                                PackedInputDataReport::Bluetooth(ref mut state) => {
                                    state.l2_trigger = value
                                }
                            }
                        }
                    }
                    GamepadTrigger::LeftTouchpadForce => (),
                    GamepadTrigger::LeftStickForce => (),
                    GamepadTrigger::RightTrigger => {
                        if let InputValue::Float(normal_value) = value {
                            let value = denormalize_unsigned_value(normal_value, TRIGGER_MAX);
                            match self.state {
                                PackedInputDataReport::Usb(ref mut state) => {
                                    state.r2_trigger = value
                                }
                                PackedInputDataReport::Bluetooth(ref mut state) => {
                                    state.r2_trigger = value
                                }
                            }
                        }
                    }
                    GamepadTrigger::RightTouchpadForce => (),
                    GamepadTrigger::RightStickForce => (),
                },
                Gamepad::Accelerometer => {
                    if let InputValue::Vector3 { x, y, z } = value {
                        if let Some(x) = x {
                            match self.state {
                                PackedInputDataReport::Usb(ref mut state) => {
                                    state.accel_x =
                                        Integer::from_primitive(denormalize_accel_value(x))
                                }
                                PackedInputDataReport::Bluetooth(_) => (),
                            }
                        }
                        if let Some(y) = y {
                            match self.state {
                                PackedInputDataReport::Usb(ref mut state) => {
                                    state.accel_y =
                                        Integer::from_primitive(denormalize_accel_value(y))
                                }
                                PackedInputDataReport::Bluetooth(_) => (),
                            }
                        }
                        if let Some(z) = z {
                            match self.state {
                                PackedInputDataReport::Usb(ref mut state) => {
                                    state.accel_z =
                                        Integer::from_primitive(denormalize_accel_value(z))
                                }
                                PackedInputDataReport::Bluetooth(_) => (),
                            }
                        }
                    }
                }
                Gamepad::Gyro => {
                    if let InputValue::Vector3 { x, y, z } = value {
                        if let Some(x) = x {
                            match self.state {
                                PackedInputDataReport::Usb(ref mut state) => {
                                    state.gyro_x =
                                        Integer::from_primitive(denormalize_gyro_value(x));
                                }
                                PackedInputDataReport::Bluetooth(_) => (),
                            }
                        }
                        if let Some(y) = y {
                            match self.state {
                                PackedInputDataReport::Usb(ref mut state) => {
                                    state.gyro_y =
                                        Integer::from_primitive(denormalize_gyro_value(y))
                                }
                                PackedInputDataReport::Bluetooth(_) => (),
                            }
                        }
                        if let Some(z) = z {
                            match self.state {
                                PackedInputDataReport::Usb(ref mut state) => {
                                    state.gyro_z =
                                        Integer::from_primitive(denormalize_gyro_value(z))
                                }
                                PackedInputDataReport::Bluetooth(_) => (),
                            }
                        }
                    }
                }
            },
            Capability::Mouse(_) => (),
            Capability::Keyboard(_) => (),
            Capability::DBus(_) => (),
        };
    }
}

/// Structure for running the underlying HIDRAW device with uhid
struct HidRawDevice {
    device: UHIDDevice<File>,
    state: PackedInputDataReport,
    config: DualSenseHardware,
    event_rx: mpsc::Receiver<PackedInputDataReport>,
}

impl HidRawDevice {
    fn new(
        device: UHIDDevice<File>,
        config: DualSenseHardware,
        event_rx: mpsc::Receiver<PackedInputDataReport>,
    ) -> Self {
        let state = match config.bus_type {
            BusType::Bluetooth => {
                PackedInputDataReport::Bluetooth(BluetoothPackedInputDataReport::new())
            }
            BusType::Usb => PackedInputDataReport::Usb(USBPackedInputDataReport::new()),
        };

        Self {
            device,
            state,
            config,
            event_rx,
        }
    }

    /// Handle reading from the device and processing input events from source
    /// devices over the event channel
    /// https://www.kernel.org/doc/html/latest/hid/uhid.html#read
    fn poll(&mut self) -> Result<(), Box<dyn Error>> {
        let result = self.device.read();
        match result {
            Ok(event) => {
                match event {
                    // This is sent when the HID device is started. Consider this as an answer to
                    // UHID_CREATE. This is always the first event that is sent.
                    OutputEvent::Start { dev_flags: _ } => {
                        log::debug!("Start event received");
                    }
                    // This is sent when the HID device is stopped. Consider this as an answer to
                    // UHID_DESTROY.
                    OutputEvent::Stop => {
                        log::debug!("Stop event received");
                        return Err("HID device was destroyed".into());
                    }
                    // This is sent when the HID device is opened. That is, the data that the HID
                    // device provides is read by some other process. You may ignore this event but
                    // it is useful for power-management. As long as you haven't received this event
                    // there is actually no other process that reads your data so there is no need to
                    // send UHID_INPUT events to the kernel.
                    OutputEvent::Open => {
                        log::debug!("Open event received");
                    }
                    // This is sent when there are no more processes which read the HID data. It is
                    // the counterpart of UHID_OPEN and you may as well ignore this event.
                    OutputEvent::Close => {
                        log::debug!("Close event received");
                    }
                    // This is sent if the HID device driver wants to send raw data to the I/O
                    // device. You should read the payload and forward it to the device.
                    OutputEvent::Output { data } => {
                        log::trace!("Got output data: {:?}", data);
                        let result = self.handle_output(data);
                        if let Err(e) = result {
                            let err = format!("Failed process output event: {:?}", e);
                            return Err(err.into());
                        }
                    }
                    // This event is sent if the kernel driver wants to perform a GET_REPORT request
                    // on the control channel as described in the HID specs. The report-type and
                    // report-number are available in the payload.
                    // The kernel serializes GET_REPORT requests so there will never be two in
                    // parallel. However, if you fail to respond with a UHID_GET_REPORT_REPLY, the
                    // request might silently time out.
                    // Once you read a GET_REPORT request, you shall forward it to the HID device and
                    // remember the "id" field in the payload. Once your HID device responds to the
                    // GET_REPORT (or if it fails), you must send a UHID_GET_REPORT_REPLY to the
                    // kernel with the exact same "id" as in the request. If the request already
                    // timed out, the kernel will ignore the response silently. The "id" field is
                    // never re-used, so conflicts cannot happen.
                    OutputEvent::GetReport {
                        id,
                        report_number,
                        report_type,
                    } => {
                        log::trace!(
                            "Received GetReport event: id: {id}, num: {report_number}, type: {:?}",
                            report_type
                        );
                        let result = self.handle_get_report(id, report_number, report_type);
                        if let Err(e) = result {
                            let err = format!("Failed to process GetReport event: {:?}", e);
                            return Err(err.into());
                        }
                    }
                    // This is the SET_REPORT equivalent of UHID_GET_REPORT. On receipt, you shall
                    // send a SET_REPORT request to your HID device. Once it replies, you must tell
                    // the kernel about it via UHID_SET_REPORT_REPLY.
                    // The same restrictions as for UHID_GET_REPORT apply.
                    OutputEvent::SetReport {
                        id,
                        report_number,
                        report_type,
                        data,
                    } => {
                        log::trace!("Received SetReport event: id: {id}, num: {report_number}, type: {:?}, data: {:?}", report_type, data);
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

        // Try to receive input events from the channel until it is empty
        loop {
            match self.event_rx.try_recv() {
                Ok(new_state) => {
                    self.state = new_state;
                }
                Err(e) => match e {
                    TryRecvError::Empty => break,
                    TryRecvError::Disconnected => return Err("Disconnected".into()),
                },
            };
        }

        // Pack the state into a binary array
        match self.state {
            PackedInputDataReport::Usb(state) => {
                let data = state.pack()?;

                // Write the state to the virtual HID
                if let Err(e) = self.device.write(&data) {
                    let err = format!("Failed to write input data report: {:?}", e);
                    return Err(err.into());
                }
            }
            PackedInputDataReport::Bluetooth(state) => {
                let data = state.pack()?;

                // Write the state to the virtual HID
                if let Err(e) = self.device.write(&data) {
                    let err = format!("Failed to write input data report: {:?}", e);
                    return Err(err.into());
                }
            }
        };

        Ok(())
    }

    /// Handle [OutputEvent::Output] events from the HIDRAW device. These are
    /// events which should be forwarded back to source devices.
    fn handle_output(&mut self, data: Vec<u8>) -> Result<(), Box<dyn Error>> {
        // Validate the output report size
        let _expected_report_size = match self.config.bus_type {
            BusType::Bluetooth => OUTPUT_REPORT_BT_SIZE,
            BusType::Usb => OUTPUT_REPORT_USB_SIZE,
        };

        // The first byte should be the report id
        let Some(report_id) = data.first() else {
            log::warn!("Received empty output report.");
            return Ok(());
        };

        log::debug!("Got output report with ID: {report_id}");

        // TODO: Implement forwarding output data back to source devices to
        // control LEDs, rumble, etc.

        Ok(())
    }

    /// Handle [OutputEvent::GetReport] events from the HIDRAW device
    fn handle_get_report(
        &mut self,
        id: u32,
        report_number: u8,
        _report_type: uhid_virt::ReportType,
    ) -> Result<(), Box<dyn Error>> {
        // Handle report pairing requests
        let data = match report_number {
            // Pairing information report
            FEATURE_REPORT_PAIRING_INFO => {
                log::debug!("Got report pairing report request");
                // TODO: Can we define this somewhere as a const?
                let data = vec![
                    FEATURE_REPORT_PAIRING_INFO,
                    self.config.mac_addr[0],
                    self.config.mac_addr[1],
                    self.config.mac_addr[2],
                    self.config.mac_addr[3],
                    self.config.mac_addr[4],
                    self.config.mac_addr[5],
                    0x08,
                    0x25,
                    0x00,
                    0x1e,
                    0x00,
                    0xee,
                    0x74,
                    0xd0,
                    0xbc,
                    0x00,
                    0x00,
                    0x00,
                    0x00,
                ];

                // If this is a bluetooth gamepad, include the crc
                if self.config.bus_type == BusType::Bluetooth {
                    // TODO: Handle bluetooth CRC32
                }

                data
            }
            // Firmware information report
            FEATURE_REPORT_FIRMWARE_INFO => {
                log::debug!("Got report firmware info request");
                // TODO: Can we define this somewhere as a const?
                let data = vec![
                    FEATURE_REPORT_FIRMWARE_INFO,
                    0x4a,
                    0x75,
                    0x6e,
                    0x20,
                    0x31,
                    0x39,
                    0x20,
                    0x32,
                    0x30,
                    0x32,
                    0x33,
                    0x31,
                    0x34,
                    0x3a,
                    0x34,
                    0x37,
                    0x3a,
                    0x33,
                    0x34,
                    0x03,
                    0x00,
                    0x44,
                    0x00,
                    0x08,
                    0x02,
                    0x00,
                    0x01,
                    0x36,
                    0x00,
                    0x00,
                    0x01,
                    0xc1,
                    0xc8,
                    0x00,
                    0x00,
                    0x00,
                    0x00,
                    0x00,
                    0x00,
                    0x00,
                    0x00,
                    0x00,
                    0x00,
                    0x54,
                    0x01,
                    0x00,
                    0x00,
                    0x14,
                    0x00,
                    0x00,
                    0x00,
                    0x0b,
                    0x00,
                    0x01,
                    0x00,
                    0x06,
                    0x00,
                    0x00,
                    0x00,
                    0x00,
                    0x00,
                    0x00,
                    0x00,
                ];

                // If this is a bluetooth gamepad, include the crc
                if self.config.bus_type == BusType::Bluetooth {
                    // TODO: Handle bluetooth CRC32
                }

                data
            }
            // Calibration report
            FEATURE_REPORT_CALIBRATION => {
                log::debug!("Got report request for calibration");
                // TODO: Can we define this somewhere as a const?
                let data = vec![
                    FEATURE_REPORT_CALIBRATION,
                    0xff,
                    0xfc,
                    0xff,
                    0xfe,
                    0xff,
                    0x83,
                    0x22,
                    0x78,
                    0xdd,
                    0x92,
                    0x22,
                    0x5f,
                    0xdd,
                    0x95,
                    0x22,
                    0x6d,
                    0xdd,
                    0x1c,
                    0x02,
                    0x1c,
                    0x02,
                    0xf2,
                    0x1f,
                    0xed,
                    0xdf,
                    0xe3,
                    0x20,
                    0xda,
                    0xe0,
                    0xee,
                    0x1f,
                    0xdf,
                    0xdf,
                    0x0b,
                    0x00,
                    0x00,
                    0x00,
                    0x00,
                    0x00,
                    0x00,
                ];

                // If this is a bluetooth gamepad, include the crc
                if self.config.bus_type == BusType::Bluetooth {
                    // TODO: Handle bluetooth CRC32
                }

                data
            }
            _ => {
                let err = format!("Unknown get report request with report number: {report_number}");
                return Err(err.into());
            }
        };

        // Write the report reply to the HIDRAW device
        if let Err(e) = self.device.write_get_report_reply(id, 0, data) {
            log::warn!("Failed to write get report reply: {:?}", e);
            return Err(e.to_string().into());
        }

        Ok(())
    }

    /// Destroy the underlying HIDRAW device
    fn destroy(&mut self) -> Result<usize, std::io::Error> {
        self.device.destroy()
    }
}

/// Convert the given normalized value between -1.0 - 1.0 to the real value
/// based on the given minimum and maximum axis range. Playstation gamepads
/// use a range from 0-255, with 127 being the "nuetral" point.
fn denormalize_signed_value(normal_value: f64, min: f64, max: f64) -> u8 {
    let mid = (max + min) / 2.0;
    let normal_value_abs = normal_value.abs();
    if normal_value >= 0.0 {
        let maximum = max - mid;
        let value = normal_value * maximum + mid;
        value as u8
    } else {
        let minimum = min - mid;
        let value = normal_value_abs * minimum + mid;
        value as u8
    }
}

/// De-normalizes the given value from 0.0 - 1.0 into a real value based on
/// the maximum axis range.
fn denormalize_unsigned_value(normal_value: f64, max: f64) -> u8 {
    (normal_value * max).round() as u8
}

/// De-normalizes the given value in meters per second into a real value that
/// the DS5 controller understands.
/// DualSense accelerometer values are measured in [DS5_ACC_RES_PER_G]
/// units of G acceleration (1G == 9.8m/s). InputPlumber accelerometer
/// values are measured in units of meters per second. To denormalize
/// the value, it needs to be converted into G units (by dividing by 9.8),
/// then multiplying that value by the [DS5_ACC_RES_PER_G].
fn denormalize_accel_value(value_meters_sec: f64) -> i16 {
    let value_g = value_meters_sec / 9.8;
    let value = value_g * DS5_ACC_RES_PER_G as f64;
    value as i16
}

/// DualSense gyro values are measured in units of degrees per second.
/// InputPlumber gyro values are also measured in degrees per second.
fn denormalize_gyro_value(value_degrees_sec: f64) -> i16 {
    let value = value_degrees_sec;
    value as i16
}
