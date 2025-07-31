use std::{error::Error, ffi::CString};

use hidapi::HidDevice;
use packed_struct::PackedStruct;

use crate::drivers::lego::hid_report::GamepadMode;

use super::{
    event::{
        AxisEvent, BinaryInput, Event, GamepadButtonEvent, JoyAxisInput, MouseWheelInput,
        StatusEvent, StatusInput, TriggerEvent, TriggerInput,
    },
    hid_report::XInputDataReport,
    GP_IID, HID_TIMEOUT, PIDS, VID, XINPUT_DATA, XINPUT_PACKET_SIZE,
};

pub struct Driver {
    /// State for the internal gamepad  controller
    xinput_state: Option<XInputDataReport>,
    /// HIDRAW device instance
    device: HidDevice,
}

impl Driver {
    pub fn new(path: String) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let fmtpath = path.clone();
        let path = CString::new(path)?;
        let api = hidapi::HidApi::new()?;
        let device = api.open_path(&path)?;
        let info = device.get_device_info()?;

        if info.vendor_id() != VID
            || !PIDS.contains(&info.product_id())
            || info.interface_number() != GP_IID
        {
            return Err(format!("Device '{fmtpath}' is not a Legion Go S Controller").into());
        }

        Ok(Self {
            device,
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

        let events = match report_id {
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
                    "Gamepad mode changed from {} to {}",
                    old_state.gamepad_mode,
                    state.gamepad_mode
                );
            }

            if state.gamepad_mode == GamepadMode::Fps {
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

                if state.show_desktop != old_state.show_desktop {
                    events.push(Event::GamepadButton(GamepadButtonEvent::ShowDesktop(
                        BinaryInput {
                            pressed: state.show_desktop,
                        },
                    )));
                }
                if state.alt_tab != old_state.alt_tab {
                    events.push(Event::GamepadButton(GamepadButtonEvent::AltTab(
                        BinaryInput {
                            pressed: state.alt_tab,
                        },
                    )));
                }
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
            if state.show_desktop != old_state.show_desktop {
                events.push(Event::GamepadButton(GamepadButtonEvent::ShowDesktop(
                    BinaryInput {
                        pressed: state.show_desktop,
                    },
                )));
            }
            if state.alt_tab != old_state.alt_tab {
                events.push(Event::GamepadButton(GamepadButtonEvent::AltTab(
                    BinaryInput {
                        pressed: state.alt_tab,
                    },
                )));
            }
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
}
