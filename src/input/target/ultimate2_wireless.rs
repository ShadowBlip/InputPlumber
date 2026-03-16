//! Emulates an 8BitDo Ultimate 2 Wireless controller (DInput / hidraw mode) as a UHID target.
//! Steam identifies this device by VID=0x2DC8/PID=0x6012 and decodes the 34-byte HID input
//! report using fixed byte offsets from SDL_hidapi_8bitdo.c. Axis coordinate rotation and
//! IMU scale factors are also derived from that source.

use std::{cmp::Ordering, error::Error, fmt::Debug, fs::File, time::Duration};

use packed_struct::prelude::*;
use packed_struct::types::SizedInteger;
use uhid_virt::{Bus, CreateParams, StreamError, UHIDDevice};

use crate::{
    drivers::ultimate2_wireless::{
        driver::{
            ACCEL_SCALE, JOY_AXIS_MAX, JOY_AXIS_MIN, PID, REPORT_ID_RUMBLE,
            TRIGGER_AXIS_MAX, VID,
        },
        hid_report::{DpadDirection, PackedInputDataReport, PackedRumbleOutputReport},
        report_descriptor::REPORT_DESCRIPTOR,
    },
    input::{
        capability::{Capability, Gamepad, GamepadAxis, GamepadButton, GamepadTrigger},
        composite_device::client::CompositeDeviceClient,
        event::{
            native::{NativeEvent, ScheduledNativeEvent},
            value::InputValue,
            value::{denormalize_signed_value_u8, denormalize_unsigned_value_u8},
        },
        output_capability::OutputCapability,
        output_event::OutputEvent,
    },
};

use super::{InputError, OutputError, TargetInputDevice, TargetOutputDevice};

const GRAVITY: f64 = 9.80665;

pub struct Ultimate2WirelessDevice {
    device: UHIDDevice<File>,
    state: PackedInputDataReport,
    queued_events: Vec<ScheduledNativeEvent>,
}

