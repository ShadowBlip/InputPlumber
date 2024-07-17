use packed_struct::prelude::*;
use std::{error::Error, ffi::CString};

use hidapi::HidDevice;

use crate::drivers::dualsense::{
    event::{BinaryInput, ButtonEvent, TriggerEvent, TriggerInput},
    hid_report::Direction,
};

use super::{
    event::{AccelerometerEvent, AccelerometerInput, AxisEvent, AxisInput, Event, TouchAxisInput},
    hid_report::{PackedInputDataReport, SetStatePackedOutputData, UsbPackedOutputReport},
};

// Source: https://github.com/torvalds/linux/blob/master/drivers/hid/hid-playstation.c
pub const DS5_EDGE_NAME: &str = "Sony Interactive Entertainment DualSense Edge Wireless Controller";
pub const DS5_EDGE_VERSION: u16 = 256;
pub const DS5_EDGE_VID: u16 = 0x054c;
pub const DS5_EDGE_PID: u16 = 0x0df2;

pub const DS5_NAME: &str = "Sony Interactive Entertainment DualSense Wireless Controller";
pub const DS5_VERSION: u16 = 0x8111;
pub const DS5_VID: u16 = 0x054c;
pub const DS5_PID: u16 = 0x0ce6;

pub const PIDS: [u16; 2] = [DS5_EDGE_PID, DS5_PID];

pub const FEATURE_REPORT_PAIRING_INFO: u8 = 0x09;
pub const FEATURE_REPORT_PAIRING_INFO_SIZE: u8 = 20;
pub const FEATURE_REPORT_FIRMWARE_INFO: u8 = 0x20;
pub const FEATURE_REPORT_FIRMWARE_INFO_SIZE: u8 = 64;
pub const FEATURE_REPORT_CALIBRATION: u8 = 0x05;
pub const FEATURE_REPORT_CALIBRATION_SIZE: u8 = 41;

pub const INPUT_REPORT_USB: u8 = 0x01;
pub const INPUT_REPORT_USB_SIZE: usize = 64;
pub const INPUT_REPORT_BT: u8 = 0x31;
pub const INPUT_REPORT_BT_SIZE: usize = 78;
pub const OUTPUT_REPORT_USB: u8 = 0x02;
pub const OUTPUT_REPORT_USB_SIZE: usize = 63;
pub const OUTPUT_REPORT_USB_SHORT_SIZE: usize = 48;
pub const OUTPUT_REPORT_BT: u8 = 0x31;
pub const OUTPUT_REPORT_BT_SIZE: usize = 78;

// Input report axis ranges
pub const STICK_X_MIN: f64 = u8::MIN as f64;
pub const STICK_X_MAX: f64 = u8::MAX as f64;
pub const STICK_Y_MIN: f64 = u8::MIN as f64;
pub const STICK_Y_MAX: f64 = u8::MAX as f64;
pub const TRIGGER_MAX: f64 = u8::MAX as f64;

//#define DS5_SPEC_DELTA_TIME         4096.0f

// DualSense hardware limits
pub const DS5_ACC_RES_PER_G: u32 = 8192;
pub const DS5_ACC_RANGE: u32 = 4 * DS5_ACC_RES_PER_G;
pub const DS5_GYRO_RES_PER_DEG_S: u32 = 1024;
pub const DS5_GYRO_RANGE: u32 = 2048 * DS5_GYRO_RES_PER_DEG_S;
pub const DS5_TOUCHPAD_WIDTH: f64 = 1920.0;
pub const DS5_TOUCHPAD_HEIGHT: f64 = 1080.0;

/// PS5 Dualsense controller driver for reading gamepad input
pub struct Driver {
    state: Option<PackedInputDataReport>,
    device: HidDevice,
    leds_initialized: bool,
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

