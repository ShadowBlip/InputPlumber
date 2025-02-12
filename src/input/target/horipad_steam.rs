//! Emulates a Horipad Steam Controller as a target input device.
use std::{cmp::Ordering, error::Error, fmt::Debug, fs::File, time::Duration};

use packed_struct::prelude::*;
use uhid_virt::{Bus, CreateParams, StreamError, UHIDDevice};

use crate::{
    drivers::horipad_steam::{
        driver::{JOY_AXIS_MAX, JOY_AXIS_MIN, PIDS, TRIGGER_AXIS_MAX, VID},
        hid_report::{Direction, PackedInputDataReport},
        report_descriptor::REPORT_DESCRIPTOR,
    },
    input::{
        capability::{Capability, Gamepad, GamepadAxis, GamepadButton, GamepadTrigger},
        composite_device::client::CompositeDeviceClient,
        event::{
            native::{NativeEvent, ScheduledNativeEvent},
            value::InputValue,
        },
        output_capability::OutputCapability,
        output_event::OutputEvent,
    },
};

use super::{InputError, OutputError, TargetInputDevice, TargetOutputDevice};

/// The [HoripadSteamDevice] is a target input device implementation that emulates
/// a Horipad Steam Controller using uhid.
pub struct HoripadSteamDevice {
    device: UHIDDevice<File>,
    state: PackedInputDataReport,
    timestamp: u8,
    queued_events: Vec<ScheduledNativeEvent>,
}