impl Ultimate2WirelessDevice {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let device = UHIDDevice::create(CreateParams {
            name: String::from("8BitDo Ultimate 2 Wireless Controller"),
            phys: String::from(""),
            uniq: String::from(""),
            bus: Bus::USB,
            vendor: VID as u32,
            product: PID as u32,
            version: 0x0100,
            country: 0,
            rd_data: REPORT_DESCRIPTOR.to_vec(),
        })?;
        Ok(Self {
            device,
            state: PackedInputDataReport::default(),
            queued_events: Vec::new(),
        })
    }

    fn write_state(&mut self) -> Result<(), Box<dyn Error>> {
        let data = self.state.pack()?;
        self.device
            .write(&data)
            .map(|_| ())
            .map_err(|e| format!("Failed to write input data report: {e:?}").into())
    }

    fn update_state(&mut self, event: NativeEvent) {
        let value = event.get_value();
        let capability = event.as_capability();

        match capability {
            Capability::Gamepad(gamepad) => match gamepad {
                Gamepad::Button(btn) => match btn {
                    GamepadButton::South => self.state.btn_a = event.pressed(),
                    GamepadButton::East => self.state.btn_b = event.pressed(),
                    GamepadButton::North => self.state.btn_y = event.pressed(),
                    GamepadButton::West => self.state.btn_x = event.pressed(),
                    GamepadButton::Start => self.state.btn_start = event.pressed(),
                    GamepadButton::Select => self.state.btn_select = event.pressed(),
                    GamepadButton::Guide => self.state.btn_guide = event.pressed(),
                    GamepadButton::LeftBumper => self.state.btn_lb = event.pressed(),
                    GamepadButton::RightBumper => self.state.btn_rb = event.pressed(),
                    GamepadButton::LeftStick => self.state.btn_l3 = event.pressed(),
                    GamepadButton::RightStick => self.state.btn_r3 = event.pressed(),
                    GamepadButton::LeftPaddle1 => self.state.btn_l4 = event.pressed(),
                    GamepadButton::RightPaddle1 => self.state.btn_r4 = event.pressed(),
                    GamepadButton::LeftPaddle2 => self.state.btn_pl = event.pressed(),
                    GamepadButton::RightPaddle2 => self.state.btn_pr = event.pressed(),
                    GamepadButton::DPadUp => {
                        self.state.set_dpad(DpadDirection::Up, event.pressed())
                    }
                    GamepadButton::DPadDown => {
                        self.state.set_dpad(DpadDirection::Down, event.pressed())
                    }
                    GamepadButton::DPadLeft => {
                        self.state.set_dpad(DpadDirection::Left, event.pressed())
                    }
                    GamepadButton::DPadRight => {
                        self.state.set_dpad(DpadDirection::Right, event.pressed())
                    }
                    // Digital trigger fallback
                    GamepadButton::LeftTrigger => {
                        self.state.lt_analog = if event.pressed() { 0xff } else { 0x00 }
                    }
                    GamepadButton::RightTrigger => {
                        self.state.rt_analog = if event.pressed() { 0xff } else { 0x00 }
                    }
                    _ => (),
                },

                Gamepad::Axis(axis) => match axis {
                    GamepadAxis::LeftStick => {
                        if let InputValue::Vector2 { x, y } = value {
                            if let Some(x) = x {
                                self.state.joystick_l_x =
                                    denormalize_signed_value_u8(x, JOY_AXIS_MIN, JOY_AXIS_MAX);
                            }
                            if let Some(y) = y {
                                self.state.joystick_l_y =
                                    denormalize_signed_value_u8(y, JOY_AXIS_MIN, JOY_AXIS_MAX);
                            }
                        }
                    }
                    GamepadAxis::RightStick => {
                        if let InputValue::Vector2 { x, y } = value {
                            if let Some(x) = x {
                                self.state.joystick_r_x =
                                    denormalize_signed_value_u8(x, JOY_AXIS_MIN, JOY_AXIS_MAX);
                            }
                            if let Some(y) = y {
                                self.state.joystick_r_y =
                                    denormalize_signed_value_u8(y, JOY_AXIS_MIN, JOY_AXIS_MAX);
                            }
                        }
                    }
                    GamepadAxis::Hat0 => {
                        if let InputValue::Vector2 { x, y } = value {
                            if let Some(x) = x {
                                match x.partial_cmp(&0.0) {
                                    Some(Ordering::Less) => {
                                        self.state.set_dpad(DpadDirection::Left, true)
                                    }
                                    Some(Ordering::Equal) => {
                                        self.state.set_dpad(DpadDirection::Left, false);
                                        self.state.set_dpad(DpadDirection::Right, false);
                                    }
                                    Some(Ordering::Greater) => {
                                        self.state.set_dpad(DpadDirection::Right, true)
                                    }
                                    None => (),
                                }
                            }
                            if let Some(y) = y {
                                match y.partial_cmp(&0.0) {
                                    Some(Ordering::Less) => {
                                        self.state.set_dpad(DpadDirection::Up, true)
                                    }
                                    Some(Ordering::Equal) => {
                                        self.state.set_dpad(DpadDirection::Up, false);
                                        self.state.set_dpad(DpadDirection::Down, false);
                                    }
                                    Some(Ordering::Greater) => {
                                        self.state.set_dpad(DpadDirection::Down, true)
                                    }
                                    None => (),
                                }
                            }
                        }
                    }
                    _ => (),
                },

                Gamepad::Trigger(trigger) => match trigger {
                    GamepadTrigger::LeftTrigger => {
                        if let InputValue::Float(v) = value {
                            self.state.lt_analog =
                                denormalize_unsigned_value_u8(v, TRIGGER_AXIS_MAX);
                        }
                    }
                    GamepadTrigger::RightTrigger => {
                        if let InputValue::Float(v) = value {
                            self.state.rt_analog =
                                denormalize_unsigned_value_u8(v, TRIGGER_AXIS_MAX);
                        }
                    }
                    _ => (),
                },

                // Axis layout (x=pitch, y=yaw, z=roll): yaw/roll axes are swapped
                // relative to SDL sGyro/sAccel naming; pitch is negated.
                Gamepad::Accelerometer => {
                    if let InputValue::Vector3 { x, y, z } = value {
                        if let Some(x) = x {
                            self.state.accel_y =
                                Integer::from_primitive(denormalize_accel(x).wrapping_neg());
                        }
                        if let Some(y) = y {
                            self.state.accel_z = Integer::from_primitive(denormalize_accel(y));
                        }
                        if let Some(z) = z {
                            self.state.accel_x =
                                Integer::from_primitive(denormalize_accel(z).wrapping_neg());
                        }
                    }
                }

                Gamepad::Gyro => {
                    if let InputValue::Vector3 { x, y, z } = value {
                        if let Some(x) = x {
                            self.state.gyro_y = Integer::from_primitive((x as i16).wrapping_neg());
                        }
                        if let Some(y) = y {
                            self.state.gyro_z = Integer::from_primitive(y as i16);
                        }
                        if let Some(z) = z {
                            self.state.gyro_x = Integer::from_primitive((z as i16).wrapping_neg());
                        }
                    }
                }

                _ => (),
            },

            Capability::Gyroscope(_) => {
                if let InputValue::Vector3 { x, y, z } = value {
                    if let Some(x) = x {
                        self.state.gyro_x = Integer::from_primitive(x as i16);
                    }
                    if let Some(y) = y {
                        self.state.gyro_y = Integer::from_primitive(y as i16);
                    }
                    if let Some(z) = z {
                        self.state.gyro_z = Integer::from_primitive(z as i16);
                    }
                }
            }
            Capability::Accelerometer(_) => {
                if let InputValue::Vector3 { x, y, z } = value {
                    if let Some(x) = x {
                        self.state.accel_x = Integer::from_primitive(x as i16);
                    }
                    if let Some(y) = y {
                        self.state.accel_y = Integer::from_primitive(y as i16);
                    }
                    if let Some(z) = z {
                        self.state.accel_z = Integer::from_primitive(z as i16);
                    }
                }
            }

            _ => (),
        }
    }

    // Rumble output report format: [0x05, strong_hi, weak_hi, 0x00, 0x00]
    fn handle_output(&mut self, data: Vec<u8>) -> Result<Vec<OutputEvent>, Box<dyn Error>> {
        let Some(&report_id) = data.first() else {
            return Ok(vec![]);
        };

        if report_id == REPORT_ID_RUMBLE {
            let buf: [u8; 5] = data.as_slice().try_into()?;
            let report = PackedRumbleOutputReport::unpack(&buf)?;
            let strong_magnitude = (report.strong_magnitude as u16) << 8;
            let weak_magnitude = (report.weak_magnitude as u16) << 8;
            log::trace!("Rumble: strong={strong_magnitude} weak={weak_magnitude}");
            return Ok(vec![OutputEvent::Rumble {
                weak_magnitude,
                strong_magnitude,
            }]);
        }

        Ok(vec![])
    }
}

