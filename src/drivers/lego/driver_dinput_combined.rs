use std::{
    error::Error,
    ffi::CString,
    time::{Duration, Instant},
};

use hidapi::HidDevice;
use packed_struct::{types::SizedInteger, PackedStruct};

use super::{
    event::{
        AxisEvent, BinaryInput, Event, GamepadButtonEvent, JoyAxisInput, MouseWheelInput,
        StatusEvent, StatusInput, TouchAxisInput, TouchButtonEvent, TriggerEvent, TriggerInput,
    },
    hid_report::{DInputDataFullReport, DPadDirection, TouchpadDataReport, XInputDataReport},
};

// Hardware ID's
pub const VID: u16 = 0x17ef;
pub const PID: u16 = 0x6183;

// Report ID's
pub const KEYBOARD_TOUCH_DATA: u8 = 0x01;
pub const XINPUT_DATA: u8 = 0x04;
pub const DINPUT_DATA: u8 = 0x07;

// Input report sizes
const DINPUT_PACKET_SIZE: usize = 13;
const XINPUT_PACKET_SIZE: usize = 60;
const TOUCHPAD_PACKET_SIZE: usize = 20;
const HID_TIMEOUT: i32 = 10;

// Input report axis ranges
pub const PAD_X_MAX: f64 = 1024.0;
pub const PAD_Y_MAX: f64 = 1024.0;
pub const STICK_X_MAX: f64 = 255.0;
pub const STICK_X_MIN: f64 = 0.0;
pub const STICK_Y_MAX: f64 = 255.0;
pub const STICK_Y_MIN: f64 = 0.0;
pub const TRIGG_MAX: f64 = 255.0;

pub struct Driver {
    /// State for the left controller when detached in dinput mode
    dinput_state: Option<DInputDataFullReport>,
    /// State for the touchpad device
    touchpad_state: Option<TouchpadDataReport>,
    /// State for the internal gamepad  controller
    xinput_state: Option<XInputDataReport>,
    /// HIDRAW device instance
    device: HidDevice,
    /// Timestamp of the first touch event. Used to detect tap-to-click events
    first_touch: Instant,
    /// Timestamp of the last touch event.
    last_touch: Instant,
    /// Whether or not we are detecting a touch event currently.
    is_touching: bool,
    /// Whether or not we are currently holding a tap-to-click.
    is_tapped: bool,
}

impl Driver {
    pub fn new(path: String) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let fmtpath = path.clone();
        let path = CString::new(path)?;
        let api = hidapi::HidApi::new()?;
        let device = api.open_path(&path)?;
        let info = device.get_device_info()?;
        if info.vendor_id() != VID || info.product_id() != PID {
            return Err(format!("Device '{fmtpath}' is not a Legion Go Controller").into());
        }

