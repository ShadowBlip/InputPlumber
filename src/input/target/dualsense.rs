//! Emulates a Sony DualSense gamepad as a target input device.
//! The DualSense implementation is based on the great work done by NeroReflex
//! and the ROGueENEMY project:
//! https://github.com/NeroReflex/ROGueENEMY/
use std::{cmp::Ordering, error::Error, fmt::Debug, fs::File, time::Duration};

use packed_struct::prelude::*;
use rand::Rng;
use uhid_virt::{Bus, CreateParams, StreamError, UHIDDevice};

use crate::{
    drivers::dualsense::{
        driver::{
            DS5_ACC_RES_PER_G, DS5_EDGE_NAME, DS5_EDGE_PID, DS5_EDGE_VERSION, DS5_EDGE_VID,
            DS5_NAME, DS5_PID, DS5_TOUCHPAD_HEIGHT, DS5_TOUCHPAD_WIDTH, DS5_VERSION, DS5_VID,
            FEATURE_REPORT_CALIBRATION, FEATURE_REPORT_FIRMWARE_INFO, FEATURE_REPORT_PAIRING_INFO,
            OUTPUT_REPORT_BT, OUTPUT_REPORT_BT_SIZE, OUTPUT_REPORT_USB,
            OUTPUT_REPORT_USB_SHORT_SIZE, OUTPUT_REPORT_USB_SIZE, STICK_X_MAX, STICK_X_MIN,
            STICK_Y_MAX, STICK_Y_MIN, TRIGGER_MAX,
        },
        hid_report::{
            Direction, PackedInputDataReport, USBPackedInputDataReport, UsbPackedOutputReport,
            UsbPackedOutputReportShort,
        },
        report_descriptor::{
            DS_BT_DESCRIPTOR, DS_EDGE_BT_DESCRIPTOR, DS_EDGE_USB_DESCRIPTOR, DS_USB_DESCRIPTOR,
        },
    },
    input::{
        capability::{
            Capability, Gamepad, GamepadAxis, GamepadButton, GamepadTrigger, Touch, TouchButton,
            Touchpad,
        },
        composite_device::client::CompositeDeviceClient,
        event::{
            native::{NativeEvent, ScheduledNativeEvent},
            value::InputValue,
            value::{denormalize_signed_value_u8, denormalize_unsigned_value_u8},
        },
        output_capability::{OutputCapability, LED},
        output_event::OutputEvent,
    },
};

use super::{InputError, OutputError, TargetInputDevice, TargetOutputDevice};

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
        //let mac_addr = [0x74, 0xe7, 0xd6, 0x3a, 0x47, 0xe8];
        let mut rng = rand::rng();
        let mac_addr: [u8; 6] = [
            rng.random(),
            rng.random(),
            rng.random(),
            rng.random(),
            rng.random(),
            rng.random(),
        ];
        log::debug!(
            "Creating new DualSense Edge device using MAC Address: {:?}",
            mac_addr
        );

        Self {
            model,
            bus_type,
            mac_addr,
        }
    }
}

impl Default for DualSenseHardware {
    fn default() -> Self {
        let mut rng = rand::rng();
        let mac_addr: [u8; 6] = [
            rng.random(),
            rng.random(),
            rng.random(),
            rng.random(),
            rng.random(),
            rng.random(),
        ];
        Self {
            model: ModelType::Normal,
            bus_type: BusType::Usb,
            mac_addr,
        }
    }
}

/// The [DualSenseDevice] is a target input device implementation that emulates
/// a Playstation DualSense controller using uhid.
pub struct DualSenseDevice {
    device: UHIDDevice<File>,
    state: PackedInputDataReport,
    timestamp: u8,
    hardware: DualSenseHardware,
    queued_events: Vec<ScheduledNativeEvent>,
}

impl DualSenseDevice {
    pub fn new(hardware: DualSenseHardware) -> Result<Self, Box<dyn Error>> {
        let device = DualSenseDevice::create_virtual_device(&hardware)?;
        Ok(Self {
            device,
            state: PackedInputDataReport::Usb(USBPackedInputDataReport::new()),
            timestamp: 0,
            hardware,
            queued_events: Vec::new(),
        })
    }