impl TargetInputDevice for Ultimate2WirelessDevice {
    fn write_event(&mut self, event: NativeEvent) -> Result<(), InputError> {
        log::trace!("Received event: {event:?}");

        // QuickAccess maps to Guide+South combo (press: Guide@0ms South@160ms,
        // release: South@160ms Guide@240ms), same timing as xpad target.
        if event.as_capability()
            == Capability::Gamepad(Gamepad::Button(GamepadButton::QuickAccess))
        {
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
                (
                    ScheduledNativeEvent::new(guide, Duration::from_millis(0)),
                    ScheduledNativeEvent::new(south, Duration::from_millis(160)),
                )
            } else {
                (
                    ScheduledNativeEvent::new(guide, Duration::from_millis(240)),
                    ScheduledNativeEvent::new(south, Duration::from_millis(160)),
                )
            };
            self.queued_events.push(guide);
            self.queued_events.push(south);
            return Ok(());
        }

        // Screenshot maps to Guide+RightTrigger combo (press: Guide@0ms RightTrigger@160ms,
        // release: RightTrigger@160ms Guide@240ms), same timing as horipad target.
        if event.as_capability()
            == Capability::Gamepad(Gamepad::Button(GamepadButton::Screenshot))
        {
            let pressed = event.pressed();
            let guide = NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::Guide)),
                event.get_value(),
            );
            let trigv = if pressed { 0.5 } else { 0.0 };
            let trigr = NativeEvent::new(
                Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::RightTrigger)),
                InputValue::Float(trigv),
            );
            let (guide, trigr) = if pressed {
                (
                    ScheduledNativeEvent::new(guide, Duration::from_millis(0)),
                    ScheduledNativeEvent::new(trigr, Duration::from_millis(160)),
                )
            } else {
                (
                    ScheduledNativeEvent::new(guide, Duration::from_millis(240)),
                    ScheduledNativeEvent::new(trigr, Duration::from_millis(160)),
                )
            };
            self.queued_events.push(guide);
            self.queued_events.push(trigr);
            return Ok(());
        }

        self.update_state(event);
        Ok(())
    }

    fn get_capabilities(&self) -> Result<Vec<Capability>, InputError> {
        Ok(vec![
            Capability::Gamepad(Gamepad::Accelerometer),
            Capability::Gamepad(Gamepad::Gyro),
            Capability::Gamepad(Gamepad::Button(GamepadButton::QuickAccess)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::Screenshot)),
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
            Capability::Gamepad(Gamepad::Button(GamepadButton::RightBumper)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::RightPaddle1)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::RightPaddle2)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::RightStick)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::RightTrigger)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::Select)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::South)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::Start)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::West)),
            Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::LeftTrigger)),
            Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::RightTrigger)),
        ])
    }

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

    fn clear_state(&mut self) {
        self.state = PackedInputDataReport::default();
    }
}

