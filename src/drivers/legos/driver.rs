use std::{error::Error, ffi::CString};

use hidapi::HidDevice;
use packed_struct::{types::SizedInteger, PackedStruct};

use super::{
    event::{
        AxisEvent, BinaryInput, ButtonEvent, Event, InertialEvent, InertialInput, JoyAxisInput,
        TriggerEvent, TriggerInput,
    },
    hid_report::{
        InertialInputDataReport, InputReportType, RumbleOutputDataReport, XInputDataReport,
    },
};

// Hardware ID's
pub const VID: u16 = 0x1a86;
pub const XINPUT_PID: u16 = 0xe310;
pub const DINPUT_PID: u16 = 0xe310;
pub const PIDS: [u16; 2] = [XINPUT_PID, DINPUT_PID];
// Input report sizes
const XINPUT_PACKET_SIZE: usize = 32;
const INERTIAL_PACKET_SIZE: usize = 9;
const HID_TIMEOUT: i32 = 10;
// Input report axis ranges
pub const PAD_X_MAX: f64 = 1024.0;
pub const PAD_Y_MAX: f64 = 1024.0;
pub const STICK_X_MAX: f64 = 127.0;
pub const STICK_X_MIN: f64 = -127.0;
pub const STICK_Y_MAX: f64 = 127.0;
pub const STICK_Y_MIN: f64 = -127.0;
pub const TRIGG_MAX: f64 = 255.0;
pub const GYRO_SCALE: i16 = 2;

pub struct Driver {
    /// HIDRAW device instance
    device: HidDevice,
    /// State for the IMU Accelerometer
    accel_state: Option<InertialInputDataReport>,
    /// State for the IMU Gyroscope
    gyro_state: Option<InertialInputDataReport>,
    /// State for the internal gamepad  controller
    xinput_state: Option<XInputDataReport>,
    /// Tracks if the bad data pushed when grabbing the gyro device
    bad_data_passed: bool,
}