    /// Create the virtual device to emulate
    fn create_virtual_device(
        hardware: &DualSenseHardware,
    ) -> Result<UHIDDevice<File>, Box<dyn Error>> {
        let device = UHIDDevice::create(CreateParams {
            name: match hardware.model {
                ModelType::Edge => String::from(DS5_EDGE_NAME),
                ModelType::Normal => String::from(DS5_NAME),
            },
            phys: String::from(""),
            uniq: format!(
                "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
                hardware.mac_addr[5],
                hardware.mac_addr[4],
                hardware.mac_addr[3],
                hardware.mac_addr[2],
                hardware.mac_addr[1],
                hardware.mac_addr[0],
            ),
            bus: match hardware.bus_type {
                BusType::Bluetooth => Bus::BLUETOOTH,
                BusType::Usb => Bus::USB,
            },
            vendor: match hardware.model {
                ModelType::Edge => DS5_EDGE_VID as u32,
                ModelType::Normal => DS5_VID as u32,
            },
            product: match hardware.model {
                ModelType::Edge => DS5_EDGE_PID as u32,
                ModelType::Normal => DS5_PID as u32,
            },
            version: match hardware.model {
                ModelType::Edge => DS5_EDGE_VERSION as u32,
                ModelType::Normal => DS5_VERSION as u32,
            },
            country: 0,
            rd_data: match hardware.model {
                ModelType::Edge => match hardware.bus_type {
                    BusType::Bluetooth => DS_EDGE_BT_DESCRIPTOR.to_vec(),
                    BusType::Usb => DS_EDGE_USB_DESCRIPTOR.to_vec(),
                },
                ModelType::Normal => match hardware.bus_type {
                    BusType::Bluetooth => DS_BT_DESCRIPTOR.to_vec(),
                    BusType::Usb => DS_USB_DESCRIPTOR.to_vec(),
                },
            },
        })?;

        Ok(device)
    }