impl TargetOutputDevice for Ultimate2WirelessDevice {
    fn poll(
        &mut self,
        _composite_device: &Option<CompositeDeviceClient>,
    ) -> Result<Vec<OutputEvent>, OutputError> {
        let event = match self.device.read() {
            Ok(event) => event,
            Err(err) => match err {
                StreamError::Io(_) => {
                    self.write_state()?;
                    return Ok(vec![]);
                }
                StreamError::UnknownEventType(e) => {
                    log::debug!("Unknown UHID event type: {:?}", e);
                    self.write_state()?;
                    return Ok(vec![]);
                }
            },
        };

        let output_events = match event {
            uhid_virt::OutputEvent::Start { dev_flags: _ } => {
                log::debug!("Start event received");
                Ok(vec![])
            }
            uhid_virt::OutputEvent::Stop => {
                log::debug!("Stop event received");
                Ok(vec![])
            }
            uhid_virt::OutputEvent::Open => {
                log::debug!("Open event received");
                Ok(vec![])
            }
            uhid_virt::OutputEvent::Close => {
                log::debug!("Close event received");
                Ok(vec![])
            }
            uhid_virt::OutputEvent::Output { data } => {
                log::trace!("Got output data: {:?}", data);
                let events = self.handle_output(data).map_err(|e| {
                    OutputError::DeviceError(format!("Failed to process output event: {:?}", e))
                })?;
                Ok(events)
            }
            uhid_virt::OutputEvent::GetReport { .. } => Ok(vec![]),
            uhid_virt::OutputEvent::SetReport { id, .. } => {
                if let Err(e) = self.device.write_set_report_reply(id, 0) {
                    log::warn!("Failed to write set report reply: {:?}", e);
                    return Err(e.to_string().into());
                }
                Ok(vec![])
            }
        };

        self.write_state()?;

        output_events
    }

    fn get_output_capabilities(&self) -> Result<Vec<OutputCapability>, OutputError> {
        Ok(vec![OutputCapability::ForceFeedback])
    }
}

impl Debug for Ultimate2WirelessDevice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Ultimate2WirelessDevice")
            .field("state", &self.state)
            .finish()
    }
}

// m/s² → raw i16 (4096 units = 1G)
fn denormalize_accel(value_m_s2: f64) -> i16 {
    let g = value_m_s2 / GRAVITY;
    (g * ACCEL_SCALE).clamp(i16::MIN as f64, i16::MAX as f64) as i16
}

