use std::{error::Error, ffi::CString};

/* use std::{error::Error, ffi::CString, time::SystemTime, time::UNIX_EPOCH}; // adding timestamp logging. delete after. */


use hidapi::{HidApi, HidDevice};
use packed_struct::{types::SizedInteger, PackedStruct};

use crate::{drivers::flydigi_vader_4_pro::hid_report::Direction, udev::device::UdevDevice};

use super::{
    event::{
        BinaryInput, ButtonEvent, Event, JoystickEvent,
        JoystickInput, TriggerEvent, TriggerInput,
    },
    hid_report::PackedInputDataReport,
};

/* use super::{
    event::{
        BinaryInput, ButtonEvent, Event, InertialEvent, InertialInput, JoystickEvent,
        JoystickInput, TriggerEvent, TriggerInput,
    },
    hid_report::PackedInputDataReport,
};
 */
// Report ID
pub const REPORT_ID: u8 = 0x04;

// Input report size
const PACKET_SIZE: usize = 31;


// HID buffer read timeout
const HID_TIMEOUT: i32 = 10;

// Input report axis ranges
pub const JOY_AXIS_MAX: f64 = 255.0;
pub const JOY_AXIS_MIN: f64 = 0.0;
pub const TRIGGER_AXIS_MAX: f64 = 255.0;

pub const VID: u16 = 0x04b4;
pub const PID: u16 = 0x2412;

#[derive(Debug, Clone, Default)]
struct DPadState {
    up: bool,
    down: bool,
    left: bool,
    right: bool,
}

pub struct Driver {
    /// Path to the HIDRAW device
    device: HidDevice,
    /// State for the device
    state: Option<PackedInputDataReport>,
    /// Last DPad state
    dpad: DPadState,
}

impl Driver {
    pub fn new(udevice: UdevDevice) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let hidrawpath = udevice.devnode();
        let cs_path = CString::new(hidrawpath.clone())?;
        let api = hidapi::HidApi::new()?;
        let device = api.open_path(&cs_path)?;

        let info = device.get_device_info()?;
        if info.vendor_id() != VID || info.product_id() != PID {
            return Err(format!("Device '{hidrawpath}' is not a Flydigi Vader 4 Pro").into());
        }