    /// Write the current device state to the device
    fn write_state(&mut self) -> Result<(), Box<dyn Error>> {
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

    /// Update the internal controller state when events are emitted.
    fn update_state(&mut self, event: NativeEvent) {
        let value = event.get_value();
        let capability = event.as_capability();
        let state = self.state.state_mut();
        match capability {
            Capability::None => (),
            Capability::NotImplemented => (),
            Capability::Sync => (),
            Capability::Gamepad(gamepad) => match gamepad {
                Gamepad::Button(btn) => match btn {
                    GamepadButton::South => state.cross = event.pressed(),
                    GamepadButton::East => state.circle = event.pressed(),
                    GamepadButton::North => state.square = event.pressed(),
                    GamepadButton::West => state.triangle = event.pressed(),
                    GamepadButton::Start => state.options = event.pressed(),
                    GamepadButton::Select => state.create = event.pressed(),
                    GamepadButton::Guide => state.ps = event.pressed(),
                    GamepadButton::QuickAccess => (),
                    GamepadButton::DPadUp => match state.dpad {
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
                    GamepadButton::DPadDown => match state.dpad {
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
                    GamepadButton::DPadLeft => match state.dpad {
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
                    GamepadButton::DPadRight => match state.dpad {
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
                    GamepadButton::LeftBumper => state.l1 = event.pressed(),
                    GamepadButton::LeftTrigger => state.l2 = event.pressed(),
                    GamepadButton::LeftPaddle1 => state.left_fn = event.pressed(),
                    GamepadButton::LeftPaddle2 => state.left_paddle = event.pressed(),
                    GamepadButton::LeftStick => state.l3 = event.pressed(),
                    GamepadButton::LeftStickTouch => (),
                    GamepadButton::RightBumper => state.r1 = event.pressed(),
                    GamepadButton::RightTrigger => state.r2 = event.pressed(),
                    GamepadButton::RightPaddle1 => state.right_fn = event.pressed(),
                    GamepadButton::RightPaddle2 => state.right_paddle = event.pressed(),
                    GamepadButton::RightStick => state.r3 = event.pressed(),
                    GamepadButton::RightStickTouch => (),
                    GamepadButton::LeftPaddle3 => (),
                    GamepadButton::RightPaddle3 => (),
                    GamepadButton::Mute => state.mute = event.pressed(),
                    GamepadButton::Screenshot => state.mute = event.pressed(),
                    _ => (),
                },
                Gamepad::Axis(axis) => match axis {
                    GamepadAxis::LeftStick => {
                        if let InputValue::Vector2 { x, y } = value {
                            if let Some(x) = x {
                                let value =
                                    denormalize_signed_value_u8(x, STICK_X_MIN, STICK_X_MAX);
                                state.joystick_l_x = value
                            }
                            if let Some(y) = y {
                                let value =
                                    denormalize_signed_value_u8(y, STICK_Y_MIN, STICK_Y_MAX);
                                state.joystick_l_y = value
                            }
                        }
                    }
                    GamepadAxis::RightStick => {
                        if let InputValue::Vector2 { x, y } = value {
                            if let Some(x) = x {
                                let value =
                                    denormalize_signed_value_u8(x, STICK_X_MIN, STICK_X_MAX);
                                state.joystick_r_x = value
                            }
                            if let Some(y) = y {
                                let value =
                                    denormalize_signed_value_u8(y, STICK_Y_MIN, STICK_Y_MAX);
                                state.joystick_r_y = value
                            }
                        }
                    }
                    GamepadAxis::Hat0 => {
                        if let InputValue::Vector2 { x, y } = value {
                            if let Some(x) = x {
                                let value = denormalize_signed_value_u8(x, -1.0, 1.0);
                                match value.cmp(&0) {
                                    Ordering::Less => match state.dpad {
                                        Direction::North => state.dpad = Direction::NorthWest,
                                        Direction::South => state.dpad = Direction::SouthWest,
                                        _ => state.dpad = Direction::West,
                                    },
                                    Ordering::Equal => match state.dpad {
                                        Direction::NorthWest => state.dpad = Direction::North,
                                        Direction::SouthWest => state.dpad = Direction::South,
                                        Direction::NorthEast => state.dpad = Direction::North,
                                        Direction::SouthEast => state.dpad = Direction::South,
                                        Direction::East => state.dpad = Direction::None,
                                        Direction::West => state.dpad = Direction::None,
                                        _ => (),
                                    },
                                    Ordering::Greater => match state.dpad {
                                        Direction::North => state.dpad = Direction::NorthEast,
                                        Direction::South => state.dpad = Direction::SouthEast,
                                        _ => state.dpad = Direction::East,
                                    },
                                }
                            }
                            if let Some(y) = y {
                                let value = denormalize_signed_value_u8(y, -1.0, 1.0);
                                match value.cmp(&0) {
                                    Ordering::Less => match state.dpad {
                                        Direction::East => state.dpad = Direction::NorthEast,
                                        Direction::West => state.dpad = Direction::NorthWest,
                                        _ => state.dpad = Direction::North,
                                    },
                                    Ordering::Equal => match state.dpad {
                                        Direction::NorthWest => state.dpad = Direction::West,
                                        Direction::SouthWest => state.dpad = Direction::West,
                                        Direction::NorthEast => state.dpad = Direction::East,
                                        Direction::SouthEast => state.dpad = Direction::East,
                                        Direction::North => state.dpad = Direction::None,
                                        Direction::South => state.dpad = Direction::None,
                                        _ => (),
                                    },
                                    Ordering::Greater => match state.dpad {
                                        Direction::East => state.dpad = Direction::SouthEast,
                                        Direction::West => state.dpad = Direction::SouthWest,
                                        _ => state.dpad = Direction::South,
                                    },
                                }
                            }
                        }
                    }
                    GamepadAxis::Hat1 => (),
                    GamepadAxis::Hat2 => (),
                    GamepadAxis::Hat3 => (),
                },
                Gamepad::Trigger(trigger) => match trigger {
                    GamepadTrigger::LeftTrigger => {
                        if let InputValue::Float(normal_value) = value {
                            let value = denormalize_unsigned_value_u8(normal_value, TRIGGER_MAX);
                            state.l2_trigger = value
                        }
                    }
                    GamepadTrigger::LeftTouchpadForce => (),
                    GamepadTrigger::LeftStickForce => (),
                    GamepadTrigger::RightTrigger => {
                        if let InputValue::Float(normal_value) = value {
                            let value = denormalize_unsigned_value_u8(normal_value, TRIGGER_MAX);
                            state.r2_trigger = value
                        }
                    }
                    GamepadTrigger::RightTouchpadForce => (),
                    GamepadTrigger::RightStickForce => (),
                },
                Gamepad::Accelerometer => {
                    if let InputValue::Vector3 { x, y, z } = value {
                        if let Some(x) = x {
                            state.accel_x = Integer::from_primitive(denormalize_accel_value(x))
                        }
                        if let Some(y) = y {
                            state.accel_y = Integer::from_primitive(denormalize_accel_value(y))
                        }
                        if let Some(z) = z {
                            state.accel_z = Integer::from_primitive(denormalize_accel_value(z))
                        }
                    }
                }
                Gamepad::Gyro => {
                    if let InputValue::Vector3 { x, y, z } = value {
                        if let Some(x) = x {
                            state.gyro_x = Integer::from_primitive(denormalize_gyro_value(x));
                        }
                        if let Some(y) = y {
                            state.gyro_y = Integer::from_primitive(denormalize_gyro_value(y))
                        }
                        if let Some(z) = z {
                            state.gyro_z = Integer::from_primitive(denormalize_gyro_value(z))
                        }
                    }
                }
                Gamepad::Dial(_) => (),
            },
            Capability::Touchpad(touch) => {
                match touch {
                    Touchpad::CenterPad(touch_event) => {
                        match touch_event {
                            Touch::Motion => {
                                if let InputValue::Touch {
                                    index,
                                    is_touching,
                                    pressure: _,
                                    x,
                                    y,
                                } = value
                                {
                                    // Check to see if this is the start of any touch
                                    let was_touching = state.touch_data.has_touches();

                                    let idx = index as usize;
                                    // TouchData has an array size of 2, ignore more than 2 touch events.
                                    if idx > 1 {
                                        return;
                                    }
                                    if let Some(x) = x {
                                        state.touch_data.touch_finger_data[idx]
                                            .set_x(denormalize_touch_value(x, DS5_TOUCHPAD_WIDTH));
                                    }
                                    if let Some(y) = y {
                                        state.touch_data.touch_finger_data[idx]
                                            .set_y(denormalize_touch_value(y, DS5_TOUCHPAD_HEIGHT));
                                    }

                                    if is_touching {
                                        state.touch_data.touch_finger_data[idx].context = 127;
                                    } else {
                                        state.touch_data.touch_finger_data[idx].context = 128;
                                    }

                                    // Reset the timestamp back to zero when all touches
                                    // have completed
                                    let now_touching = state.touch_data.has_touches();
                                    if was_touching && !now_touching {
                                        self.timestamp = 0;
                                    }
                                }
                            }
                            Touch::Button(button) => match button {
                                TouchButton::Touch => (),
                                TouchButton::Press => state.touchpad = event.pressed(),
                            },
                        }
                    }
                    // Not supported
                    Touchpad::RightPad(_) => {}

                    Touchpad::LeftPad(_) => {}
                }
            }
            Capability::Mouse(_) => (),
            Capability::Keyboard(_) => (),
            Capability::DBus(_) => (),
            Capability::Touchscreen(_) => (),
            Capability::Gyroscope(_) => (),
            Capability::Accelerometer(_) => (),
        };
    }

    /// Handle [OutputEvent::Output] events from the HIDRAW device. These are
    /// events which should be forwarded back to source devices.
    fn handle_output(&mut self, data: Vec<u8>) -> Result<Vec<OutputEvent>, Box<dyn Error>> {
        // Validate the output report size
        let _expected_report_size = match self.hardware.bus_type {
            BusType::Bluetooth => OUTPUT_REPORT_BT_SIZE,
            BusType::Usb => OUTPUT_REPORT_USB_SIZE,
        };

        // The first byte should be the report id
        let Some(report_id) = data.first() else {
            log::warn!("Received empty output report.");
            return Ok(vec![]);
        };

        log::debug!("Got output report with ID: {report_id}");

        match *report_id {
            OUTPUT_REPORT_USB => {
                log::debug!("Received USB output report with length: {}", data.len());
                let state = match data.len() {
                    OUTPUT_REPORT_USB_SIZE => {
                        let buf: [u8; OUTPUT_REPORT_USB_SIZE] = data.try_into().unwrap();
                        let report = UsbPackedOutputReport::unpack(&buf)?;
                        report.state
                    }
                    OUTPUT_REPORT_USB_SHORT_SIZE => {
                        let buf: [u8; OUTPUT_REPORT_USB_SHORT_SIZE] = data.try_into().unwrap();
                        let report = UsbPackedOutputReportShort::unpack(&buf)?;

                        // NOTE: Hack for supporting Steam Input rumble
                        let mut state = report.state;
                        if !state.allow_audio_control
                            && !state.allow_mic_volume
                            && !state.allow_speaker_volume
                            && !state.allow_headphone_volume
                            && !state.allow_left_trigger_ffb
                            && !state.allow_right_trigger_ffb
                            && !state.use_rumble_not_haptics
                            && !state.enable_rumble_emulation
                        {
                            state.use_rumble_not_haptics = true;
                        }
                        state
                    }
                    _ => {
                        log::warn!("Failed to unpack output report. Expected size {OUTPUT_REPORT_USB_SIZE} or {OUTPUT_REPORT_USB_SHORT_SIZE}, got {}.", data.len());
                        return Ok(vec![]);
                    }
                };

                log::trace!("{}", state);

                // Send the output report to the composite device so it can
                // be processed by source devices.
                let event = OutputEvent::DualSense(state);
                return Ok(vec![event]);
            }
            OUTPUT_REPORT_BT => {
                log::debug!(
                    "Received Bluetooth output report with length: {}",
                    data.len()
                );
                //
            }
            _ => {
                log::debug!("Unknown output report: {report_id}");
            }
        }

        Ok(vec![])
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
                    self.hardware.mac_addr[0],
                    self.hardware.mac_addr[1],
                    self.hardware.mac_addr[2],
                    self.hardware.mac_addr[3],
                    self.hardware.mac_addr[4],
                    self.hardware.mac_addr[5],
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
                if self.hardware.bus_type == BusType::Bluetooth {
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
                if self.hardware.bus_type == BusType::Bluetooth {
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
                if self.hardware.bus_type == BusType::Bluetooth {
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
}

impl TargetInputDevice for DualSenseDevice {
    fn write_event(&mut self, event: NativeEvent) -> Result<(), InputError> {
        log::trace!("Received event: {event:?}");
        // Check for QuickAccess, create chord for event.
        let cap = event.as_capability();
        if cap == Capability::Gamepad(Gamepad::Button(GamepadButton::QuickAccess)) {
            let pressed = event.pressed();
            let guide = NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::Guide)),
                event.get_value(),
            );
            let south = NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::South)),
                event.get_value(),
            );

            let (guide, south) = if pressed {
                let guide = ScheduledNativeEvent::new(guide, Duration::from_millis(0));
                let south = ScheduledNativeEvent::new(south, Duration::from_millis(160));
                (guide, south)
            } else {
                let guide = ScheduledNativeEvent::new(guide, Duration::from_millis(240));
                let south = ScheduledNativeEvent::new(south, Duration::from_millis(160));
                (guide, south)
            };

            self.queued_events.push(guide);
            self.queued_events.push(south);
            return Ok(());
        }
        self.update_state(event);

        // Check if the timestamp needs to be updated
        if self.state.state().touch_data.has_touches() {
            self.timestamp = self.timestamp.wrapping_add(3); // TODO: num?
            self.state.state_mut().touch_data.timestamp = self.timestamp;
        }

        Ok(())
    }

    fn get_capabilities(&self) -> Result<Vec<crate::input::capability::Capability>, InputError> {
        Ok(vec![
            Capability::Gamepad(Gamepad::Accelerometer),
            Capability::Gamepad(Gamepad::Axis(GamepadAxis::LeftStick)),
            Capability::Gamepad(Gamepad::Axis(GamepadAxis::RightStick)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::DPadDown)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::DPadLeft)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::DPadRight)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::DPadUp)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::East)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::Guide)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::LeftBumper)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::LeftPaddle1)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::LeftPaddle2)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::LeftStick)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::LeftTrigger)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::North)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::QuickAccess)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::RightBumper)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::RightPaddle1)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::RightPaddle2)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::RightStick)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::RightTrigger)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::Screenshot)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::Select)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::South)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::Start)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::West)),
            Capability::Gamepad(Gamepad::Gyro),
            Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::LeftTrigger)),
            Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::RightTrigger)),
            Capability::Touchpad(Touchpad::CenterPad(Touch::Button(TouchButton::Press))),
            Capability::Touchpad(Touchpad::CenterPad(Touch::Button(TouchButton::Touch))),
            Capability::Touchpad(Touchpad::CenterPad(Touch::Motion)),
        ])
    }

    /// Returns any events in the queue up to the [TargetDriver]
    fn scheduled_events(&mut self) -> Option<Vec<ScheduledNativeEvent>> {
        if self.queued_events.is_empty() {
            return None;
        }
        Some(self.queued_events.drain(..).collect())
    }

    fn stop(&mut self) -> Result<(), InputError> {
        let _ = self.device.destroy();
        Ok(())
    }

    /// Clear any local state on the target device.
    fn clear_state(&mut self) {
        self.state = match self.state {
            PackedInputDataReport::Usb(_) => PackedInputDataReport::Usb(Default::default()),
            PackedInputDataReport::Bluetooth(_) => {
                PackedInputDataReport::Bluetooth(Default::default())
            }
        };
    }
}

