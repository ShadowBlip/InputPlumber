use std::{error::Error, ffi::CString};

use hidapi::HidDevice;
use packed_struct::PackedStruct;
use tokio::time::Instant;

use crate::drivers::lego::{
    event::{ImuAxisInput, TouchAxisInput, TouchButtonEvent},
    hid_report::{ConnectedState, GamepadMode},
    CLICK_DELAY, PAD_FORCE_NORMAL, RELEASE_DELAY, XINPUT_COMMAND_ID,
};

use super::{
    event::{
        AxisEvent, BinaryInput, Event, GamepadButtonEvent, JoyAxisInput, MouseWheelInput,
        TriggerEvent, TriggerInput,
    },
    hid_report::XInputDataReport,
    GP_IID, HID_TIMEOUT, PIDS, VID, XINPUT_DATA, XINPUT_PACKET_SIZE,
};

pub struct Driver {
    /// HIDRAW device instance
    device: HidDevice,
    /// Timestamp of the first touch event.
    first_touch: Instant,
    /// Whether or not we are currently holding a click-to-click.
    is_clicked: bool,
    /// Whether or not we are detecting a touch event currently.
    is_touching: bool,
    /// Timestamp of the last touch event.
    last_touch: Instant,
    /// Whether or not a touch event was started that hasn't been cleared.
    touch_started: bool,
    /// State for the internal gamepad  controller
    xinput_state: Option<XInputDataReport>,
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
            first_touch: Instant::now(),
            is_clicked: false,
            is_touching: false,
            last_touch: Instant::now(),
            touch_started: false,
            xinput_state: None,
        })
    }

    /// Poll the device and read input reports
    pub fn poll(&mut self) -> Result<Vec<Event>, Box<dyn Error + Send + Sync>> {
        // Read data from the device into a buffer
        let mut buf = [0; XINPUT_PACKET_SIZE];
        let bytes_read = self.device.read_timeout(&mut buf[..], HID_TIMEOUT)?;

        let report_id = buf[0];
        let command_id = buf[2];

        // Configuration event responses happen on the same endpoint. If this data packet isn't
        // specifically xinput data it can crash the driver, so block it.
        if command_id != XINPUT_COMMAND_ID {
            //log::trace!("Got event that isn't xinput data, skipping");
            return Ok(vec![]);
        }
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
            if state.m1 != old_state.m1 {
                events.push(Event::GamepadButton(GamepadButtonEvent::M1(BinaryInput {
                    pressed: state.m1,
                })));
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

            // IMU Events
            if state.l_con_state != old_state.l_con_state
                || state.r_con_state != old_state.r_con_state
            {
                log::trace!("Left controller connected state: {:?}", state.l_con_state);
                log::trace!("Right controller connected state: {:?}", state.r_con_state);
            }
            if state.left_accel_x != old_state.left_accel_x
                || state.left_accel_y != old_state.left_accel_y
                || state.left_accel_z != old_state.left_accel_z
            {
                let event = match state.l_con_state {
                    ConnectedState::Connecting => Event::None,
                    ConnectedState::Attached => Event::Axis(AxisEvent::LeftAccel(ImuAxisInput {
                        x: state.left_accel_x,
                        y: state.left_accel_y,
                        z: state.left_accel_z,
                    })),
                    ConnectedState::Detached => Event::Axis(AxisEvent::LeftAccel(ImuAxisInput {
                        x: state.left_accel_x * 2,
                        y: state.left_accel_y,
                        z: state.left_accel_z,
                    })),
                };
                events.push(event);
            }
            if state.right_accel_x != old_state.right_accel_x
                || state.right_accel_y != old_state.right_accel_y
                || state.right_accel_z != old_state.right_accel_z
            {
                let event = match state.r_con_state {
                    ConnectedState::Connecting => Event::None,
                    ConnectedState::Attached => Event::Axis(AxisEvent::RightAccel(ImuAxisInput {
                        x: -state.right_accel_x,
                        y: -state.right_accel_y,
                        z: state.right_accel_z,
                    })),
                    ConnectedState::Detached => Event::Axis(AxisEvent::RightAccel(ImuAxisInput {
                        x: -state.right_accel_x * 2,
                        y: -state.right_accel_y,
                        z: state.right_accel_z,
                    })),
                };
                events.push(event);
            }
            if state.left_gyro_x != old_state.left_gyro_x
                || state.left_gyro_y != old_state.left_gyro_y
                || state.left_gyro_z != old_state.left_gyro_z
            {
                let event = match state.l_con_state {
                    ConnectedState::Connecting => Event::None,
                    ConnectedState::Attached => Event::Axis(AxisEvent::LeftGyro(ImuAxisInput {
                        x: state.left_gyro_x,
                        y: state.left_gyro_y,
                        z: state.left_gyro_z,
                    })),
                    ConnectedState::Detached => Event::Axis(AxisEvent::LeftGyro(ImuAxisInput {
                        x: state.left_gyro_x * 2,
                        y: state.left_gyro_y,
                        z: state.left_gyro_z,
                    })),
                };
                events.push(event);
            }
            if state.right_gyro_x != old_state.right_gyro_x
                || state.right_gyro_y != old_state.right_gyro_y
                || state.right_gyro_z != old_state.right_gyro_z
            {
                let event = match state.r_con_state {
                    ConnectedState::Connecting => Event::None,
                    ConnectedState::Attached => Event::Axis(AxisEvent::RightGyro(ImuAxisInput {
                        x: state.right_gyro_x,
                        y: state.right_gyro_y,
                        z: state.right_gyro_z,
                    })),
                    ConnectedState::Detached => Event::Axis(AxisEvent::RightGyro(ImuAxisInput {
                        x: state.right_gyro_x * 2,
                        y: state.right_gyro_y,
                        z: state.right_gyro_z,
                    })),
                };
                events.push(event);
            }

            // Touchpad events

            // Detect if we are touching or not, x, y will always be 0, 0 when the pad is not
            // touched.
            self.is_touching = state.touch_x != 0 && state.touch_y != 0;

            // Handle touching
            if self.is_touching {
                self.last_touch = Instant::now();

                // If this is the first event of a new touch, log the time.
                if !self.touch_started {
                    log::debug!("START Touch");
                    log::debug!("Last touch elapsed: {:?}", self.last_touch.elapsed());

                    self.touch_started = true;
                    self.first_touch = Instant::now();
                }
            // Handle tap to click
            } else if !self.is_touching
                && self.touch_started
                && self.first_touch.elapsed() < CLICK_DELAY
            {
                // Handle double click
                if self.is_clicked && self.first_touch.elapsed() < RELEASE_DELAY {
                    log::debug!("Double Click");
                    let mut new_events = self.release_click();
                    events.append(&mut new_events);
                }
                let mut click_events = self.start_click();
                events.append(&mut click_events);
            // Handle release events
            } else if !self.is_touching && self.last_touch.elapsed() > RELEASE_DELAY {
                // Unclick if we we clicking and are no longer touching.
                if self.is_clicked {
                    let mut new_events = self.release_click();
                    events.append(&mut new_events);
                }

                // Clear this touch sequence
                if self.touch_started {
                    self.touch_started = false;
                    log::debug!("END Touch");
                }
            }

            if state.touch_x != old_state.touch_x || state.touch_y != old_state.touch_y {
                events.push(Event::Axis(AxisEvent::Touchpad(TouchAxisInput {
                    index: 0,
                    is_touching: self.is_touching,
                    x: state.touch_x,
                    y: state.touch_y,
                })));
            }
        }
        events
    }

    fn start_click(&mut self) -> Vec<Event> {
        if self.is_clicked {
            log::debug!("Regecting extra click");
            return vec![];
        }
        log::debug!("Started CLICK event.");
        log::debug!("First touch elapsed: {:?}", self.first_touch.elapsed());
        log::debug!("Last touch elapsed: {:?}", self.last_touch.elapsed());
        self.is_clicked = true;
        let mut events = Vec::new();

        let event = Event::TouchButton(TouchButtonEvent::Left(BinaryInput { pressed: true }));
        events.push(event);
        // The touchpad doesn't have a force sensor. The deck target wont produce a "click"
        // event in desktop or lizard mode without a force value. Simulate a 1/4 press to work
        // around this.
        let event = Event::Trigger(TriggerEvent::RpadForce(TriggerInput {
            value: PAD_FORCE_NORMAL,
        }));
        events.push(event);
        events
    }

    fn release_click(&mut self) -> Vec<Event> {
        log::debug!("Released CLICK event.");
        log::debug!("First touch elapsed: {:?}", self.first_touch.elapsed());
        log::debug!("Last touch elapsed: {:?}", self.last_touch.elapsed());
        self.is_clicked = false;
        self.touch_started = false;
        let mut events = Vec::new();
        let event = Event::TouchButton(TouchButtonEvent::Left(BinaryInput { pressed: false }));
        events.push(event);
        // The touchpad doesn't have a force sensor. The deck target wont produce a "click"
        // event in desktop or lizard mode without a force value. Simulate a 1/4 press to work
        // around this.
        let event = Event::Trigger(TriggerEvent::RpadForce(TriggerInput { value: 0 }));
        events.push(event);
        events
    }
}
