use packed_struct::prelude::*;
use std::{error::Error, ffi::CString, time::Instant};

use hidapi::HidDevice;

use crate::drivers::dualsense::{
    event::{BinaryInput, ButtonEvent, TriggerEvent, TriggerInput},
    hid_report::Direction,
};

use super::{
    event::{AxisEvent, AxisInput, Event, InertialInput, IntertialEvent, TouchAxisInput},
    hid_report::{
        CalibrationReport, PackedInputDataReport, SetStatePackedOutputData, UsbPackedOutputReport,
    },
    DS5_ACCEL_RAW_TO_MPS2, DS5_GYRO_RAW_TO_RAD_S, DS5_VID, DS_FEATURE_REPORT_CALIBRATION_SIZE,
    FEATURE_REPORT_CALIBRATION, INPUT_REPORT_BT_SIZE, PIDS,
};

/// Linear per-axis calibration: `physical = (raw - bias) * scale`.
#[derive(Debug, Clone, Copy)]
pub struct AxisCalibration {
    pub bias: f64,
    pub scale: f64,
}

impl AxisCalibration {
    pub fn apply(&self, raw: i16) -> f64 {
        (raw as f64 - self.bias) * self.scale
    }
}

/// Computes a gyroscope axis's bias and raw-to-rad/s scale from its
/// calibration report fields, using the same formula as hid-playstation.c.
pub fn gyro_axis_calibration(
    plus: i16,
    minus: i16,
    bias: i16,
    speed_2x: i32,
) -> AxisCalibration {
    let plus = plus as i32;
    let minus = minus as i32;
    let bias = bias as i32;
    let denom = (plus - bias).abs() + (minus - bias).abs();
    let (numer, denom) = if denom == 0 {
        (2048 * 1024, i16::MAX as i32)
    } else {
        (speed_2x * 1024, denom)
    };
    AxisCalibration {
        bias: 0.0, // kernel always reports gyro bias-corrected to 0
        scale: numer as f64 / denom as f64 / 1024.0 * std::f64::consts::PI / 180.0,
    }
}

/// Computes an accelerometer axis's bias and raw-to-m/s² scale from its
/// calibration report fields, using the same formula as hid-playstation.c.
pub fn accel_axis_calibration(plus: i16, minus: i16) -> AxisCalibration {
    let plus = plus as i32;
    let minus = minus as i32;
    let range = plus - minus;
    let (numer, denom, bias) = if range == 0 {
        (4 * 8192, i16::MAX as i32, 0.0)
    } else {
        (2 * 8192, range, plus as f64 - range as f64 / 2.0)
    };
    AxisCalibration {
        bias,
        scale: numer as f64 / denom as f64 / 8192.0 * 9.80665,
    }
}

/// Reads the calibration report and derives per-axis gyro (pitch/yaw/roll)
/// and accel (x/y/z) calibration from it. Falls back to a nominal scale if
/// the report can't be read or parsed.
fn read_calibration(device: &HidDevice) -> ([AxisCalibration; 3], [AxisCalibration; 3]) {
    let fallback_gyro = [AxisCalibration {
        bias: 0.0,
        scale: DS5_GYRO_RAW_TO_RAD_S,
    }; 3];
    let fallback_accel = [AxisCalibration {
        bias: 0.0,
        scale: DS5_ACCEL_RAW_TO_MPS2,
    }; 3];

    let mut buf = [0u8; DS_FEATURE_REPORT_CALIBRATION_SIZE];
    buf[0] = FEATURE_REPORT_CALIBRATION;
    if let Err(e) = device.get_feature_report(&mut buf) {
        log::warn!("Failed to read DualSense calibration report, using nominal scale: {e}");
        return (fallback_gyro, fallback_accel);
    }
    let report = match CalibrationReport::unpack(&buf) {
        Ok(report) => report,
        Err(e) => {
            log::warn!("Failed to parse DualSense calibration report, using nominal scale: {e}");
            return (fallback_gyro, fallback_accel);
        }
    };

    let speed_2x = report.gyro_speed_plus.to_primitive() as i32
        + report.gyro_speed_minus.to_primitive() as i32;
    let gyro = [
        gyro_axis_calibration(
            report.gyro_pitch_plus.to_primitive(),
            report.gyro_pitch_minus.to_primitive(),
            report.gyro_pitch_bias.to_primitive(),
            speed_2x,
        ),
        gyro_axis_calibration(
            report.gyro_yaw_plus.to_primitive(),
            report.gyro_yaw_minus.to_primitive(),
            report.gyro_yaw_bias.to_primitive(),
            speed_2x,
        ),
        gyro_axis_calibration(
            report.gyro_roll_plus.to_primitive(),
            report.gyro_roll_minus.to_primitive(),
            report.gyro_roll_bias.to_primitive(),
            speed_2x,
        ),
    ];
    let accel = [
        accel_axis_calibration(
            report.acc_x_plus.to_primitive(),
            report.acc_x_minus.to_primitive(),
        ),
        accel_axis_calibration(
            report.acc_y_plus.to_primitive(),
            report.acc_y_minus.to_primitive(),
        ),
        accel_axis_calibration(
            report.acc_z_plus.to_primitive(),
            report.acc_z_minus.to_primitive(),
        ),
    ];
    (gyro, accel)
}