impl TargetOutputDevice for DualSenseDevice {
    /// Handle reading from the device and processing input events from source
    /// devices.
    /// https://www.kernel.org/doc/html/latest/hid/uhid.html#read
    fn poll(&mut self, _: &Option<CompositeDeviceClient>) -> Result<Vec<OutputEvent>, OutputError> {
        // Read output events
        let event = match self.device.read() {
            Ok(event) => event,
            Err(err) => match err {
                StreamError::Io(_e) => {
                    //log::error!("Error reading from UHID device: {e:?}");
                    // Write the current state
                    self.write_state()?;
                    return Ok(vec![]);
                }
                StreamError::UnknownEventType(e) => {
                    log::debug!("Unknown event type: {:?}", e);
                    // Write the current state
                    self.write_state()?;
                    return Ok(vec![]);
                }
            },
        };

        // Match the type of UHID output event
        let output_events = match event {
            // This is sent when the HID device is started. Consider this as an answer to
            // UHID_CREATE. This is always the first event that is sent.
            uhid_virt::OutputEvent::Start { dev_flags: _ } => {
                log::debug!("Start event received");
                Ok(vec![])
            }
            // This is sent when the HID device is stopped. Consider this as an answer to
            // UHID_DESTROY.
            uhid_virt::OutputEvent::Stop => {
                log::debug!("Stop event received");
                Ok(vec![])
            }
            // This is sent when the HID device is opened. That is, the data that the HID
            // device provides is read by some other process. You may ignore this event but
            // it is useful for power-management. As long as you haven't received this event
            // there is actually no other process that reads your data so there is no need to
            // send UHID_INPUT events to the kernel.
            uhid_virt::OutputEvent::Open => {
                log::debug!("Open event received");
                Ok(vec![])
            }
            // This is sent when there are no more processes which read the HID data. It is
            // the counterpart of UHID_OPEN and you may as well ignore this event.
            uhid_virt::OutputEvent::Close => {
                log::debug!("Close event received");
                Ok(vec![])
            }
            // This is sent if the HID device driver wants to send raw data to the I/O
            // device. You should read the payload and forward it to the device.
            uhid_virt::OutputEvent::Output { data } => {
                log::trace!("Got output data: {:?}", data);
                let result = self.handle_output(data);
                match result {
                    Ok(events) => Ok(events),
                    Err(e) => {
                        let err = format!("Failed process output event: {:?}", e);
                        Err(err.into())
                    }
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
            uhid_virt::OutputEvent::GetReport {
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
                Ok(vec![])
            }
            // This is the SET_REPORT equivalent of UHID_GET_REPORT. On receipt, you shall
            // send a SET_REPORT request to your HID device. Once it replies, you must tell
            // the kernel about it via UHID_SET_REPORT_REPLY.
            // The same restrictions as for UHID_GET_REPORT apply.
            uhid_virt::OutputEvent::SetReport {
                id,
                report_number,
                report_type,
                data,
            } => {
                log::debug!("Received SetReport event: id: {id}, num: {report_number}, type: {:?}, data: {:?}", report_type, data);
                if let Err(e) = self.device.write_set_report_reply(id, 0) {
                    log::warn!("Failed to write set report reply: {:?}", e);
                    return Err(e.to_string().into());
                }
                Ok(vec![])
            }
        };

        // Write the current state
        self.write_state()?;

        output_events
    }

    fn get_output_capabilities(&self) -> Result<Vec<OutputCapability>, OutputError> {
        Ok(vec![
            OutputCapability::ForceFeedback,
            OutputCapability::LED(LED::Color),
        ])
    }
}

impl Debug for DualSenseDevice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DualSenseDevice")
            .field("state", &self.state)
            .field("timestamp", &self.timestamp)
            .field("hardware", &self.hardware)
            .finish()
    }
}

/// De-normalizes the given value from 0.0 - 1.0 into a real value based on
/// the maximum axis range.
fn denormalize_touch_value(normal_value: f64, max: f64) -> u16 {
    (normal_value * max).round() as u16
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
