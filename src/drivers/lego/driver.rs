use std::{error::Error, ffi::CString, u8, vec};

use hidapi::HidDevice;
use packed_struct::{types::SizedInteger, PackedStruct};

use super::{
    event::{
        AxisEvent, BinaryInput, ButtonEvent, Event, GyroEvent, GyroInput, JoyAxisInput,
        MouseAxisInput, MouseButtonEvent, MouseWheelInput, StatusEvent, StatusInput,
        TouchAxisInput, TriggerEvent, TriggerInput,
    },
    hid_report::{
        DInputDataLeftReport, DInputDataRightReport, KeyboardDataReport, MouseDataReport,
        TouchpadDataReport, XInputDataReport,
    },
};

pub const VID: u16 = 0x17ef;
pub const PID: u16 = 0x6182;
pub const PID2: u16 = 0x6185;

const DINPUT_PACKET_SIZE: usize = 13;
const XINPUT_PACKET_SIZE: usize = 60;
const KEYBOARD_PACKET_SIZE: usize = 15;
const MOUSE_PACKET_SIZE: usize = 7;
const TOUCHPAD_PACKET_SIZE: usize = 20;
const HID_TIMEOUT: i32 = 5000;

pub const DINPUTLEFT_DATA: u8 = 0x07;
pub const DINPUTRIGHT_DATA: u8 = 0x08;
pub const KEYBOARD_TOUCH_DATA: u8 = 0x01;
pub const MOUSEFPS_DATA: u8 = 0x02;
//pub const MOUSE_DATA: u8 = 0x09;
pub const XINPUT_DATA: u8 = 0x04;

// Input report axis ranges
// TODO: actual mouse range
// TODO: ACCEL Left/Right X/Y, Z?
//pub const GYRO_X_MAX: f64 = 255.0;
//pub const GYRO_X_MIN: f64 = 0.0;
//pub const GYRO_Y_MAX: f64 = 255.0;
//pub const GYRO_Y_MIN: f64 = 0.0;
pub const MOUSE_WHEEL_MAX: f64 = 120.0;
//pub const MOUSE_WHEEL_MIN: f64 = -120.0;
pub const MOUSE_X_MAX: f64 = 2048.0;
pub const MOUSE_X_MIN: f64 = -2048.0;
pub const MOUSE_Y_MAX: f64 = 2048.0;
pub const MOUSE_Y_MIN: f64 = -2048.0;
pub const PAD_X_MAX: f64 = 1024.0;
pub const PAD_X_MIN: f64 = 0.0;
pub const PAD_Y_MAX: f64 = 1024.0;
pub const PAD_Y_MIN: f64 = 0.0;
pub const STICK_X_MAX: f64 = 255.0;
pub const STICK_X_MIN: f64 = 0.0;
pub const STICK_Y_MAX: f64 = 255.0;
pub const STICK_Y_MIN: f64 = 0.0;
pub const TRIGG_MAX: f64 = 255.0;
//pub const TRIGG_MIN: f64 = 0.0;

// NORMALIZED AXIS
//pub const GYRO_X_NORM: f64 = 1.0 / GYRO_X_MAX;
//pub const ACCEL_Y_NORM: f64 = 1.0 / GYRO_Y_MAX;
//pub const MOUSE_WHEEL_NORM: f64 = 1.0 / MOUSE_WHEEL_MAX;
//pub const MOUSE_X_NORM: f64 = 1.0 / MOUSE_X_MAX;
//pub const MOUSE_Y_NORM: f64 = 1.0 / MOUSE_Y_MAX;
//pub const PAD_X_AXIS_NORM: f64 = 1.0 / PAD_X_MAX;
//pub const PAD_Y_AXIS_NORM: f64 = 1.0 / PAD_Y_MAX;
//pub const STICK_X_AXIS_NORM: f64 = 1.0 / STICK_X_MAX;
//pub const STICK_Y_AXIS_NORM: f64 = 1.0 / STICK_Y_MAX;
//pub const TRIGG_AXIS_NORM: f64 = 1.0 / TRIGG_MAX;

