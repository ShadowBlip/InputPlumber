use std::{error::Error, ffi::CString};

use hidapi::HidDevice;
use packed_struct::PackedStruct;

use super::{
    event::{AxisEvent, BinaryInput, ButtonEvent, Event, JoyAxisInput, TriggerEvent, TriggerInput},
    hid_report::{InputDataReport, RumbleOutputDataReport},
    HID_TIMEOUT, INPUT_PACKET_SIZE, PID, VID,
};

pub struct Driver {
    /// HIDRAW device instance
    device: HidDevice,
    /// State for the internal gamepad  controller
    input_state: Option<InputDataReport>,
}

impl Driver {
    pub fn new(path: String) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let fmtpath = path.clone();
        let path = CString::new(path)?;
        let api = hidapi::HidApi::new()?;
        let device = api.open_path(&path)?;
        let info = device.get_device_info()?;

        if info.vendor_id() != VID || info.product_id() != PID {
            return Err(format!("Device '{fmtpath}' is not an MSI Claw Controller").into());
        }
        Ok(Self {
            device,
            input_state: None,
        })
    }

    /// Poll the device and read input reports
    pub fn poll(&mut self) -> Result<Vec<Event>, Box<dyn Error + Send + Sync>> {
        // Read data from the device into a buffer
        let mut buf = [0; INPUT_PACKET_SIZE];
        let bytes_read = self.device.read_timeout(&mut buf[..], HID_TIMEOUT)?;

        if bytes_read != INPUT_PACKET_SIZE {
            return Ok(vec![]);
        }

        match self.handle_input_report(buf) {
            Ok(events) => Ok(events),
            Err(_e) => Ok(vec![]),
        }
    }

    /// Writes the given output state to the gamepad. This can be used to change
    /// the color of LEDs, activate rumble, etc.
    pub fn write(&self, buf: &[u8]) -> Result<(), Box<dyn Error + Send + Sync>> {
        let bytes_written = self.device.write(buf)?;
        log::debug!("Wrote {bytes_written} bytes");
        Ok(())
    }

    pub fn haptic_rumble(
        &self,
        strong_magnitude: u8,
        weak_magnitude: u8,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let report = RumbleOutputDataReport {
            strong_magnitude,
            weak_magnitude,
            ..Default::default()
        };
        log::debug!("Got rumble event: {report:?}");

        let buf = report.pack()?;
        self.write(&buf)
    }

    /* GamePad */
    /// Unpacks the buffer into a [InputDataReport] structure and updates
    /// the internal input_state
    fn handle_input_report(
        &mut self,
        buf: [u8; INPUT_PACKET_SIZE],
    ) -> Result<Vec<Event>, Box<dyn Error + Send + Sync>> {
        let input_report = InputDataReport::unpack(&buf)?;

        // Print input report for debugging
        //log::debug!("--- Input report ---");
        //log::debug!("{input_report}");
        //log::debug!(" ---- End Report ----");

        // Update the state
        let old_input_state = self.update_input_state(input_report);

        // Translate the state into a stream of input events
        let events = self.translate_input(old_input_state);

        Ok(events)
    }

    /// Update gamepad state
    fn update_input_state(&mut self, input_report: InputDataReport) -> Option<InputDataReport> {
        let old_state = self.input_state;
        self.input_state = Some(input_report);
        old_state
    }

    /// Translate the state into individual events
    fn translate_input(&self, old_state: Option<InputDataReport>) -> Vec<Event> {
        let mut events = Vec::new();
        let Some(state) = self.input_state else {
            return events;
        };

        // Translate state changes into events if they have changed
        let Some(old_state) = old_state else {
            return events;
        };

        // Binary Events
        if state.button_a != old_state.button_a {
            events.push(Event::Button(ButtonEvent::A(BinaryInput {
                pressed: state.button_a,
            })));
        }
        if state.button_b != old_state.button_b {
            events.push(Event::Button(ButtonEvent::B(BinaryInput {
                pressed: state.button_b,
            })));
        }
        if state.button_x != old_state.button_x {
            events.push(Event::Button(ButtonEvent::X(BinaryInput {
                pressed: state.button_x,
            })));
        }
        if state.button_y != old_state.button_y {
            events.push(Event::Button(ButtonEvent::Y(BinaryInput {
                pressed: state.button_y,
            })));
        }
        if state.button_menu != old_state.button_menu {
            events.push(Event::Button(ButtonEvent::Menu(BinaryInput {
                pressed: state.button_menu,
            })));
        }
        if state.button_view != old_state.button_view {
            events.push(Event::Button(ButtonEvent::View(BinaryInput {
                pressed: state.button_view,
            })));
        }
        if state.button_lb != old_state.button_lb {
            events.push(Event::Button(ButtonEvent::LB(BinaryInput {
                pressed: state.button_lb,
            })));
        }
        if state.button_rb != old_state.button_rb {
            events.push(Event::Button(ButtonEvent::RB(BinaryInput {
                pressed: state.button_rb,
            })));
        }
        if state.button_l3 != old_state.button_l3 {
            events.push(Event::Button(ButtonEvent::ThumbL(BinaryInput {
                pressed: state.button_l3,
            })));
        }
        if state.button_r3 != old_state.button_r3 {
            events.push(Event::Button(ButtonEvent::ThumbR(BinaryInput {
                pressed: state.button_r3,
            })));
        }

        // Dpad Events
        if state.dpad_dir != old_state.dpad_dir {
            let (new_up, new_right, new_down, new_left) = state.dpad_dir.button_states();
            let (old_up, old_right, old_down, old_left) = old_state.dpad_dir.button_states();

            if new_up != old_up {
                events.push(Event::Button(ButtonEvent::DPadUp(BinaryInput {
                    pressed: new_up,
                })));
            }
            if new_right != old_right {
                events.push(Event::Button(ButtonEvent::DPadRight(BinaryInput {
                    pressed: new_right,
                })));
            }
            if new_down != old_down {
                events.push(Event::Button(ButtonEvent::DPadDown(BinaryInput {
                    pressed: new_down,
                })));
            }
            if new_left != old_left {
                events.push(Event::Button(ButtonEvent::DPadLeft(BinaryInput {
                    pressed: new_left,
                })));
            }
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
            events.push(Event::Trigger(TriggerEvent::ATriggerL(TriggerInput {
                value: state.trigger_l,
            })));
        }
        if state.trigger_r != old_state.trigger_r {
            events.push(Event::Trigger(TriggerEvent::ATriggerR(TriggerInput {
                value: state.trigger_r,
            })));
        }

        events
    }
}
