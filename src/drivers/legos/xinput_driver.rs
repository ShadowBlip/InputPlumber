use std::{error::Error, ffi::CString};

use hidapi::HidDevice;
use packed_struct::PackedStruct;

use super::{
    event::{AxisEvent, BinaryInput, ButtonEvent, Event, JoyAxisInput, TriggerEvent, TriggerInput},
    hid_report::{RumbleOutputDataReport, XInputDataReport},
    GP_IID, HID_TIMEOUT, PIDS, VID, XINPUT_PACKET_SIZE,
};

pub struct XInputDriver {
    /// HIDRAW device instance
    device: HidDevice,
    /// State for the internal gamepad  controller
    xinput_state: Option<XInputDataReport>,
}

impl XInputDriver {
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

        if bytes_read != XINPUT_PACKET_SIZE {
            return Ok(vec![]);
        }

        match self.handle_xinput_report(buf) {
            Ok(events) => Ok(events),
            Err(_e) => Ok(vec![]),
        }
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
        let report = RumbleOutputDataReport {
            l_motor_speed,
            r_motor_speed,
            ..Default::default()
        };
        //log::debug!("Got rumble event: {report:?}");

        let buf = report.pack()?;
        self.write(&buf)
    }

    /* GamePad */
    /// Unpacks the buffer into a [XInputDataReport] structure and updates
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

        events
    }
}