impl HoripadSteamDevice {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let device = HoripadSteamDevice::create_virtual_device()?;
        Ok(Self {
            device,
            state: PackedInputDataReport::default(),
            timestamp: 0,
            queued_events: Vec::new(),
        })
    }

    /// Create the virtual device to emulate
    fn create_virtual_device() -> Result<UHIDDevice<File>, Box<dyn Error>> {
        let device = UHIDDevice::create(CreateParams {
            name: String::from("HORI CO.,LTD. HORIPAD STEAM"),
            phys: String::from(""),
            uniq: String::from(""),
            bus: Bus::USB,
            vendor: VID as u32,
            product: PIDS[1] as u32,
            version: 0x111,
            country: 0,
            rd_data: REPORT_DESCRIPTOR.to_vec(),
        })?;

        Ok(device)
    }

    /// Write the current device state to the device
    fn write_state(&mut self) -> Result<(), Box<dyn Error>> {
        let data = self.state.pack()?;

        // Write the state to the virtual HID
        if let Err(e) = self.device.write(&data) {
            let err = format!("Failed to write input data report: {:?}", e);
            return Err(err.into());
        }

        Ok(())
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
                    GamepadButton::South => self.state.a = event.pressed(),
                    GamepadButton::East => self.state.b = event.pressed(),
                    GamepadButton::North => self.state.x = event.pressed(),
                    GamepadButton::West => self.state.y = event.pressed(),
                    GamepadButton::Start => self.state.menu = event.pressed(),
                    GamepadButton::Select => self.state.view = event.pressed(),
                    GamepadButton::Guide => self.state.steam = event.pressed(),
                    GamepadButton::QuickAccess => self.state.quick = event.pressed(),
                    GamepadButton::DPadUp => {
                        self.state.dpad = self.state.dpad.change(Direction::Up, event.pressed())
                    }
                    GamepadButton::DPadDown => {
                        self.state.dpad = self.state.dpad.change(Direction::Down, event.pressed())
                    }
                    GamepadButton::DPadLeft => {
                        self.state.dpad = self.state.dpad.change(Direction::Left, event.pressed())
                    }
                    GamepadButton::DPadRight => {
                        self.state.dpad = self.state.dpad.change(Direction::Right, event.pressed())
                    }
                    GamepadButton::LeftBumper => self.state.lb = event.pressed(),
                    GamepadButton::LeftTrigger => self.state.lt_digital = event.pressed(),
                    GamepadButton::LeftPaddle1 => self.state.l4 = event.pressed(),
                    GamepadButton::LeftPaddle2 => self.state.m1 = event.pressed(),
                    GamepadButton::LeftStick => self.state.ls_click = event.pressed(),
                    GamepadButton::LeftStickTouch => self.state.ls_touch = event.pressed(),
                    GamepadButton::RightBumper => self.state.rb = event.pressed(),
                    GamepadButton::RightTrigger => self.state.rt_digital = event.pressed(),
                    GamepadButton::RightPaddle1 => self.state.r4 = event.pressed(),
                    GamepadButton::RightPaddle2 => self.state.m2 = event.pressed(),
                    GamepadButton::RightStick => self.state.rs_click = event.pressed(),
                    GamepadButton::RightStickTouch => self.state.rs_touch = event.pressed(),
                    GamepadButton::LeftPaddle3 => (),
                    GamepadButton::RightPaddle3 => (),
                    GamepadButton::Screenshot => (),
                    _ => (),
                },
                Gamepad::Axis(axis) => match axis {
                    GamepadAxis::LeftStick => {
                        if let InputValue::Vector2 { x, y } = value {
                            if let Some(x) = x {
                                let value = denormalize_signed_value(x, JOY_AXIS_MIN, JOY_AXIS_MAX);
                                self.state.joystick_l_x = value
                            }
                            if let Some(y) = y {
                                let value = denormalize_signed_value(y, JOY_AXIS_MIN, JOY_AXIS_MAX);
                                self.state.joystick_l_y = value
                            }
                        }
                    }
                    GamepadAxis::RightStick => {
                        if let InputValue::Vector2 { x, y } = value {
                            if let Some(x) = x {
                                let value = denormalize_signed_value(x, JOY_AXIS_MIN, JOY_AXIS_MAX);
                                self.state.joystick_r_x = value
                            }
                            if let Some(y) = y {
                                let value = denormalize_signed_value(y, JOY_AXIS_MIN, JOY_AXIS_MAX);
                                self.state.joystick_r_y = value
                            }
                        }
                    }
                    GamepadAxis::Hat0 => {
                        if let InputValue::Vector2 { x, y } = value {
                            if let Some(x) = x {
                                let value = denormalize_signed_value(x, -1.0, 1.0);
                                match value.cmp(&0) {
                                    Ordering::Less => {
                                        self.state.dpad =
                                            self.state.dpad.change(Direction::Left, true)
                                    }
                                    Ordering::Equal => {
                                        self.state.dpad =
                                            self.state.dpad.change(Direction::Left, false);
                                        self.state.dpad =
                                            self.state.dpad.change(Direction::Right, false);
                                    }
                                    Ordering::Greater => {
                                        self.state.dpad =
                                            self.state.dpad.change(Direction::Right, true)
                                    }
                                }
                            }
                            if let Some(y) = y {
                                let value = denormalize_signed_value(y, -1.0, 1.0);
                                match value.cmp(&0) {
                                    Ordering::Less => {
                                        self.state.dpad =
                                            self.state.dpad.change(Direction::Up, true)
                                    }
                                    Ordering::Equal => {
                                        self.state.dpad =
                                            self.state.dpad.change(Direction::Up, false);
                                        self.state.dpad =
                                            self.state.dpad.change(Direction::Down, false);
                                    }
                                    Ordering::Greater => {
                                        self.state.dpad =
                                            self.state.dpad.change(Direction::Down, true)
                                    }
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
                            let value = denormalize_unsigned_value(normal_value, TRIGGER_AXIS_MAX);
                            self.state.lt_analog = value
                        }
                    }
                    GamepadTrigger::LeftTouchpadForce => (),
                    GamepadTrigger::LeftStickForce => (),
                    GamepadTrigger::RightTrigger => {
                        if let InputValue::Float(normal_value) = value {
                            let value = denormalize_unsigned_value(normal_value, TRIGGER_AXIS_MAX);
                            self.state.rt_analog = value
                        }
                    }
                    GamepadTrigger::RightTouchpadForce => (),
                    GamepadTrigger::RightStickForce => (),
                },
                Gamepad::Accelerometer => {
                    if let InputValue::Vector3 { x, y, z } = value {
                        if let Some(x) = x {
                            self.state.accel_x = Integer::from_primitive(denormalize_accel_value(x))
                        }
                        if let Some(y) = y {
                            self.state.accel_y = Integer::from_primitive(denormalize_accel_value(y))
                        }
                        if let Some(z) = z {
                            self.state.accel_z = Integer::from_primitive(denormalize_accel_value(z))
                        }
                    }
                }
                Gamepad::Gyro => {
                    if let InputValue::Vector3 { x, y, z } = value {
                        if let Some(x) = x {
                            self.state.gyro_x = Integer::from_primitive(denormalize_gyro_value(x));
                        }
                        if let Some(y) = y {
                            self.state.gyro_y = Integer::from_primitive(denormalize_gyro_value(y))
                        }
                        if let Some(z) = z {
                            self.state.gyro_z = Integer::from_primitive(denormalize_gyro_value(z))
                        }
                    }
                }
            },
            Capability::DBus(_) => (),
            Capability::Mouse(_) => (),
            Capability::Keyboard(_) => (),
            Capability::Touchpad(_) => (),
            Capability::Touchscreen(_) => (),
        };
    }

    /// Handle [OutputEvent::Output] events from the HIDRAW device. These are
    /// events which should be forwarded back to source devices.
    fn handle_output(&mut self, _data: Vec<u8>) -> Result<Vec<OutputEvent>, Box<dyn Error>> {
        // Validate the output report size
        Ok(vec![])
    }

    /// Handle [OutputEvent::GetReport] events from the HIDRAW device
    fn handle_get_report(
        &mut self,
        _id: u32,
        _report_number: u8,
        _report_type: uhid_virt::ReportType,
    ) -> Result<(), Box<dyn Error>> {
        Ok(())
    }
}