/// PS5 Dualsense controller driver for reading gamepad input
pub struct Driver {
    state: Option<PackedInputDataReport>,
    touch_state: [bool; 2],
    /// Timestamp of the last touch event.
    last_touch: Instant,
    device: HidDevice,
    leds_initialized: bool,
    /// Pitch, yaw, roll.
    gyro_calibration: [AxisCalibration; 3],
    /// X, y, z.
    accel_calibration: [AxisCalibration; 3],
}

impl Driver {
    pub fn new(path: String) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let c_path = CString::new(path.clone())?;
        let api = hidapi::HidApi::new()?;
        let device = api.open_path(&c_path)?;
        let info = device.get_device_info()?;
        let vid = info.vendor_id();
        let pid = info.product_id();
        if vid != DS5_VID || !PIDS.contains(&pid) {
            return Err(
                format!("Device '{path}' is not a DualSense Controller: {vid}:{pid}").into(),
            );
        }

        let (gyro_calibration, accel_calibration) = read_calibration(&device);

        Ok(Self {
            device,
            state: None,
            touch_state: [false, false],
            last_touch: Instant::now(),
            leds_initialized: false,
            gyro_calibration,
            accel_calibration,
        })
    }

    /// Returns the per-axis (pitch, yaw, roll) gyroscope calibration derived
    /// from the device's calibration report.
    pub fn gyro_calibration(&self) -> [AxisCalibration; 3] {
        self.gyro_calibration
    }

    /// Returns the per-axis (x, y, z) accelerometer calibration derived from
    /// the device's calibration report.
    pub fn accel_calibration(&self) -> [AxisCalibration; 3] {
        self.accel_calibration
    }

    /// Poll the device and read input reports
    pub fn poll(&mut self) -> Result<Vec<Event>, Box<dyn Error + Send + Sync>> {
        // Read data from the device into a buffer
        let mut buf = [0; INPUT_REPORT_BT_SIZE];
        let bytes_read = self.device.read(&mut buf[..])?;
        let slice = &buf[..bytes_read];

        // Handle the incoming input report
        let events = self.handle_input_report(slice, bytes_read)?;

        Ok(events)
    }

    /// Writes the given output state to the gamepad. This can be used to change
    /// the color of LEDs, activate rumble, etc.
    pub fn write(
        &self,
        state: SetStatePackedOutputData,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let report = UsbPackedOutputReport {
            state,
            ..Default::default()
        };
        let buf = report.pack()?;
        let _bytes_written = self.device.write(&buf)?;

        Ok(())
    }

    /// Release the LEDs from Wireless firmware control
    /// When in wireless mode this must be signaled to control LEDs
    /// This cannot be applied during the BT pair animation.
    /// SDL2 waits until the SensorTimestamp value is >= 10200000
    /// before pulsing this bit once.
    pub fn reset_lights(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        let state = SetStatePackedOutputData {
            reset_lights: true,
            ..Default::default()
        };
        self.write(state)
    }

    /// Set the color of the gamepad to the given value
    pub fn set_led_color(&self, r: u8, g: u8, b: u8) -> Result<(), Box<dyn Error + Send + Sync>> {
        log::debug!("Setting LED color to: {r}, {g}, {b}");
        let state = SetStatePackedOutputData {
            allow_led_color: true,
            led_red: r,
            led_green: g,
            led_blue: b,
            ..Default::default()
        };

        self.write(state)
    }

    /// Use rumble emulation to rumble the gamepad
    pub fn rumble(
        &self,
        left_speed: u8,
        right_speed: u8,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let state = SetStatePackedOutputData {
            use_rumble_not_haptics: true,
            enable_rumble_emulation: true,
            rumble_emulation_right: right_speed,
            rumble_emulation_left: left_speed,
            ..Default::default()
        };

        self.write(state)
    }

    /// Unpacks the buffer into a [PackedInputDataReport] structure and updates
    /// the internal gamepad state
    fn handle_input_report(
        &mut self,
        buf: &[u8],
        bytes_read: usize,
    ) -> Result<Vec<Event>, Box<dyn Error + Send + Sync>> {
        let input_report = PackedInputDataReport::unpack(buf, bytes_read)?;

        // Check if LEDs need to be initialized
        if !self.leds_initialized
            && input_report.state().sensor_timestamp.to_primitive() >= 10200000
        {
            log::debug!("Initializing LEDs");
            self.reset_lights()?;
            self.leds_initialized = true;
            // TODO: Remove this after adding LED support
            self.set_led_color(100, 72, 42)?;
        }

        // Print input report for debugging
        //log::debug!("--- Input report ---");
        //log::debug!("{input_report}");
        //log::debug!("---- End Report ----");

        // Update the state
        let old_state = self.update_state(input_report);

        // Translate the state into a stream of input events
        let events = self.translate(old_state);

        Ok(events)
    }

    /// Update the internal state and return the old state
    fn update_state(
        &mut self,
        input_report: PackedInputDataReport,
    ) -> Option<PackedInputDataReport> {
        let old_state = self.state;
        self.state = Some(input_report);
        old_state
    }

    /// Translate the current state into events
    fn translate(&mut self, old_state: Option<PackedInputDataReport>) -> Vec<Event> {
        let mut events = Vec::new();
        let Some(report) = self.state else {
            return events;
        };
        let state = report.state();

        let Some(old_report) = old_state else {
            return events;
        };
        let old_state = old_report.state();

        // Button events
        if state.square != old_state.square {
            events.push(Event::Button(ButtonEvent::Square(BinaryInput {
                pressed: state.square,
            })));
        }
        if state.circle != old_state.circle {
            events.push(Event::Button(ButtonEvent::Circle(BinaryInput {
                pressed: state.circle,
            })));
        }
        if state.triangle != old_state.triangle {
            events.push(Event::Button(ButtonEvent::Triangle(BinaryInput {
                pressed: state.triangle,
            })));
        }
        if state.cross != old_state.cross {
            events.push(Event::Button(ButtonEvent::Cross(BinaryInput {
                pressed: state.cross,
            })));
        }
        if state.ps != old_state.ps {
            events.push(Event::Button(ButtonEvent::Guide(BinaryInput {
                pressed: state.ps,
            })));
        }
        if state.dpad != old_state.dpad {
            let new_dpad = state.dpad.as_bitflag();
            let old_dpad = old_state.dpad.as_bitflag();
            let disabled_fields = old_dpad & !new_dpad;

            if new_dpad & Direction::North.as_bitflag() != 0 {
                events.push(Event::Button(ButtonEvent::DPadUp(BinaryInput {
                    pressed: true,
                })));
            }
            if new_dpad & Direction::East.as_bitflag() != 0 {
                events.push(Event::Button(ButtonEvent::DPadRight(BinaryInput {
                    pressed: true,
                })))
            }
            if new_dpad & Direction::West.as_bitflag() != 0 {
                events.push(Event::Button(ButtonEvent::DPadLeft(BinaryInput {
                    pressed: true,
                })))
            }
            if new_dpad & Direction::South.as_bitflag() != 0 {
                events.push(Event::Button(ButtonEvent::DPadDown(BinaryInput {
                    pressed: true,
                })))
            }

            if disabled_fields & Direction::North.as_bitflag() != 0 {
                events.push(Event::Button(ButtonEvent::DPadUp(BinaryInput {
                    pressed: false,
                })));
            }
            if disabled_fields & Direction::East.as_bitflag() != 0 {
                events.push(Event::Button(ButtonEvent::DPadRight(BinaryInput {
                    pressed: false,
                })))
            }
            if disabled_fields & Direction::West.as_bitflag() != 0 {
                events.push(Event::Button(ButtonEvent::DPadLeft(BinaryInput {
                    pressed: false,
                })))
            }
            if disabled_fields & Direction::South.as_bitflag() != 0 {
                events.push(Event::Button(ButtonEvent::DPadDown(BinaryInput {
                    pressed: false,
                })))
            }
        }
        if state.l1 != old_state.l1 {
            events.push(Event::Button(ButtonEvent::L1(BinaryInput {
                pressed: state.l1,
            })));
        }
        if state.r1 != old_state.r1 {
            events.push(Event::Button(ButtonEvent::R1(BinaryInput {
                pressed: state.r1,
            })));
        }
        if state.l2 != old_state.l2 {
            events.push(Event::Button(ButtonEvent::L2(BinaryInput {
                pressed: state.l2,
            })));
        }
        if state.r2 != old_state.r2 {
            events.push(Event::Button(ButtonEvent::R2(BinaryInput {
                pressed: state.r2,
            })));
        }
        if state.l3 != old_state.l3 {
            events.push(Event::Button(ButtonEvent::L3(BinaryInput {
                pressed: state.l3,
            })));
        }
        if state.r3 != old_state.r3 {
            events.push(Event::Button(ButtonEvent::R3(BinaryInput {
                pressed: state.r3,
            })));
        }
        if state.options != old_state.options {
            events.push(Event::Button(ButtonEvent::Options(BinaryInput {
                pressed: state.options,
            })));
        }
        if state.create != old_state.create {
            events.push(Event::Button(ButtonEvent::Create(BinaryInput {
                pressed: state.create,
            })));
        }
        if state.right_paddle != old_state.right_paddle {
            events.push(Event::Button(ButtonEvent::R4(BinaryInput {
                pressed: state.right_paddle,
            })));
        }
        if state.left_paddle != old_state.left_paddle {
            events.push(Event::Button(ButtonEvent::L4(BinaryInput {
                pressed: state.left_paddle,
            })));
        }
        if state.right_fn != old_state.right_fn {
            events.push(Event::Button(ButtonEvent::R5(BinaryInput {
                pressed: state.right_fn,
            })));
        }
        if state.left_fn != old_state.left_fn {
            events.push(Event::Button(ButtonEvent::L5(BinaryInput {
                pressed: state.left_fn,
            })));
        }
        if state.mute != old_state.mute {
            events.push(Event::Button(ButtonEvent::Mute(BinaryInput {
                pressed: state.mute,
            })));
        }
        if state.touchpad != old_state.touchpad {
            events.push(Event::Button(ButtonEvent::PadPress(BinaryInput {
                pressed: state.touchpad,
            })));
        }

        // Trigger events
        if state.l2_trigger != old_state.l2_trigger {
            events.push(Event::Trigger(TriggerEvent::L2(TriggerInput {
                value: state.l2_trigger,
            })));
        }
        if state.r2_trigger != old_state.r2_trigger {
            events.push(Event::Trigger(TriggerEvent::R2(TriggerInput {
                value: state.r2_trigger,
            })));
        }

        // Axis events
        if state.joystick_l_x != old_state.joystick_l_x
            || state.joystick_l_y != old_state.joystick_l_y
        {
            events.push(Event::Axis(AxisEvent::LStick(AxisInput {
                x: state.joystick_l_x,
                y: state.joystick_l_y,
            })));
        }
        if state.joystick_r_x != old_state.joystick_r_x
            || state.joystick_r_y != old_state.joystick_r_y
        {
            events.push(Event::Axis(AxisEvent::RStick(AxisInput {
                x: state.joystick_r_x,
                y: state.joystick_r_y,
            })));
        }

        // Touch events
        if state.touch_data != old_state.touch_data {
            // Timestamp changes indicate that touches are happening
            self.last_touch = Instant::now();
            let finger_data_0 = state.touch_data.touch_finger_data[0];
            let old_finger_data_0 = old_state.touch_data.touch_finger_data[0];
            if finger_data_0 != old_finger_data_0 {
                let is_touching = finger_data_0.context <= 127;
                self.touch_state[0] = is_touching;
                events.push(Event::Axis(AxisEvent::Pad(TouchAxisInput {
                    index: 0,
                    is_touching,
                    x: finger_data_0.get_x(),
                    y: finger_data_0.get_y(),
                })))
            }
            let finger_data_1 = state.touch_data.touch_finger_data[1];
            let old_finger_data_1 = old_state.touch_data.touch_finger_data[1];
            if finger_data_1 != old_finger_data_1 {
                let is_touching = finger_data_1.context <= 127;
                self.touch_state[1] = is_touching;
                events.push(Event::Axis(AxisEvent::Pad(TouchAxisInput {
                    index: 1,
                    is_touching,
                    x: finger_data_1.get_x(),
                    y: finger_data_1.get_y(),
                })))
            }
        }

        // Accelerometer events
        events.push(Event::Accelerometer(IntertialEvent::Accelerometer(
            InertialInput {
                x: state.accel_x.to_primitive(),
                y: state.accel_y.to_primitive(),
                z: state.accel_z.to_primitive(),
            },
        )));
        events.push(Event::Accelerometer(IntertialEvent::Gyroscope(
            InertialInput {
                x: state.pitch.to_primitive(),
                y: state.yaw.to_primitive(),
                z: state.roll.to_primitive(),
            },
        )));

        events
    }
}
