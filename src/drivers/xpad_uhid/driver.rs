use std::{error::Error, ffi::CString};

use hidapi::HidDevice;
use packed_struct::PackedStruct;

use crate::{
    drivers::xpad_uhid::hid_report::{DPadDirection, XBoxSeriesInputDataReport},
    udev::device::UdevDevice,
};

use super::{
    event::{AxisEvent, BinaryInput, ButtonEvent, Event, JoyAxisInput, TriggerEvent, TriggerInput},
    hid_report::{XpadUhidOutputData, XpadUhidOutputReport},
};

// Hardware ID's
pub const VIDS: [u16; 1] = [0x045e];
pub const PIDS: [u16; 1] = [0x0b13];

// Report ID
pub const DATA: u8 = 0x01;

// Input report size
const PACKET_SIZE: usize = 17;

// HID buffer read timeout
const HID_TIMEOUT: i32 = 10;

// Input report axis ranges
pub const JOY_AXIS_MAX: f64 = 65535.0;
pub const JOY_AXIS_MIN: f64 = 0.0;
pub const TRIGGER_AXIS_MAX: f64 = 1023.0;

#[derive(Debug, Clone, Default)]
struct DPadState {
    up: bool,
    down: bool,
    left: bool,
    right: bool,
}

pub struct Driver {
    /// HIDRAW device instance
    device: HidDevice,
    /// State for the device
    state: Option<XBoxSeriesInputDataReport>,
    /// Last DPad state
    dpad: DPadState,
}