impl Driver {
    pub fn new(path: String) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let fmtpath = path.clone();
        let path = CString::new(path)?;
        let api = hidapi::HidApi::new()?;
        let device = api.open_path(&path)?;
        let info = device.get_device_info()?;
        if info.vendor_id() != VID || !PIDS.contains(&info.product_id()) {
            return Err(format!("Device '{fmtpath}' is not a Legion Go S Controller").into());
        }
        Ok(Self {
            device,
            accel_state: None,
            gyro_state: None,
            xinput_state: None,
            bad_data_passed: false,
        })
    }

    /// Poll the device and read input reports
    pub fn poll(&mut self) -> Result<Vec<Event>, Box<dyn Error + Send + Sync>> {
        // Read data from the device into a buffer
        let mut buf = [0; XINPUT_PACKET_SIZE];
        let bytes_read = self.device.read_timeout(&mut buf[..], HID_TIMEOUT)?;

        let events = match bytes_read {
            XINPUT_PACKET_SIZE => match self.handle_xinput_report(buf) {
                Ok(events) => events,
                Err(e) => {
                    log::error!("Got error processing XinputDataReport: {e:?}");
                    vec![]
                }
            },

            INERTIAL_PACKET_SIZE => {
                let slice = &buf[..bytes_read];
                // Handle the incoming input report
                let sized_buf = slice.try_into()?;
                match self.handle_inertial_report(sized_buf) {
                    Ok(events) => events,
                    Err(e) => {
                        log::error!("Got error processing InertailInputDataReport: {e:?}");
                        vec![]
                    }
                }
            }

            _ => vec![],
        };

        Ok(events)
    }

    /// Writes the given output state to the gamepad. This can be used to change
    /// the color of LEDs, activate rumble, etc.
    pub fn write(&self, buf: &[u8]) -> Result<(), Box<dyn Error + Send + Sync>> {
        let _bytes_written = self.device.write(buf)?;

        Ok(())
    }

    pub fn haptic_rumble(
        &self,
        l_motor_speed: u8,
        r_motor_speed: u8,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let mut report = RumbleOutputDataReport::default();
        report.l_motor_speed = l_motor_speed;
        report.r_motor_speed = r_motor_speed;
        log::debug!("Got rumble event: {report:?}");

        let buf = report.pack()?;
        self.write(&buf)
    }

    /// Unpacks the buffer into a [XinputDataReport] structure and updates
    /// the internal xinput_state
    fn handle_xinput_report(
        &mut self,
        buf: [u8; XINPUT_PACKET_SIZE],
    ) -> Result<Vec<Event>, Box<dyn Error + Send + Sync>> {
        let input_report = XInputDataReport::unpack(&buf)?;

        // Hacky workaround. When the gyro device is grabbed the XInputDataReport is full of
        // garbage. Since it doesn't have a report_id we cant reject it. This only seems to hapen
        // one time, so we can save a lot of checks in the future if we clear a bool once it
        // happens.
        if !self.bad_data_passed && input_report.is_bad_data() {
            log::debug!("Got bad XInputDataReport, regecting it.");
            self.bad_data_passed = true;
            return Ok(vec![]);
        }

        // Print input report for debugging
        //log::debug!("--- Input report ---");
        //log::debug!("{input_report}");
        //log::debug!(" ---- End Report ----");

        // Update the state
        let old_input_state = self.update_xinput_state(input_report);

        // Translate the state into a stream of input events
        let events = self.translate_xinput(old_input_state);

        Ok(events)
    }

    /// Update gamepad state
    fn update_xinput_state(&mut self, input_report: XInputDataReport) -> Option<XInputDataReport> {
        let old_state = self.xinput_state;
        self.xinput_state = Some(input_report);
        old_state
    }

    /// Translate the state into individual events
    fn translate_xinput(&self, old_state: Option<XInputDataReport>) -> Vec<Event> {
        let mut events = Vec::new();
        let Some(state) = self.xinput_state else {
            return events;
        };

        // Translate state changes into events if they have changed
        let Some(old_state) = old_state else {
            return events;
        };

        // Binary Events
        if state.a != old_state.a {
            events.push(Event::Button(ButtonEvent::A(BinaryInput {
                pressed: state.a,
            })));
        }
        if state.b != old_state.b {
            events.push(Event::Button(ButtonEvent::B(BinaryInput {
                pressed: state.b,
            })));
        }
        if state.x != old_state.x {
            events.push(Event::Button(ButtonEvent::X(BinaryInput {
                pressed: state.x,
            })));
        }
        if state.y != old_state.y {
            events.push(Event::Button(ButtonEvent::Y(BinaryInput {
                pressed: state.y,
            })));
        }
        if state.menu != old_state.menu {
            events.push(Event::Button(ButtonEvent::Menu(BinaryInput {
                pressed: state.menu,
            })));
        }
        if state.view != old_state.view {
            events.push(Event::Button(ButtonEvent::View(BinaryInput {
                pressed: state.view,
            })));
        }
        if state.legion != old_state.legion {
            events.push(Event::Button(ButtonEvent::Legion(BinaryInput {
                pressed: state.legion,
            })));
        }
        if state.quick_access != old_state.quick_access {
            events.push(Event::Button(ButtonEvent::QuickAccess(BinaryInput {
                pressed: state.quick_access,
            })));
        }
        if state.down != old_state.down {
            events.push(Event::Button(ButtonEvent::DPadDown(BinaryInput {
                pressed: state.down,
            })));
        }
        if state.up != old_state.up {
            events.push(Event::Button(ButtonEvent::DPadUp(BinaryInput {
                pressed: state.up,
            })));
        }
        if state.left != old_state.left {
            events.push(Event::Button(ButtonEvent::DPadLeft(BinaryInput {
                pressed: state.left,
            })));
        }
        if state.right != old_state.right {
            events.push(Event::Button(ButtonEvent::DPadRight(BinaryInput {
                pressed: state.right,
            })));
        }
        if state.lb != old_state.lb {
            events.push(Event::Button(ButtonEvent::LB(BinaryInput {
                pressed: state.lb,
            })));
        }
        if state.rb != old_state.rb {
            events.push(Event::Button(ButtonEvent::RB(BinaryInput {
                pressed: state.rb,
            })));
        }
        if state.d_trigger_l != old_state.d_trigger_l {
            events.push(Event::Button(ButtonEvent::DTriggerL(BinaryInput {
                pressed: state.d_trigger_l,
            })));
        }
        if state.d_trigger_r != old_state.d_trigger_r {
            events.push(Event::Button(ButtonEvent::DTriggerR(BinaryInput {
                pressed: state.d_trigger_r,
            })));
        }
        if state.y1 != old_state.y1 {
            events.push(Event::Button(ButtonEvent::Y1(BinaryInput {
                pressed: state.y1,
            })));
        }
        if state.y2 != old_state.y2 {
            events.push(Event::Button(ButtonEvent::Y2(BinaryInput {
                pressed: state.y2,
            })));
        }
        if state.rpad_tap != old_state.rpad_tap {
            events.push(Event::Button(ButtonEvent::RPadTap(BinaryInput {
                pressed: state.rpad_tap,
            })));
        }
        if state.thumb_l != old_state.thumb_l {
            events.push(Event::Button(ButtonEvent::ThumbL(BinaryInput {
                pressed: state.thumb_l,
            })));
        }
        if state.thumb_r != old_state.thumb_r {
            events.push(Event::Button(ButtonEvent::ThumbR(BinaryInput {
                pressed: state.thumb_r,
            })));
        }

        // Axis events
        if state.l_stick_x != old_state.l_stick_x || state.l_stick_y != old_state.l_stick_y {
            events.push(Event::Axis(AxisEvent::LStick(JoyAxisInput {
                x: state.l_stick_x,
                y: state.l_stick_y,
            })));
        }
        if state.r_stick_x != old_state.r_stick_x || state.r_stick_y != old_state.r_stick_y {
            events.push(Event::Axis(AxisEvent::RStick(JoyAxisInput {
                x: state.r_stick_x,
                y: state.r_stick_y,
            })));
        }

        if state.a_trigger_l != old_state.a_trigger_l {
            events.push(Event::Trigger(TriggerEvent::ATriggerL(TriggerInput {
                value: state.a_trigger_l,
            })));
        }
        if state.a_trigger_r != old_state.a_trigger_r {
            events.push(Event::Trigger(TriggerEvent::ATriggerR(TriggerInput {
                value: state.a_trigger_r,
            })));
        }
        //TODO: When touchpad firmware is updated to use ABS events, enable this
        //if state.touch_x != old_state.touch_x
        //    || state.touch_y != old_state.touch_y
        //    || state.rpad_touching != old_state.rpad_touching
        //{
        //    events.push(Event::Axis(AxisEvent::Touchpad(TouchAxisInput {
        //        x: state.touch_x,
        //        y: state.touch_y,
        //        index: 0,
        //        is_touching: state.rpad_touching,
        //    })))
        //}

        events
    }

    fn handle_inertial_report(
        &mut self,
        buf: [u8; INERTIAL_PACKET_SIZE],
    ) -> Result<Vec<Event>, Box<dyn Error + Send + Sync>> {
        let input_report = InertialInputDataReport::unpack(&buf)?;

        // Print input report for debugging
        //log::debug!("--- Input report ---");
        //log::debug!("{input_report}");
        //log::debug!(" ---- End Report ----");

        let report_type = match input_report.report_id {
            1 => InputReportType::AccelData,
            2 => InputReportType::GyroData,
            _ => {
                let report_id = input_report.report_id;
                return Err(format!("Unknown report type: {report_id}").into());
            }
        };

        match report_type {
            InputReportType::AccelData => {
                // Update the state
                let old_state = self.update_accel_state(input_report);
                // Translate the state into a stream of input events
                let events = self.translate_accel_data(old_state);
                Ok(events)
            }
            InputReportType::GyroData => {
                // Update the state
                let old_state = self.update_gyro_state(input_report);
                // Translate the state into a stream of input events
                let events = self.translate_gyro_data(old_state);
                Ok(events)
            }
        }
    }

    /// Update accel_state
    fn update_accel_state(
        &mut self,
        input_report: InertialInputDataReport,
    ) -> Option<InertialInputDataReport> {
        let old_state = self.accel_state;
        self.accel_state = Some(input_report);
        old_state
    }

    /// Update gyro_state
    fn update_gyro_state(
        &mut self,
        input_report: InertialInputDataReport,
    ) -> Option<InertialInputDataReport> {
        let old_state = self.gyro_state;
        self.gyro_state = Some(input_report);
        old_state
    }

    /// Translate the accel_state into individual events
    fn translate_accel_data(&self, old_state: Option<InertialInputDataReport>) -> Vec<Event> {
        let mut events = Vec::new();
        let Some(state) = self.accel_state else {
            return events;
        };

        // Translate state changes into events if they have changed
        let Some(old_state) = old_state else {
            return events;
        };
        if state.x != old_state.x || state.y != old_state.y || state.z != old_state.z {
            events.push(Event::Inertia(InertialEvent::Accelerometer(
                InertialInput {
                    x: -state.x.to_primitive(),
                    y: -state.y.to_primitive(),
                    z: -state.z.to_primitive(),
                },
            )))
        };

        events
    }

    /// Translate the gyro_state into individual events
    fn translate_gyro_data(&self, old_state: Option<InertialInputDataReport>) -> Vec<Event> {
        let mut events = Vec::new();
        let Some(state) = self.gyro_state else {
            return events;
        };

        // Translate state changes into events if they have changed
        let Some(old_state) = old_state else {
            return events;
        };

        if state.x != old_state.x || state.y != old_state.y || state.z != old_state.z {
            events.push(Event::Inertia(InertialEvent::Gyro(InertialInput {
                x: -state.x.to_primitive() * GYRO_SCALE,
                y: -state.y.to_primitive() * GYRO_SCALE,
                z: -state.z.to_primitive() * GYRO_SCALE,
            })))
        };

        events
    }
}