        Ok(Self {
            device,
            state: None,
            leds_initialized: false,
        })
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
    fn translate(&self, old_state: Option<PackedInputDataReport>) -> Vec<Event> {
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
            match state.dpad {
                Direction::North => events.push(Event::Button(ButtonEvent::DPadUp(BinaryInput {
                    pressed: true,
                }))),
                Direction::NorthEast => {
                    events.push(Event::Button(ButtonEvent::DPadUp(BinaryInput {
                        pressed: true,
                    })));
                    events.push(Event::Button(ButtonEvent::DPadRight(BinaryInput {
                        pressed: true,
                    })));
                }
                Direction::East => {
                    events.push(Event::Button(ButtonEvent::DPadRight(BinaryInput {
                        pressed: true,
                    })))
                }
                Direction::SouthEast => {
                    events.push(Event::Button(ButtonEvent::DPadRight(BinaryInput {
                        pressed: true,
                    })));
                    events.push(Event::Button(ButtonEvent::DPadDown(BinaryInput {
                        pressed: true,
                    })))
                }
                Direction::South => {
                    events.push(Event::Button(ButtonEvent::DPadDown(BinaryInput {
                        pressed: true,
                    })))
                }
                Direction::SouthWest => {
                    events.push(Event::Button(ButtonEvent::DPadDown(BinaryInput {
                        pressed: true,
                    })));
                    events.push(Event::Button(ButtonEvent::DPadLeft(BinaryInput {
                        pressed: true,
                    })))
                }
                Direction::West => events.push(Event::Button(ButtonEvent::DPadLeft(BinaryInput {
                    pressed: true,
                }))),
                Direction::NorthWest => {
                    events.push(Event::Button(ButtonEvent::DPadLeft(BinaryInput {
                        pressed: true,
                    })));
                    events.push(Event::Button(ButtonEvent::DPadUp(BinaryInput {
                        pressed: true,
                    })))
                }
                Direction::None => match old_state.dpad {
                    Direction::North => {
                        events.push(Event::Button(ButtonEvent::DPadUp(BinaryInput {
                            pressed: false,
                        })))
                    }
                    Direction::NorthEast => {
                        events.push(Event::Button(ButtonEvent::DPadUp(BinaryInput {
                            pressed: false,
                        })));
                        events.push(Event::Button(ButtonEvent::DPadRight(BinaryInput {
                            pressed: false,
                        })));
                    }
                    Direction::East => {
                        events.push(Event::Button(ButtonEvent::DPadRight(BinaryInput {
                            pressed: false,
                        })))
                    }
                    Direction::SouthEast => {
                        events.push(Event::Button(ButtonEvent::DPadRight(BinaryInput {
                            pressed: false,
                        })));
                        events.push(Event::Button(ButtonEvent::DPadDown(BinaryInput {
                            pressed: false,
                        })))
                    }
                    Direction::South => {
                        events.push(Event::Button(ButtonEvent::DPadDown(BinaryInput {
                            pressed: false,
                        })))
                    }
                    Direction::SouthWest => {
                        events.push(Event::Button(ButtonEvent::DPadDown(BinaryInput {
                            pressed: false,
                        })));
                        events.push(Event::Button(ButtonEvent::DPadLeft(BinaryInput {
                            pressed: false,
                        })))
                    }
                    Direction::West => {
                        events.push(Event::Button(ButtonEvent::DPadLeft(BinaryInput {
                            pressed: false,
                        })))
                    }
                    Direction::NorthWest => {
                        events.push(Event::Button(ButtonEvent::DPadLeft(BinaryInput {
                            pressed: false,
                        })));
                        events.push(Event::Button(ButtonEvent::DPadUp(BinaryInput {
                            pressed: false,
                        })))
                    }
                    Direction::None => (),
                },
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
        let finger_data_0 = state.touch_data.touch_finger_data[0];
        let old_finger_data_0 = old_state.touch_data.touch_finger_data[0];
        if finger_data_0 != old_finger_data_0 {
            events.push(Event::Axis(AxisEvent::Pad(TouchAxisInput {
                index: 0,
                is_touching: finger_data_0.context == 127, // Set to 127 when touching
                x: finger_data_0.get_x(),
                y: finger_data_0.get_y(),
            })))
        }
        let finger_data_1 = state.touch_data.touch_finger_data[1];
        let old_finger_data_1 = old_state.touch_data.touch_finger_data[1];
        if finger_data_1 != old_finger_data_1 {
            events.push(Event::Axis(AxisEvent::Pad(TouchAxisInput {
                index: 1,
                is_touching: finger_data_1.context == 127, // Set to 127 when touching
                x: finger_data_1.get_x(),
                y: finger_data_1.get_y(),
            })))
        }

        // Accelerometer events
        events.push(Event::Accelerometer(AccelerometerEvent::Accelerometer(
            AccelerometerInput {
                x: state.accel_x.to_primitive(),
                y: state.accel_y.to_primitive(),
                z: state.accel_z.to_primitive(),
            },
        )));
        events.push(Event::Accelerometer(AccelerometerEvent::Gyro(
            AccelerometerInput {
                x: state.gyro_x.to_primitive(),
                y: state.gyro_y.to_primitive(),
                z: state.gyro_z.to_primitive(),
            },
        )));

        events
    }
}
