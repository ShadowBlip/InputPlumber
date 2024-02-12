use std::{error::Error, ffi::CString};

use crate::drivers::steam_deck::hid_report::PackedInputDataReport;
use hidapi::HidDevice;
use packed_struct::{types::SizedInteger, PackedStruct};

use super::{
    event::{
        AccelerometerEvent, AccelerometerInput, AxisEvent, AxisInput, BinaryInput, ButtonEvent,
        Event, TriggerEvent, TriggerInput,
    },
    hid_report::{PackedMappingsReport, ReportType},
};

pub const VID: u16 = 0x28de;
pub const PID: u16 = 0x1205;
const PACKET_SIZE: usize = 64;
const HID_TIMEOUT: i32 = 5000;

pub struct Driver {
    state: Option<PackedInputDataReport>,
    device: HidDevice,
}

impl Driver {
    pub fn new(path: String) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let path = CString::new(path)?;
        let api = hidapi::HidApi::new()?;
        let device = api.open_path(&path)?;
        let info = device.get_device_info()?;
        if info.vendor_id() != VID || info.product_id() != PID {
            return Err("Device '{path}' is not a Steam Deck Controller".into());
        }

        Ok(Self {
            device,
            state: None,
        })
    }

    /// Poll the device and read input reports
    pub fn poll(&mut self) -> Result<Vec<Event>, Box<dyn Error + Send + Sync>> {
        // Read data from the device into a buffer
        let mut buf = [0; PACKET_SIZE];
        let bytes_read = self.device.read_timeout(&mut buf[..], HID_TIMEOUT)?;

        // All report descriptors are 64 bytes, so this is just to be safe
        if bytes_read != PACKET_SIZE {
            let msg = format!("Invalid input report size was received from gamepad device: {bytes_read}/{PACKET_SIZE}");
            return Err(msg.into());
        }

        // Handle the incoming input report
        let events = self.handle_input_report(buf)?;

        Ok(events)
    }

    /// Set lizard mode, which will automatically try to emulate mouse/keyboard
    /// if enabled.
    pub fn set_lizard_mode(&self, enabled: bool) -> Result<(), Box<dyn Error + Send + Sync>> {
        // Initialize the report to send
        let report = match enabled {
            true => PackedMappingsReport {
                report_id: ReportType::DefaultMappings as u8,
            },
            false => PackedMappingsReport {
                report_id: ReportType::ClearMappings as u8,
            },
        };

        // Write the report to the device
        let buf = report.pack()?;
        let _bytes_written = self.device.write(&buf)?;

        Ok(())
    }

    /// Strangely, the only known method to disable keyboard emulation only does
    /// so for a few seconds, whereas disabling the mouse is permanent until
    /// re-enabled.  This means we have to run a separate thread which wakes up
    /// every couple seconds and disabled the keyboard again using the
    /// CLEAR_MAPPINGS report.  If there's a better way to do this, I'd love to
    /// know about it.  Looking at you, Valve.
    pub fn handle_lizard_mode(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.set_lizard_mode(false)
    }

    /// Unpacks the buffer into a [PackedInputDataReport] structure and updates
    /// the internal gamepad state
    fn handle_input_report(
        &mut self,
        buf: [u8; 64],
    ) -> Result<Vec<Event>, Box<dyn Error + Send + Sync>> {
        let input_report = PackedInputDataReport::unpack(&buf)?;

        // Print input report for debugging
        //log::debug!("--- Input report ---");
        //log::debug!("{input_report}");
        //log::debug!("---- End Report ----");

        // Update the state
        let old_state = self.update_state(input_report);

        // Translate the state into a stream of input events
        let events = self.translate(old_state);

        Ok(events)
    }

    /// Update internal gamepad state
    fn update_state(
        &mut self,
        input_report: PackedInputDataReport,
    ) -> Option<PackedInputDataReport> {
        let old_state = self.state;
        self.state = Some(input_report);
        old_state
    }

    /// Translate the state into individual events
    fn translate(&self, old_state: Option<PackedInputDataReport>) -> Vec<Event> {
        let mut events = Vec::new();
        let Some(state) = self.state else {
            return events;
        };

        // Translate state changes into events if they have changed
        if let Some(old_state) = old_state {
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
            if state.options != old_state.options {
                events.push(Event::Button(ButtonEvent::Options(BinaryInput {
                    pressed: state.options,
                })));
            }
            if state.steam != old_state.steam {
                events.push(Event::Button(ButtonEvent::Steam(BinaryInput {
                    pressed: state.steam,
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
            if state.l1 != old_state.l1 {
                events.push(Event::Button(ButtonEvent::L1(BinaryInput {
                    pressed: state.l1,
                })));
            }
            if state.l2 != old_state.l2 {
                events.push(Event::Button(ButtonEvent::L2(BinaryInput {
                    pressed: state.l2,
                })));
            }
            if state.l3 != old_state.l3 {
                events.push(Event::Button(ButtonEvent::L3(BinaryInput {
                    pressed: state.l3,
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
            if state.r1 != old_state.r1 {
                events.push(Event::Button(ButtonEvent::R1(BinaryInput {
                    pressed: state.r1,
                })));
            }
            if state.r2 != old_state.r2 {
                events.push(Event::Button(ButtonEvent::R2(BinaryInput {
                    pressed: state.r2,
                })));
            }
            if state.r3 != old_state.r3 {
                events.push(Event::Button(ButtonEvent::R3(BinaryInput {
                    pressed: state.r3,
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
            if state.r_pad_touch != old_state.r_pad_touch {
                events.push(Event::Button(ButtonEvent::RPadTouch(BinaryInput {
                    pressed: state.r_pad_touch,
                })));
            }
            if state.l_pad_touch != old_state.l_pad_touch {
                events.push(Event::Button(ButtonEvent::LPadTouch(BinaryInput {
                    pressed: state.l_pad_touch,
                })));
            }
            if state.r_pad_press != old_state.r_pad_press {
                events.push(Event::Button(ButtonEvent::RPadPress(BinaryInput {
                    pressed: state.r_pad_press,
                })));
            }
            if state.l_pad_press != old_state.l_pad_press {
                events.push(Event::Button(ButtonEvent::LPadPress(BinaryInput {
                    pressed: state.l_pad_press,
                })));
            }
            if state.r_stick_touch != old_state.r_stick_touch {
                events.push(Event::Button(ButtonEvent::RStickTouch(BinaryInput {
                    pressed: state.r_stick_touch,
                })));
            }
            if state.l_stick_touch != old_state.l_stick_touch {
                events.push(Event::Button(ButtonEvent::LStickTouch(BinaryInput {
                    pressed: state.l_stick_touch,
                })));
            }

            // Axis events
            if state.l_pad_x != old_state.l_pad_x || state.l_pad_y != old_state.l_pad_y {
                events.push(Event::Axis(AxisEvent::LPad(AxisInput {
                    x: state.l_pad_x.to_primitive(),
                    y: state.l_pad_y.to_primitive(),
                })));
            }
            if state.r_pad_x != old_state.r_pad_x || state.r_pad_y != old_state.r_pad_y {
                events.push(Event::Axis(AxisEvent::RPad(AxisInput {
                    x: state.r_pad_x.to_primitive(),
                    y: state.r_pad_y.to_primitive(),
                })));
            }
            if state.l_stick_x != old_state.l_stick_x || state.l_stick_y != old_state.l_stick_y {
                events.push(Event::Axis(AxisEvent::LStick(AxisInput {
                    x: state.l_stick_x.to_primitive(),
                    y: state.l_stick_y.to_primitive(),
                })));
            }
            if state.r_stick_x != old_state.r_stick_x || state.r_stick_y != old_state.r_stick_y {
                events.push(Event::Axis(AxisEvent::RStick(AxisInput {
                    x: state.r_stick_x.to_primitive(),
                    y: state.r_stick_y.to_primitive(),
                })));
            }

            // Trigger events
            if state.l_trigg != old_state.l_trigg {
                events.push(Event::Trigger(TriggerEvent::LTrigger(TriggerInput {
                    value: state.l_trigg.to_primitive(),
                })));
            }
            if state.r_trigg != old_state.r_trigg {
                events.push(Event::Trigger(TriggerEvent::RTrigger(TriggerInput {
                    value: state.r_trigg.to_primitive(),
                })));
            }
            if state.l_pad_force != old_state.l_pad_force {
                events.push(Event::Trigger(TriggerEvent::LPadForce(TriggerInput {
                    value: state.l_pad_force.to_primitive(),
                })));
            }
            if state.r_pad_force != old_state.r_pad_force {
                events.push(Event::Trigger(TriggerEvent::RPadForce(TriggerInput {
                    value: state.r_pad_force.to_primitive(),
                })));
            }
            if state.l_stick_force != old_state.l_stick_force {
                events.push(Event::Trigger(TriggerEvent::LStickForce(TriggerInput {
                    value: state.l_stick_force.to_primitive(),
                })));
            }
            if state.r_stick_force != old_state.r_stick_force {
                events.push(Event::Trigger(TriggerEvent::RStickForce(TriggerInput {
                    value: state.r_stick_force.to_primitive(),
                })));
            }

            // Accelerometer events
            events.push(Event::Accelerometer(AccelerometerEvent::Accelerometer(
                AccelerometerInput {
                    x: state.accel_x.to_primitive(),
                    y: state.accel_y.to_primitive(),
                    z: state.accel_z.to_primitive(),
                },
            )));
            events.push(Event::Accelerometer(AccelerometerEvent::Attitude(
                AccelerometerInput {
                    x: state.pitch.to_primitive(),
                    y: state.yaw.to_primitive(),
                    z: state.roll.to_primitive(),
                },
            )));
        };

        events
    }
}
