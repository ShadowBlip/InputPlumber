use std::{
    error::Error,
    ffi::CString,
    time::{Duration, Instant},
};

use hidapi::HidDevice;
use packed_struct::PackedStruct;

use super::{
    event::{
        AxisEvent, BinaryInput, Event, GamepadButtonEvent, JoyAxisInput,
        TouchAxisInput,
        TouchButtonEvent, TriggerEvent, TriggerInput,
    },
    hid_report::{
        XInputDataReport,
    },
};

// Hardware ID's
pub const VID: u16 = 0x1a86;
pub const PID: u16 = 0xe310;
// Hardware limits
pub const XINPUT_DATA: u8 = 0x00;
// Input report sizes
const XINPUT_PACKET_SIZE: usize = 32;
const HID_TIMEOUT: i32 = 10;
// Input report axis ranges
pub const MOUSE_WHEEL_MAX: f64 = 120.0;
pub const PAD_X_MAX: f64 = 1024.0;
pub const PAD_Y_MAX: f64 = 1024.0;
pub const STICK_X_MAX: f64 = 127.0;
pub const STICK_X_MIN: f64 = -127.0;
pub const STICK_Y_MAX: f64 = 127.0;
pub const STICK_Y_MIN: f64 = -127.0;
pub const TRIGG_MAX: f64 = 255.0;

pub struct Driver {
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
        if info.vendor_id() != VID || info.product_id() != PID
        {
            return Err(format!("Device '{fmtpath}' is not a Legion Go S Controller").into());
        }

        Ok(Self {
            device,
            first_touch: Instant::now(),
            is_tapped: false,
            is_touching: false,
            last_touch: Instant::now(),
            xinput_state: None,
        })
    }

    /// Poll the device and read input reports
    pub fn poll(&mut self) -> Result<Vec<Event>, Box<dyn Error + Send + Sync>> {
        // Read data from the device into a buffer
        let mut buf = [0; XINPUT_PACKET_SIZE];
        let bytes_read = self.device.read_timeout(&mut buf[..], HID_TIMEOUT)?;

        let mut events;

        if bytes_read != XINPUT_PACKET_SIZE {
            // controller device goes to sleep and reads time out after a while
            // todo: check actual timeout?, but either way shouldn't be fatal
            events = vec![];
        } else {
            events = self.handle_xinput_report(buf)?;
        }

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
            if state.down != old_state.down {
                events.push(Event::GamepadButton(GamepadButtonEvent::DPadDown(
                    BinaryInput {
                        pressed: state.down,
                    },
                )));
            }
            if state.up != old_state.up {
                events.push(Event::GamepadButton(GamepadButtonEvent::DPadUp(
                    BinaryInput { pressed: state.up },
                )));
            }
            if state.left != old_state.left {
                events.push(Event::GamepadButton(GamepadButtonEvent::DPadLeft(
                    BinaryInput {
                        pressed: state.left,
                    },
                )));
            }
            if state.right != old_state.right {
                events.push(Event::GamepadButton(GamepadButtonEvent::DPadRight(
                    BinaryInput {
                        pressed: state.right,
                    },
                )));
            }
            if state.lb != old_state.lb {
                events.push(Event::GamepadButton(GamepadButtonEvent::LB(BinaryInput {
                    pressed: state.lb,
                })));
            }
            if state.rb != old_state.rb {
                events.push(Event::GamepadButton(GamepadButtonEvent::RB(BinaryInput {
                    pressed: state.rb,
                })));
            }
            if state.d_trigger_l != old_state.d_trigger_l {
                events.push(Event::GamepadButton(GamepadButtonEvent::DTriggerL(
                    BinaryInput {
                        pressed: state.d_trigger_l,
                    },
                )));
            }
            if state.d_trigger_r != old_state.d_trigger_r {
                events.push(Event::GamepadButton(GamepadButtonEvent::DTriggerR(
                    BinaryInput {
                        pressed: state.d_trigger_r,
                    },
                )));
            }
            // if state.m2 != old_state.m2 {
            //     events.push(Event::GamepadButton(GamepadButtonEvent::M2(BinaryInput {
            //         pressed: state.m2,
            //     })));
            // }
            // if state.m3 != old_state.m3 {
            //     events.push(Event::GamepadButton(GamepadButtonEvent::M3(BinaryInput {
            //         pressed: state.m3,
            //     })));
            // }
            if state.y1 != old_state.y1 {
                events.push(Event::GamepadButton(GamepadButtonEvent::Y1(BinaryInput {
                    pressed: state.y1,
                })));
            }
            // if state.y2 != old_state.y2 {
            //     events.push(Event::GamepadButton(GamepadButtonEvent::Y2(BinaryInput {
            //         pressed: state.y2,
            //     })));
            // }
            if state.y3 != old_state.y3 {
                events.push(Event::GamepadButton(GamepadButtonEvent::Y3(BinaryInput {
                    pressed: state.y3,
                })));
            }
            // if state.mouse_click != old_state.mouse_click {
            //     events.push(Event::GamepadButton(GamepadButtonEvent::MouseClick(
            //         BinaryInput {
            //             pressed: state.mouse_click,
            //         },
            //     )));
            // }
            if state.thumb_l != old_state.thumb_l {
                events.push(Event::GamepadButton(GamepadButtonEvent::ThumbL(
                    BinaryInput {
                        pressed: state.thumb_l,
                    },
                )));
            }
            if state.thumb_r != old_state.thumb_r {
                events.push(Event::GamepadButton(GamepadButtonEvent::ThumbR(
                    BinaryInput {
                        pressed: state.thumb_r,
                    },
                )));
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
            // if state.mouse_z != old_state.mouse_z {
            //     events.push(Event::Trigger(TriggerEvent::MouseWheel(MouseWheelInput {
            //         value: state.mouse_z,
            //     })));
            // }

            // // Status events
            // if state.l_controller_battery != old_state.l_controller_battery {
            //     events.push(Event::Status(StatusEvent::LeftControllerBattery(
            //         StatusInput {
            //             value: state.l_controller_battery,
            //         },
            //     )));
            // }
            // if state.l_controller_mode0 != old_state.l_controller_mode0 {
            //     events.push(Event::Status(StatusEvent::LeftControllerMode0(
            //         StatusInput {
            //             value: state.l_controller_mode0,
            //         },
            //     )));
            // }
            // if state.l_controller_mode1 != old_state.l_controller_mode1 {
            //     events.push(Event::Status(StatusEvent::LeftControllerMode1(
            //         StatusInput {
            //             value: state.l_controller_mode1,
            //         },
            //     )));
            // }
            // if state.r_controller_battery != old_state.r_controller_battery {
            //     events.push(Event::Status(StatusEvent::RightControllerBattery(
            //         StatusInput {
            //             value: state.r_controller_battery,
            //         },
            //     )));
            // }
            // if state.r_controller_mode0 != old_state.r_controller_mode0 {
            //     events.push(Event::Status(StatusEvent::RightControllerMode0(
            //         StatusInput {
            //             value: state.r_controller_mode0,
            //         },
            //     )));
            // }
            // if state.r_controller_mode1 != old_state.r_controller_mode1 {
            //     events.push(Event::Status(StatusEvent::RightControllerMode1(
            //         StatusInput {
            //             value: state.r_controller_mode1,
            //         },
            //     )));
            // }
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