        Ok(Self {
            device: device,
            state: None,
            dpad: Default::default(),
        })
    }

    pub fn poll(&mut self) -> Result<Vec<Event>, Box<dyn Error + Send + Sync>> {
        // Read data from the device into a buffer
        let mut buf = [0; PACKET_SIZE];
        let bytes_read = self.device.read_timeout(&mut buf[..], HID_TIMEOUT)?;
        if  bytes_read != PACKET_SIZE {
            log::warn!("Got unhandled packet size {bytes_read}, someone should look into that...");
            return Ok(vec![]);
        } 
        let input_report = PackedInputDataReport::unpack(&buf)?;

        // Print input report for debugging
        //log::trace!("--- Input report ---");
        //log::trace!("{input_report}");
        //log::trace!("---- End Report ----");

        // Update the state
        let old_dinput_state = self.update_state(input_report);

        // Translate the state into a stream of input events
        let events = self.translate_events(old_dinput_state);
        Ok(events)
    }

    /// Update input state
    fn update_state(
        &mut self,
        input_report: PackedInputDataReport,
    ) -> Option<PackedInputDataReport> {
        let old_state = self.state;
        self.state = Some(input_report);
        old_state
    }

    /// Translate the state into individual events
    fn translate_events(&mut self, old_state: Option<PackedInputDataReport>) -> Vec<Event> {
        let mut events = Vec::new();
        let Some(state) = self.state else {
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
        if state.rb != old_state.rb {
            events.push(Event::Button(ButtonEvent::RB(BinaryInput {
                pressed: state.rb,
            })));
        }
        if state.lb != old_state.lb {
            events.push(Event::Button(ButtonEvent::LB(BinaryInput {
                pressed: state.lb,
            })));
        }
        if state.steam != old_state.steam {
            events.push(Event::Button(ButtonEvent::Steam(BinaryInput {
                pressed: state.steam,
            })));
        }
        if state.quick != old_state.quick {
            events.push(Event::Button(ButtonEvent::Quick(BinaryInput {
                pressed: state.quick,
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
        if state.lsclick != old_state.lsclick {
            events.push(Event::Button(ButtonEvent::LSClick(BinaryInput {
                pressed: state.lsclick,
            })));
        }
        if state.rsclick != old_state.rsclick {
            events.push(Event::Button(ButtonEvent::RSClick(BinaryInput {
                pressed: state.rsclick,
            })));
        }
        if state.ltdigital != old_state.ltdigital {
            events.push(Event::Button(ButtonEvent::LTDigital(BinaryInput {
                pressed: state.ltdigital,
            })));
        }
        if state.rtdigital != old_state.rtdigital {
            events.push(Event::Button(ButtonEvent::RTDigital(BinaryInput {
                pressed: state.rtdigital,
            })));
        }
        if state.l4 != old_state.l4 {
            events.push(Event::Button(ButtonEvent::L4(BinaryInput {
                pressed: state.l4,
            })));
        }
        if state.l5 != old_state.l5 {
            events.push(Event::Button(ButtonEvent::L5(BinaryInput {
                pressed: state.l5,
            })));
        }
        if state.r4 != old_state.r4 {
            events.push(Event::Button(ButtonEvent::R4(BinaryInput {
                pressed: state.r4,
            })));
        }
        if state.r5 != old_state.r5 {
            events.push(Event::Button(ButtonEvent::R5(BinaryInput {
                pressed: state.r5,
            })));
        }
        if state.m1 != old_state.m1 {
            events.push(Event::Button(ButtonEvent::M1(BinaryInput {
                pressed: state.m1,
            })));
        }
        if state.m2 != old_state.m2 {
            events.push(Event::Button(ButtonEvent::M2(BinaryInput {
                pressed: state.m2,
            })));
        }


         if state.dpad != old_state.dpad {
            let up = [Direction::Up, Direction::UpRight, Direction::UpLeft].contains(&state.dpad);
            let down =
                [Direction::Down, Direction::DownRight, Direction::DownLeft].contains(&state.dpad);
            let left =
                [Direction::Left, Direction::DownLeft, Direction::UpLeft].contains(&state.dpad);
            let right =
                [Direction::Right, Direction::DownRight, Direction::UpRight].contains(&state.dpad);
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
        if state.joystick_l_x != old_state.joystick_l_x
            || state.joystick_l_y != old_state.joystick_l_y
        {
            events.push(Event::Joystick(JoystickEvent::LStick(JoystickInput {
                x: state.joystick_l_x,
                y: state.joystick_l_y,
            })));
        }
        if state.joystick_r_x != old_state.joystick_r_x
            || state.joystick_r_y != old_state.joystick_r_y
        {
            events.push(Event::Joystick(JoystickEvent::RStick(JoystickInput {
                x: state.joystick_r_x,
                y: state.joystick_r_y,
            })));
        }

        if state.lt_analog != old_state.lt_analog {
            events.push(Event::Trigger(TriggerEvent::LTAnalog(TriggerInput {
                value: state.lt_analog,
            })));
        }
        if state.rt_analog != old_state.rt_analog {
            events.push(Event::Trigger(TriggerEvent::RTAnalog(TriggerInput {
                value: state.rt_analog,
            })));
        }

 //       // Accelerometer events
 //       events.push(Event::Inertia(InertialEvent::Accelerometer(
 //           InertialInput {
 //               x: -state.accel_x.to_primitive(),
 //               y: state.accel_y.to_primitive(),
 //               z: -state.accel_z.to_primitive(),
 //           },
 //       )));
 //       events.push(Event::Inertia(InertialEvent::Gyro(InertialInput {
 //           x: -state.gyro_x.to_primitive(),
 //           y: state.gyro_y.to_primitive(),
 //           z: -state.gyro_z.to_primitive(),
 //       })));

        //log::trace!("Got events: {events:?}");
        events
    }
}