impl TargetInputDevice for HoripadSteamDevice {
    fn write_event(&mut self, event: NativeEvent) -> Result<(), InputError> {
        log::trace!("Received event: {event:?}");
        self.update_state(event);
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
            Capability::Gamepad(Gamepad::Button(GamepadButton::LeftStickTouch)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::LeftTrigger)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::North)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::QuickAccess)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::RightBumper)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::RightPaddle1)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::RightPaddle2)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::RightStick)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::RightStickTouch)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::RightTrigger)),
            //           Capability::Gamepad(Gamepad::Button(GamepadButton::Screenshot)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::Select)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::South)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::Start)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::West)),
            Capability::Gamepad(Gamepad::Gyro),
            Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::LeftTrigger)),
            Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::RightTrigger)),
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
        let caps = self.get_capabilities().unwrap_or_else(|_| {
            log::error!("No target device capabilities found while clearing state.");
            Vec::new()
        });
        for cap in caps {
            let ev = NativeEvent::new(cap, InputValue::Bool(false));
            self.queued_events
                .push(ScheduledNativeEvent::new(ev, Duration::from_millis(0)));
        }
    }
}

impl TargetOutputDevice for HoripadSteamDevice {
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
                Ok(vec![])
            }
        };

        // Write the current state
        self.write_state()?;

        output_events
    }

    fn get_output_capabilities(&self) -> Result<Vec<OutputCapability>, OutputError> {
        Ok(vec![OutputCapability::ForceFeedback])
    }
}

impl Debug for HoripadSteamDevice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HoripadSteamDevice")
            .field("state", &self.state)
            .field("timestamp", &self.timestamp)
            .finish()
    }
}

/// Convert the given normalized value between -1.0 - 1.0 to the real value
/// based on the given minimum and maximum axis range. Horipad gamepads
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
/// the controller understands.
/// Accelerometer values are measured in []
/// units of G acceleration (1G == 9.8m/s). InputPlumber accelerometer
/// values are measured in units of meters per second. To denormalize
/// the value, it needs to be converted into G units (by dividing by 9.8),
/// then multiplying that value by the [].
fn denormalize_accel_value(value_meters_sec: f64) -> i16 {
    let value = value_meters_sec;
    value as i16
}

/// Horipad gyro values are measured in units of degrees per second.
/// InputPlumber gyro values are also measured in degrees per second.
fn denormalize_gyro_value(value_degrees_sec: f64) -> i16 {
    let value = value_degrees_sec;
    value as i16
}