pub struct Driver {
    dinputl_state: Option<DInputDataLeftReport>,
    dinputr_state: Option<DInputDataRightReport>,
    keyboard_state: Option<KeyboardDataReport>,
    mouse_state: Option<MouseDataReport>,
    touchpad_state: Option<TouchpadDataReport>,
    xinput_state: Option<XInputDataReport>,
    device: HidDevice,
}

impl Driver {
    pub fn new(path: String) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let fmtpath = path.clone();
        let path = CString::new(path)?;
        let api = hidapi::HidApi::new()?;
        let device = api.open_path(&path)?;
        let info = device.get_device_info()?;
        if info.vendor_id() != VID || (info.product_id() != PID && info.product_id() != PID2) {
            return Err(format!("Device '{fmtpath}' is not a Legion Go Controller").into());
        }

        Ok(Self {
            device,
            dinputl_state: None,
            dinputr_state: None,
            xinput_state: None,
            keyboard_state: None,
            mouse_state: None,
            touchpad_state: None,
        })
    }

    /// Poll the device and read input reports
    pub fn poll(&mut self) -> Result<Vec<Event>, Box<dyn Error + Send + Sync>> {
        // Read data from the device into a buffer
        let mut buf = [0; XINPUT_PACKET_SIZE];
        let bytes_read = self.device.read_timeout(&mut buf[..], HID_TIMEOUT)?;

        let report_id = buf[0];
        let slice = &buf[..bytes_read];
        //log::trace!("Got Report ID: {report_id}");
        //log::trace!("Got Report Size: {bytes_read}");

        match report_id {
            DINPUTLEFT_DATA => {
                if bytes_read != DINPUT_PACKET_SIZE {
                    return Err("Invalid packet size for Direct Input Data.".into());
                }
                // Handle the incoming input report
                let sized_buf = slice.try_into()?;
                let events = self.handle_dinputl_report(sized_buf)?;
                Ok(events)
            }

            DINPUTRIGHT_DATA => {
                if bytes_read != DINPUT_PACKET_SIZE {
                    return Err("Invalid packet size for Direct Input Data.".into());
                }
                // Handle the incoming input report
                let sized_buf = slice.try_into()?;
                let events = self.handle_dinputr_report(sized_buf)?;
                Ok(events)
            }

            KEYBOARD_TOUCH_DATA => {
                log::trace!("Got keyboard/touch data.");
                if bytes_read != KEYBOARD_PACKET_SIZE {
                    if bytes_read != TOUCHPAD_PACKET_SIZE {
                        return Err("Invalid packet size for Keyboard or Touchpad Data.".into());
                    }
                    // Handle the incoming input report
                    let sized_buf = slice.try_into()?;
                    let events = self.handle_touchinput_report(sized_buf)?;
                    Ok(events)
                } else {
                    // Handle the incoming input report
                    let sized_buf = slice.try_into()?;
                    let events = self.handle_keyboard_report(sized_buf)?;
                    Ok(events)
                }
            }

            MOUSEFPS_DATA => {
                if bytes_read != MOUSE_PACKET_SIZE {
                    return Err("Invalid packet size for Mouse Data.".into());
                }
                // Handle the incoming input report
                let sized_buf = slice.try_into()?;
                let events = self.handle_mouseinput_report(sized_buf)?;
                Ok(events)
            }

            XINPUT_DATA => {
                if bytes_read != XINPUT_PACKET_SIZE {
                    return Err("Invalid packet size for X-Input Data.".into());
                }
                // Handle the incoming input report
                let sized_buf = slice.try_into()?;
                let events = self.handle_xinput_report(sized_buf)?;
                Ok(events)
            }
            _ => {
                log::trace!("Invalid Report ID.");
                let events = vec![];
                Ok(events)
            }
        }
    }
    /// Unpacks the buffer into a [DInputDataReport] structure and updates
    /// the internal dinput_state
    fn handle_dinputl_report(
        &mut self,
        buf: [u8; DINPUT_PACKET_SIZE],
    ) -> Result<Vec<Event>, Box<dyn Error + Send + Sync>> {
        let input_report = DInputDataLeftReport::unpack(&buf)?;

        // Print input report for debugging
        // log::trace!("--- Input report ---");
        // log::trace!("{input_report}");
        // log::trace!("---- End Report ----");

        // Update the state
        let old_dinput_state = self.update_dinputl_state(input_report);

        // Translate the state into a stream of input events
        let events = self.translate_dinputl(old_dinput_state);

        Ok(events)
    }

    /// Update dinput state
    fn update_dinputl_state(
        &mut self,
        input_report: DInputDataLeftReport,
    ) -> Option<DInputDataLeftReport> {
        let old_state = self.dinputl_state;
        self.dinputl_state = Some(input_report);
        old_state
    }

    /// Translate the state into individual events
    fn translate_dinputl(&self, old_state: Option<DInputDataLeftReport>) -> Vec<Event> {
        let events = Vec::new();
        let Some(_) = self.dinputl_state else {
            return events;
        };

        // Translate state changes into events if they have changed
        if let Some(_) = old_state {}
        events
    }

    /// Unpacks the buffer into a [DInputDataReport] structure and updates
    /// the internal dinput_state
    fn handle_dinputr_report(
        &mut self,
        buf: [u8; DINPUT_PACKET_SIZE],
    ) -> Result<Vec<Event>, Box<dyn Error + Send + Sync>> {
        let input_report = DInputDataRightReport::unpack(&buf)?;

        // Print input report for debugging
        // log::trace!("--- Input report ---");
        // log::trace!("{input_report}");
        // log::trace!("---- End Report ----");

        // Update the state
        let old_dinput_state = self.update_dinputr_state(input_report);

        // Translate the state into a stream of input events
        let events = self.translate_dinputr(old_dinput_state);

        Ok(events)
    }

    /// Update dinput state
    fn update_dinputr_state(
        &mut self,
        input_report: DInputDataRightReport,
    ) -> Option<DInputDataRightReport> {
        let old_state = self.dinputr_state;
        self.dinputr_state = Some(input_report);
        old_state
    }

    /// Translate the state into individual events
    fn translate_dinputr(&self, old_state: Option<DInputDataRightReport>) -> Vec<Event> {
        let events = Vec::new();
        let Some(_) = self.dinputr_state else {
            return events;
        };

        // Translate state changes into events if they have changed
        if let Some(_) = old_state {}
        events
    }

    /// Unpacks the buffer into a [KeyboardDataReport] structure and updates
    /// the internal keyboard_state
    fn handle_keyboard_report(
        &mut self,
        buf: [u8; KEYBOARD_PACKET_SIZE],
    ) -> Result<Vec<Event>, Box<dyn Error + Send + Sync>> {
        let input_report = KeyboardDataReport::unpack(&buf)?;

        // Print input report for debugging
        // log::trace!("--- Input report ---");
        // log::trace!("{input_report}");
        // log::trace!("---- End Report ----");

        // Update the state
        let old_dinput_state = self.update_keyboard_state(input_report);

        // Translate the state into a stream of input events
        let events = self.translate_keyboard(old_dinput_state);

        Ok(events)
    }

    /// Update keyboard state
    fn update_keyboard_state(
        &mut self,
        input_report: KeyboardDataReport,
    ) -> Option<KeyboardDataReport> {
        let old_state = self.keyboard_state;
        self.keyboard_state = Some(input_report);
        old_state
    }

    /// Translate the state into individual events
    fn translate_keyboard(&self, old_state: Option<KeyboardDataReport>) -> Vec<Event> {
        let events = Vec::new();
        let Some(_) = self.keyboard_state else {
            return events;
        };

        // Translate state changes into events if they have changed
        if let Some(_) = old_state {}
        events
    }

    /// Unpacks the buffer into a [MouseDataReport] structure and updates
    /// the internal mouse_state
    fn handle_mouseinput_report(
        &mut self,
        buf: [u8; MOUSE_PACKET_SIZE],
    ) -> Result<Vec<Event>, Box<dyn Error + Send + Sync>> {
        let input_report = MouseDataReport::unpack(&buf)?;

        // Print input report for debugging
        //log::trace!("--- Input report ---");
        //log::trace!("{input_report}");
        //log::trace!("---- End Report ----");

        // Update the state
        let old_dinput_state = self.update_mouseinput_state(input_report);

        // Translate the state into a stream of input events
        let events = self.translate_mouse(old_dinput_state);

        Ok(events)
    }

    /// Update mouseinput state
    fn update_mouseinput_state(
        &mut self,
        input_report: MouseDataReport,
    ) -> Option<MouseDataReport> {
        let old_state = self.mouse_state;
        self.mouse_state = Some(input_report);
        old_state
    }

    /// Translate the state into individual events
    fn translate_mouse(&self, old_state: Option<MouseDataReport>) -> Vec<Event> {
        let mut events = Vec::new();
        let Some(state) = self.mouse_state else {
            return events;
        };

        // Translate state changes into events if they have changed
        if let Some(old_state) = old_state {
            // Button Events
            if state.y3 != old_state.y3 {
                events.push(Event::MouseButton(MouseButtonEvent::Y3(BinaryInput {
                    pressed: state.y3,
                })));
            }
            if state.m3 != old_state.m3 {
                events.push(Event::MouseButton(MouseButtonEvent::M3(BinaryInput {
                    pressed: state.m3,
                })));
            }
            if state.mouse_click != old_state.mouse_click {
                events.push(Event::MouseButton(MouseButtonEvent::MouseClick(
                    BinaryInput {
                        pressed: state.mouse_click,
                    },
                )));
            }
            if state.m2 != old_state.m2 {
                events.push(Event::MouseButton(MouseButtonEvent::M2(BinaryInput {
                    pressed: state.m2,
                })));
            }
            if state.m1 != old_state.m1 {
                events.push(Event::MouseButton(MouseButtonEvent::M1(BinaryInput {
                    pressed: state.m1,
                })));
            }

            // Axis events
            if state.mouse_x != old_state.mouse_x || state.mouse_y != old_state.mouse_y {
                events.push(Event::Axis(AxisEvent::Mouse(MouseAxisInput {
                    x: state.mouse_x.to_primitive(),
                    y: state.mouse_y.to_primitive(),
                })));
            }
            if state.mouse_z != old_state.mouse_z {
                events.push(Event::Trigger(TriggerEvent::MouseWheel(MouseWheelInput {
                    value: state.mouse_z,
                })));
            }
        }
        events
    }

    /// Unpacks the buffer into a [TouchpadDataReport] structure and updates
    /// the internal touchpad_state
    fn handle_touchinput_report(
        &mut self,
        buf: [u8; TOUCHPAD_PACKET_SIZE],
    ) -> Result<Vec<Event>, Box<dyn Error + Send + Sync>> {
        let input_report = TouchpadDataReport::unpack(&buf)?;

        // Print input report for debugging
        //log::trace!("--- Input report ---");
        //log::trace!("{input_report}");
        //log::trace!("---- End Report ----");

        // Update the state
        let old_dinput_state = self.update_touchpad_state(input_report);

        // Translate the state into a stream of input events
        let events = self.translate_touch(old_dinput_state);

        Ok(events)
    }

    /// Update touchinput state
    fn update_touchpad_state(
        &mut self,
        input_report: TouchpadDataReport,
    ) -> Option<TouchpadDataReport> {
        let old_state = self.touchpad_state;
        self.touchpad_state = Some(input_report);
        old_state
    }

    /// Translate the state into individual events
    fn translate_touch(&self, old_state: Option<TouchpadDataReport>) -> Vec<Event> {
        let events = Vec::new();
        let Some(_) = self.touchpad_state else {
            return events;
        };

        // Translate state changes into events if they have changed
        if let Some(_) = old_state {}
        events
    }

    /// Unpacks the buffer into a [XinputDataReport] structure and updates
    /// the internal xinput_state
    fn handle_xinput_report(
        &mut self,
        buf: [u8; XINPUT_PACKET_SIZE],
    ) -> Result<Vec<Event>, Box<dyn Error + Send + Sync>> {
        let input_report = XInputDataReport::unpack(&buf)?;

        // Print input report for debugging
        //log::trace!("--- Input report ---");
        //log::trace!("{input_report}");
        //log::trace!(" ---- End Report ----");

        // Update the state
        let old_dinput_state = self.update_xinput_state(input_report);

        // Translate the state into a stream of input events
        let events = self.translate_xinput(old_dinput_state);

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
        if let Some(old_state) = old_state {
            if state.gamepad_mode != old_state.gamepad_mode {
                log::debug!(
                    "Changed gamepad mode from {} to {}",
                    old_state.gamepad_mode,
                    state.gamepad_mode
                );
            }
            if state.gamepad_mode == 2 {
                log::debug!("In FPS Mode, rejecting gamepad input.");
                return events;
            }
            // Binary events
            if state.a != old_state.a {
                events.push(Event::Button(ButtonEvent::A(BinaryInput {
                    pressed: state.a,
                })));
            }
            if state.x != old_state.x {
                events.push(Event::Button(ButtonEvent::X(BinaryInput {
                    pressed: state.x,
                })));
            }
            if state.b != old_state.b {
                events.push(Event::Button(ButtonEvent::B(BinaryInput {
                    pressed: state.b,
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
            if state.m2 != old_state.m2 {
                events.push(Event::Button(ButtonEvent::M2(BinaryInput {
                    pressed: state.m2,
                })));
            }
            if state.m3 != old_state.m3 {
                events.push(Event::Button(ButtonEvent::M3(BinaryInput {
                    pressed: state.m3,
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
            if state.y3 != old_state.y3 {
                events.push(Event::Button(ButtonEvent::Y3(BinaryInput {
                    pressed: state.y3,
                })));
            }
            if state.mouse_click != old_state.mouse_click {
                events.push(Event::Button(ButtonEvent::MouseClick(BinaryInput {
                    pressed: state.mouse_click,
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
            if state.touch_x_0 != old_state.touch_x_0 || state.touch_y_0 != old_state.touch_y_0 {
                events.push(Event::Axis(AxisEvent::Touchpad(TouchAxisInput {
                    x: state.touch_x_0,
                    y: state.touch_y_0,
                })));
            }
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

            // Trigger events
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
            if state.mouse_z != old_state.mouse_z {
                events.push(Event::Trigger(TriggerEvent::MouseWheel(MouseWheelInput {
                    value: state.mouse_z,
                })));
            }

            // Accelerometer events
            events.push(Event::Gyro(GyroEvent::LeftGyro(GyroInput {
                x: state.left_gyro_x,
                y: state.left_gyro_y,
                z: 0,
            })));
            events.push(Event::Gyro(GyroEvent::RightGyro(GyroInput {
                x: state.right_gyro_x,
                y: state.right_gyro_y,
                z: 0,
            })));

            // Status events
            if state.l_controller_battery != old_state.l_controller_battery {
                events.push(Event::Status(StatusEvent::LeftControllerBattery(
                    StatusInput {
                        value: state.l_controller_battery,
                    },
                )));
            }
            if state.l_controller_mode0 != old_state.l_controller_mode0 {
                events.push(Event::Status(StatusEvent::LeftControllerMode0(
                    StatusInput {
                        value: state.l_controller_mode0,
                    },
                )));
            }
            if state.l_controller_mode1 != old_state.l_controller_mode1 {
                events.push(Event::Status(StatusEvent::LeftControllerMode1(
                    StatusInput {
                        value: state.l_controller_mode1,
                    },
                )));
            }
            if state.r_controller_battery != old_state.r_controller_battery {
                events.push(Event::Status(StatusEvent::RightControllerBattery(
                    StatusInput {
                        value: state.r_controller_battery,
                    },
                )));
            }
            if state.r_controller_mode0 != old_state.r_controller_mode0 {
                events.push(Event::Status(StatusEvent::RightControllerMode0(
                    StatusInput {
                        value: state.r_controller_mode0,
                    },
                )));
            }
            if state.r_controller_mode1 != old_state.r_controller_mode1 {
                events.push(Event::Status(StatusEvent::RightControllerMode1(
                    StatusInput {
                        value: state.r_controller_mode1,
                    },
                )));
            }
        };

        events
    }
}