impl Driver {
    pub fn new(udevice: UdevDevice) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let path = udevice.devnode();
        let cs_path = CString::new(path.clone())?;
        let api = hidapi::HidApi::new()?;
        let device = api.open_path(&cs_path)?;
        let info = device.get_device_info()?;
        if !VIDS.contains(&info.vendor_id()) || !PIDS.contains(&info.product_id()) {
            return Err(format!("Device '{path}' is not an xpad_uhid controller").into());
        }
        Ok(Self {
            device,
            state: None,
            dpad: Default::default(),
        })
    }

    /// Writes the given output state to the gamepad. This can be used to change
    /// the color of LEDs, activate rumble, etc.
    pub fn write(&self, state: XpadUhidOutputData) -> Result<(), Box<dyn Error + Send + Sync>> {
        let report = XpadUhidOutputReport {
            state,
            ..Default::default()
        };
        let buf = report.pack()?;
        let _bytes_written = self.device.write(&buf)?;

        Ok(())
    }

    /// Rumble the gamepad
    pub fn rumble(
        &self,
        _left_speed: u8,
        _right_speed: u8,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let state = XpadUhidOutputData {};

        self.write(state)
    }

    /// Poll the device and read input reports
    pub fn poll(&mut self) -> Result<Vec<Event>, Box<dyn Error + Send + Sync>> {
        // Read data from the device into a buffer
        let mut buf = [0; PACKET_SIZE];
        let bytes_read = self.device.read_timeout(&mut buf[..], HID_TIMEOUT)?;

        let report_id = buf[0];
        let slice = &buf[..bytes_read];
        //log::debug!("Got Report ID: {report_id}");
        //log::debug!("Got Report Size: {bytes_read}");

        let events = match report_id {
            DATA => {
                log::trace!("Got input data.");
                if bytes_read != PACKET_SIZE {
                    return Err("Invalid packet size for Keyboard or Touchpad Data.".into());
                }
                // Handle the incoming input report
                let sized_buf = slice.try_into()?;

                self.handle_input_report(sized_buf)?
            }
            _ => {
                //log::debug!("Invalid Report ID.");
                let events = vec![];
                events
            }
        };

        Ok(events)
    }

    /// Unpacks the buffer into a [DataReport] structure and updates
    /// the internal state
    fn handle_input_report(
        &mut self,
        buf: [u8; PACKET_SIZE],
    ) -> Result<Vec<Event>, Box<dyn Error + Send + Sync>> {
        let input_report = XBoxSeriesInputDataReport::unpack(&buf)?;

        // Print input report for debugging
        log::trace!("--- Input report ---");
        log::trace!("{input_report}");
        log::trace!("---- End Report ----");

        // Update the state
        let old_dinput_state = self.update_state(input_report);

        // Translate the state into a stream of input events
        let events = self.translate_events(old_dinput_state);

        Ok(events)
    }

    /// Update touchinput state
    fn update_state(
        &mut self,
        input_report: XBoxSeriesInputDataReport,
    ) -> Option<XBoxSeriesInputDataReport> {
        let old_state = self.state;
        self.state = Some(input_report);
        old_state
    }

    /// Translate the state into individual events
    fn translate_events(&mut self, old_state: Option<XBoxSeriesInputDataReport>) -> Vec<Event> {
        let mut events = Vec::new();
        let Some(state) = self.state else {
            return events;
        };

        // Translate state changes into events if they have changed
        if let Some(old_state) = old_state {
            // Binary Events
            if state.button_state.a != old_state.button_state.a {
                events.push(Event::Button(ButtonEvent::A(BinaryInput {
                    pressed: state.button_state.a,
                })));
            }
            if state.button_state.b != old_state.button_state.b {
                events.push(Event::Button(ButtonEvent::B(BinaryInput {
                    pressed: state.button_state.b,
                })));
            }
            if state.button_state.x != old_state.button_state.x {
                events.push(Event::Button(ButtonEvent::X(BinaryInput {
                    pressed: state.button_state.x,
                })));
            }
            if state.button_state.y != old_state.button_state.y {
                events.push(Event::Button(ButtonEvent::Y(BinaryInput {
                    pressed: state.button_state.y,
                })));
            }
            if state.button_state.rb != old_state.button_state.rb {
                events.push(Event::Button(ButtonEvent::RB(BinaryInput {
                    pressed: state.button_state.rb,
                })));
            }
            if state.button_state.lb != old_state.button_state.lb {
                events.push(Event::Button(ButtonEvent::LB(BinaryInput {
                    pressed: state.button_state.lb,
                })));
            }
            if state.button_state.view != old_state.button_state.view {
                events.push(Event::Button(ButtonEvent::View(BinaryInput {
                    pressed: state.button_state.view,
                })));
            }
            if state.button_state.menu != old_state.button_state.menu {
                events.push(Event::Button(ButtonEvent::Menu(BinaryInput {
                    pressed: state.button_state.menu,
                })));
            }
            if state.button_state.guide != old_state.button_state.guide {
                events.push(Event::Button(ButtonEvent::Guide(BinaryInput {
                    pressed: state.button_state.guide,
                })));
            }
            if state.button_state.thumb_l != old_state.button_state.thumb_l {
                events.push(Event::Button(ButtonEvent::ThumbL(BinaryInput {
                    pressed: state.button_state.thumb_l,
                })));
            }
            if state.button_state.thumb_r != old_state.button_state.thumb_r {
                events.push(Event::Button(ButtonEvent::ThumbR(BinaryInput {
                    pressed: state.button_state.thumb_r,
                })));
            }
            if state.dpad_state != old_state.dpad_state {
                let up = [
                    DPadDirection::Up,
                    DPadDirection::UpRight,
                    DPadDirection::UpLeft,
                ]
                .contains(&state.dpad_state);
                let down = [
                    DPadDirection::Down,
                    DPadDirection::DownRight,
                    DPadDirection::DownLeft,
                ]
                .contains(&state.dpad_state);
                let left = [
                    DPadDirection::Left,
                    DPadDirection::DownLeft,
                    DPadDirection::DownRight,
                ]
                .contains(&state.dpad_state);
                let right = [
                    DPadDirection::Right,
                    DPadDirection::DownRight,
                    DPadDirection::UpRight,
                ]
                .contains(&state.dpad_state);
                let dpad_state = DPadState {
                    up,
                    down,
                    left,
                    right,
                };

                if up != self.dpad.up {
                    events.push(Event::Button(ButtonEvent::DPadUp(BinaryInput {
                        pressed: up,
                    })));
                }
                if down != self.dpad.down {
                    events.push(Event::Button(ButtonEvent::DPadDown(BinaryInput {
                        pressed: down,
                    })));
                }
                if left != self.dpad.left {
                    events.push(Event::Button(ButtonEvent::DPadLeft(BinaryInput {
                        pressed: left,
                    })));
                }
                if right != self.dpad.right {
                    events.push(Event::Button(ButtonEvent::DPadRight(BinaryInput {
                        pressed: right,
                    })));
                }

                self.dpad = dpad_state;
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

            if state.trigger_l != old_state.trigger_l {
                events.push(Event::Trigger(TriggerEvent::TriggerL(TriggerInput {
                    value: state.trigger_l,
                })));
            }
            if state.trigger_l != old_state.trigger_r {
                events.push(Event::Trigger(TriggerEvent::TriggerR(TriggerInput {
                    value: state.trigger_r,
                })));
            }
            log::trace!("Got events: {events:?}");

            return events;
        };

        events
    }
}