        Ok(Self {
            device,
            dinput_state: None,
            first_touch: Instant::now(),
            is_tapped: false,
            is_touching: false,
            last_touch: Instant::now(),
            touchpad_state: None,
            xinput_state: None,
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

        let mut events = match report_id {
            DINPUT_DATA => {
                if bytes_read != DINPUT_PACKET_SIZE {
                    return Err("Invalid packet size for Direct Input Data.".into());
                }
                // Handle the incoming input report
                let sized_buf = slice.try_into()?;

                self.handle_dinput_report(sized_buf)?
            }

            KEYBOARD_TOUCH_DATA => {
                if bytes_read != TOUCHPAD_PACKET_SIZE {
                    return Err("Invalid packet size for Keyboard or Touchpad Data.".into());
                }
                // Handle the incoming input report
                let sized_buf = slice.try_into()?;

                self.handle_touchinput_report(sized_buf)?
            }

            XINPUT_DATA => {
                if bytes_read != XINPUT_PACKET_SIZE {
                    return Err("Invalid packet size for X-Input Data.".into());
                }
                // Handle the incoming input report
                let sized_buf = slice.try_into()?;

                self.handle_xinput_report(sized_buf)?
            }
            _ => {
                //log::trace!("Invalid Report ID.");
                let events = vec![];
                events
            }
        };

        // There is no release event, so check to see if we are still touching.
        if self.is_touching && (self.last_touch.elapsed() > Duration::from_millis(4)) {
            let event: Event = self.release_touch();
            events.push(event);
            // Check for tap events
            if self.first_touch.elapsed() < Duration::from_millis(200) {
                // For double clicking, ensure the previous tap is cleared.
                if self.is_tapped {
                    let event: Event = self.release_tap();
                    events.push(event);
                }
                let event: Event = self.start_tap();
                events.push(event);
            }
        }

        // If we did a click event, see if we shoudl release it. Accounts for click and drag.
        if !self.is_touching
            && self.is_tapped
            && (self.last_touch.elapsed() > Duration::from_millis(100))
        {
            let event: Event = self.release_tap();
            events.push(event);
        }

        Ok(events)
    }

    /// Converts a 0-4096 dinput x axis into a 0-255 xinput axis
    fn normalize_x_axis(&self, x_axis_sm: u16, x_axis_lg: u16) -> u8 {
        let axis = (x_axis_sm << 8 | x_axis_lg) as i16;
        (axis / 16) as u8
    }

    /// Converts a 0-4096 dinput y axis into a 0-255 xinput axis
    fn normalize_y_axis(&self, y_axis_sm: u16, y_axis_lg: u16) -> u8 {
        let axis = (y_axis_lg << 4 | y_axis_sm) as i16;
        (axis / 16) as u8
    }

    /// Unpacks the buffer into a [DInputDataReport] structure and updates
    /// the internal dinput_state
    fn handle_dinput_report(
        &mut self,
        buf: [u8; DINPUT_PACKET_SIZE],
    ) -> Result<Vec<Event>, Box<dyn Error + Send + Sync>> {
        let input_report = DInputDataFullReport::unpack(&buf)?;

        // Print input report for debugging
        //log::debug!("--- Input report ---");
        //log::debug!("{input_report}");
        //log::debug!("---- End Report ----");

        // Update the state
        let old_dinput_state = self.update_dinput_state(input_report);

        // Translate the state into a stream of input events
        let events = self.translate_dinput(old_dinput_state);

        Ok(events)
    }

    /// Update dinputl state
    fn update_dinput_state(
        &mut self,
        input_report: DInputDataFullReport,
    ) -> Option<DInputDataFullReport> {
        let old_state = self.dinput_state;
        self.dinput_state = Some(input_report);
        old_state
    }

    /// Translate the state into individual events
    fn translate_dinput(&mut self, old_state: Option<DInputDataFullReport>) -> Vec<Event> {
        let mut events = Vec::new();
        let Some(state) = self.dinput_state else {
            return events;
        };

        // Translate state changes into events if they have changed
        if let Some(old_state) = old_state {
            // Pressing the "legion"" or "settings" buttons in this mode produces an impossible
            // axis combination that we can ignore. Both buttons produce the same event to there
            // isn't a way to use this.
            if state.l_stick_x_sm.to_primitive() == 0
                && state.l_stick_x_lg == 0
                && state.l_stick_y_sm.to_primitive() == 0
                && state.l_stick_y_lg == 0
                && state.r_stick_x_sm.to_primitive() == 0
                && state.r_stick_x_lg == 0
                && state.r_stick_y_sm.to_primitive() == 0
                && state.r_stick_y_lg == 0
            {
                //log::debug!("Impossible joystick position. Ignoring event.");
                return events;
            }

            // Binary Events
            if state.a != old_state.a {
                events.push(Event::GamepadButton(GamepadButtonEvent::A(BinaryInput {
                    pressed: state.a,
                })));
            }
            if state.b != old_state.b {
                events.push(Event::GamepadButton(GamepadButtonEvent::B(BinaryInput {
                    pressed: state.b,
                })));
            }
            if state.x != old_state.x {
                events.push(Event::GamepadButton(GamepadButtonEvent::X(BinaryInput {
                    pressed: state.x,
                })));
            }
            if state.y != old_state.y {
                events.push(Event::GamepadButton(GamepadButtonEvent::Y(BinaryInput {
                    pressed: state.y,
                })));
            }
            if state.rb != old_state.rb {
                events.push(Event::GamepadButton(GamepadButtonEvent::RB(BinaryInput {
                    pressed: state.rb,
                })));
            }
            if state.lb != old_state.lb {
                events.push(Event::GamepadButton(GamepadButtonEvent::LB(BinaryInput {
                    pressed: state.lb,
                })));
            }
            if state.rt != old_state.rt {
                events.push(Event::GamepadButton(GamepadButtonEvent::DTriggerR(
                    BinaryInput { pressed: state.rt },
                )));
            }
            if state.lt != old_state.lt {
                events.push(Event::GamepadButton(GamepadButtonEvent::DTriggerL(
                    BinaryInput { pressed: state.lt },
                )));
            }
            if state.menu != old_state.menu {
                events.push(Event::GamepadButton(GamepadButtonEvent::Menu(
                    BinaryInput {
                        pressed: state.menu,
                    },
                )));
            }
            if state.view != old_state.view {
                events.push(Event::GamepadButton(GamepadButtonEvent::View(
                    BinaryInput {
                        pressed: state.view,
                    },
                )));
            }
            if state.rs != old_state.rs {
                events.push(Event::GamepadButton(GamepadButtonEvent::ThumbR(
                    BinaryInput { pressed: state.rs },
                )));
            }
            if state.ls != old_state.ls {
                events.push(Event::GamepadButton(GamepadButtonEvent::ThumbL(
                    BinaryInput { pressed: state.ls },
                )));
            }
            if state.dpad_state != old_state.dpad_state {
                let new_dpad = state.dpad_state.as_bitflag();
                let old_dpad = old_state.dpad_state.as_bitflag();
                let disabled_fields = old_dpad & !new_dpad;

                if new_dpad & DPadDirection::Up.as_bitflag() != 0 {
                    events.push(Event::GamepadButton(GamepadButtonEvent::DPadUp(
                        BinaryInput { pressed: true },
                    )));
                }
                if new_dpad & DPadDirection::Right.as_bitflag() != 0 {
                    events.push(Event::GamepadButton(GamepadButtonEvent::DPadRight(
                        BinaryInput { pressed: true },
                    )))
                }
                if new_dpad & DPadDirection::Left.as_bitflag() != 0 {
                    events.push(Event::GamepadButton(GamepadButtonEvent::DPadLeft(
                        BinaryInput { pressed: true },
                    )))
                }
                if new_dpad & DPadDirection::Down.as_bitflag() != 0 {
                    events.push(Event::GamepadButton(GamepadButtonEvent::DPadDown(
                        BinaryInput { pressed: true },
                    )))
                }

                if disabled_fields & DPadDirection::Up.as_bitflag() != 0 {
                    events.push(Event::GamepadButton(GamepadButtonEvent::DPadUp(
                        BinaryInput { pressed: false },
                    )));
                }
                if disabled_fields & DPadDirection::Right.as_bitflag() != 0 {
                    events.push(Event::GamepadButton(GamepadButtonEvent::DPadRight(
                        BinaryInput { pressed: false },
                    )))
                }
                if disabled_fields & DPadDirection::Left.as_bitflag() != 0 {
                    events.push(Event::GamepadButton(GamepadButtonEvent::DPadLeft(
                        BinaryInput { pressed: false },
                    )))
                }
                if disabled_fields & DPadDirection::Down.as_bitflag() != 0 {
                    events.push(Event::GamepadButton(GamepadButtonEvent::DPadDown(
                        BinaryInput { pressed: false },
                    )))
                }
            }

            // Axis events
            if state.l_stick_x_sm != old_state.l_stick_x_sm
                || state.l_stick_y_sm != old_state.l_stick_y_sm
                || state.l_stick_x_lg != old_state.l_stick_x_lg
                || state.l_stick_y_lg != old_state.l_stick_y_lg
            {
                events.push(Event::Axis(AxisEvent::LStick(JoyAxisInput {
                    x: self.normalize_x_axis(
                        state.l_stick_x_sm.to_primitive() as u16,
                        state.l_stick_x_lg as u16,
                    ),
                    y: self.normalize_y_axis(
                        state.l_stick_y_sm.to_primitive() as u16,
                        state.l_stick_y_lg as u16,
                    ),
                })));
            }
            if state.r_stick_x_sm != old_state.r_stick_x_sm
                || state.r_stick_y_sm != old_state.r_stick_y_sm
                || state.r_stick_x_lg != old_state.r_stick_x_lg
                || state.r_stick_y_lg != old_state.r_stick_y_lg
            {
                events.push(Event::Axis(AxisEvent::RStick(JoyAxisInput {
                    x: self.normalize_x_axis(
                        state.r_stick_x_sm.to_primitive() as u16,
                        state.r_stick_x_lg as u16,
                    ),
                    y: self.normalize_y_axis(
                        state.r_stick_y_sm.to_primitive() as u16,
                        state.r_stick_y_lg as u16,
                    ),
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
    fn translate_touch(&mut self, old_state: Option<TouchpadDataReport>) -> Vec<Event> {
        let mut events = Vec::new();
        let Some(state) = self.touchpad_state else {
            return events;
        };

        // Translate state changes into events if they have changed
        let Some(_) = old_state else {
            return events;
        };

        // There is a "hold to tap" event built into the firmware, ignore this event.
        if state.touch_x_0 == 314
            && state.touch_y_0 == 512
            && state.touch_x_1 == 682
            && state.touch_y_1 == 512
        {
            log::debug!("Detected 'hold to press' event. Ignoring");
            return events;
        }

        // Axis events
        if !self.is_touching {
            self.is_touching = true;
            self.first_touch = Instant::now();
            log::trace!("Started TOUCH event");
        }
        events.push(Event::Axis(AxisEvent::Touchpad(TouchAxisInput {
            index: 0,
            is_touching: true,
            x: state.touch_x_0,
            y: state.touch_y_0,
        })));

        self.last_touch = Instant::now();
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
        //log::debug!("--- Input report ---");
        //log::debug!("{input_report}");
        //log::debug!(" ---- End Report ----");

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
    fn translate_xinput(&mut self, old_state: Option<XInputDataReport>) -> Vec<Event> {
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

            // Watch for FPS mode, we want to ignore most events in this mode.
            // TODO: Add keyboard events for WASD stuff
            if state.gamepad_mode == 2 {
                //log::debug!("In FPS Mode, rejecting gamepad input.");
                if state.legion != old_state.legion {
                    events.push(Event::GamepadButton(GamepadButtonEvent::Legion(
                        BinaryInput {
                            pressed: state.legion,
                        },
                    )));
                }
                if state.quick_access != old_state.quick_access {
                    events.push(Event::GamepadButton(GamepadButtonEvent::QuickAccess(
                        BinaryInput {
                            pressed: state.quick_access,
                        },
                    )));
                }

                return events;
            }

            // Binary Events
            //if state.a != old_state.a {
            //    events.push(Event::GamepadButton(GamepadButtonEvent::A(BinaryInput {
            //        pressed: state.a,
            //    })));
            //}
            //if state.b != old_state.b {
            //    events.push(Event::GamepadButton(GamepadButtonEvent::B(BinaryInput {
            //        pressed: state.b,
            //    })));
            //}
            //if state.x != old_state.x {
            //    events.push(Event::GamepadButton(GamepadButtonEvent::X(BinaryInput {
            //        pressed: state.x,
            //    })));
            //}
            //if state.y != old_state.y {
            //    events.push(Event::GamepadButton(GamepadButtonEvent::Y(BinaryInput {
            //        pressed: state.y,
            //    })));
            //}
            //if state.menu != old_state.menu {
            //    events.push(Event::GamepadButton(GamepadButtonEvent::Menu(
            //        BinaryInput {
            //            pressed: state.menu,
            //        },
            //    )));
            //}
            //if state.view != old_state.view {
            //    events.push(Event::GamepadButton(GamepadButtonEvent::View(
            //        BinaryInput {
            //            pressed: state.view,
            //        },
            //    )));
            //}
            if state.legion != old_state.legion {
                events.push(Event::GamepadButton(GamepadButtonEvent::Legion(
                    BinaryInput {
                        pressed: state.legion,
                    },
                )));
            }
            if state.quick_access != old_state.quick_access {
                events.push(Event::GamepadButton(GamepadButtonEvent::QuickAccess(
                    BinaryInput {
                        pressed: state.quick_access,
                    },
                )));
            }
            //if state.down != old_state.down {
            //    events.push(Event::GamepadButton(GamepadButtonEvent::DPadDown(
            //        BinaryInput {
            //            pressed: state.down,
            //        },
            //    )));
            //}
            //if state.up != old_state.up {
            //    events.push(Event::GamepadButton(GamepadButtonEvent::DPadUp(
            //        BinaryInput { pressed: state.up },
            //    )));
            //}
            //if state.left != old_state.left {
            //    events.push(Event::GamepadButton(GamepadButtonEvent::DPadLeft(
            //        BinaryInput {
            //            pressed: state.left,
            //        },
            //    )));
            //}
            //if state.right != old_state.right {
            //    events.push(Event::GamepadButton(GamepadButtonEvent::DPadRight(
            //        BinaryInput {
            //            pressed: state.right,
            //        },
            //    )));
            //}
            //if state.lb != old_state.lb {
            //    events.push(Event::GamepadButton(GamepadButtonEvent::LB(BinaryInput {
            //        pressed: state.lb,
            //    })));
            //}
            //if state.rb != old_state.rb {
            //    events.push(Event::GamepadButton(GamepadButtonEvent::RB(BinaryInput {
            //        pressed: state.rb,
            //    })));
            //}
            //if state.d_trigger_l != old_state.d_trigger_l {
            //    events.push(Event::GamepadButton(GamepadButtonEvent::DTriggerL(
            //        BinaryInput {
            //            pressed: state.d_trigger_l,
            //        },
            //    )));
            //}
            //if state.d_trigger_r != old_state.d_trigger_r {
            //    events.push(Event::GamepadButton(GamepadButtonEvent::DTriggerR(
            //        BinaryInput {
            //            pressed: state.d_trigger_r,
            //        },
            //    )));
            //}
            if state.m2 != old_state.m2 {
                events.push(Event::GamepadButton(GamepadButtonEvent::M2(BinaryInput {
                    pressed: state.m2,
                })));
            }
            if state.m3 != old_state.m3 {
                events.push(Event::GamepadButton(GamepadButtonEvent::M3(BinaryInput {
                    pressed: state.m3,
                })));
            }
            if state.y1 != old_state.y1 {
                events.push(Event::GamepadButton(GamepadButtonEvent::Y1(BinaryInput {
                    pressed: state.y1,
                })));
            }
            if state.y2 != old_state.y2 {
                events.push(Event::GamepadButton(GamepadButtonEvent::Y2(BinaryInput {
                    pressed: state.y2,
                })));
            }
            if state.y3 != old_state.y3 {
                events.push(Event::GamepadButton(GamepadButtonEvent::Y3(BinaryInput {
                    pressed: state.y3,
                })));
            }
            if state.mouse_click != old_state.mouse_click {
                events.push(Event::GamepadButton(GamepadButtonEvent::MouseClick(
                    BinaryInput {
                        pressed: state.mouse_click,
                    },
                )));
            }
            //if state.thumb_l != old_state.thumb_l {
            //    events.push(Event::GamepadButton(GamepadButtonEvent::ThumbL(
            //        BinaryInput {
            //            pressed: state.thumb_l,
            //        },
            //    )));
            //}
            //if state.thumb_r != old_state.thumb_r {
            //    events.push(Event::GamepadButton(GamepadButtonEvent::ThumbR(
            //        BinaryInput {
            //            pressed: state.thumb_r,
            //        },
            //    )));
            //}

            // Axis events
            //if state.l_stick_x != old_state.l_stick_x || state.l_stick_y != old_state.l_stick_y {
            //    events.push(Event::Axis(AxisEvent::LStick(JoyAxisInput {
            //        x: state.l_stick_x,
            //        y: state.l_stick_y,
            //    })));
            //}
            //if state.r_stick_x != old_state.r_stick_x || state.r_stick_y != old_state.r_stick_y {
            //    events.push(Event::Axis(AxisEvent::RStick(JoyAxisInput {
            //        x: state.r_stick_x,
            //        y: state.r_stick_y,
            //    })));
            //}

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

    fn release_touch(&mut self) -> Event {
        log::trace!("Released TOUCH event.");
        self.is_touching = false;
        Event::Axis(AxisEvent::Touchpad(TouchAxisInput {
            index: 0,
            is_touching: false,
            x: 0,
            y: 0,
        }))
    }

    fn start_tap(&mut self) -> Event {
        log::trace!("Started CLICK event.");
        self.is_tapped = true;
        Event::TouchButton(TouchButtonEvent::Left(BinaryInput { pressed: true }))
    }

    fn release_tap(&mut self) -> Event {
        log::trace!("Released CLICK event.");
        self.is_tapped = false;
        Event::TouchButton(TouchButtonEvent::Left(BinaryInput { pressed: false }))
    }
}
