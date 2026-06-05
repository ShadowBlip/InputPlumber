use core::mem::size_of;
use std::{error::Error, ffi::CString};

use hidapi::HidDevice;
use packed_struct::PackedStruct;

use crate::{
    drivers::ultimate_2::{
        event::{InertialEvent, InertialInput},
        hid_report::{DPadDirection, PackedInputDataReport, PackedRumbleOutputReport},
        PID, REPORT_ID_INPUT, REPORT_ID_RUMBLE, VID,
    },
    udev::device::UdevDevice,
};

use super::event::{
    AxisEvent, BinaryInput, ButtonEvent, Event, JoyAxisInput, TriggerEvent, TriggerInput,
};

// Input report size
const PACKET_SIZE: usize = 34;

// HID buffer read timeout
const HID_TIMEOUT: i32 = 10;

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
    state: Option<PackedInputDataReport>,
    /// Last DPad state
    dpad: DPadState,
}

impl Driver {
    pub fn new(udev_device: UdevDevice) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let fmtpath = udev_device.devnode().clone();
        let path = CString::new(fmtpath.clone())?;
        let api = hidapi::HidApi::new()?;
        let hid_device = api.open_path(&path)?;
        let info = hid_device.get_device_info()?;

        if info.vendor_id() != VID || info.product_id() != PID {
            return Err(
                format!("Device '{fmtpath}' is not an 8BitDo Ultimate 2 Controller").into(),
            );
        }

        Ok(Self {
            device: hid_device,
            state: None,
            dpad: Default::default(),
        })
    }

    /// Rumble the gamepad
    pub fn rumble(&self, strong: u8, weak: u8) -> Result<(), Box<dyn Error + Send + Sync>> {
        let state = PackedRumbleOutputReport {
            report_id: REPORT_ID_RUMBLE,
            strong_magnitude: strong,
            weak_magnitude: weak,
            _padding_3: Default::default(),
            _padding_4: Default::default(),
        };

        let buf = state.pack()?;
        let bytes_written = self.device.write(&buf)?;
        if bytes_written != size_of::<PackedRumbleOutputReport>() {
            return Err("Failed to write rumble report".to_string().into());
        }
        Ok(())
    }

    /// Poll the device and read input reports
    pub fn poll(&mut self) -> Result<Vec<Event>, Box<dyn Error + Send + Sync>> {
        // Read data from the device into a buffer
        let mut buf = [0; PACKET_SIZE];
        let bytes_read = self.device.read_timeout(&mut buf[..], HID_TIMEOUT)?;

        let report_id = buf[0];
        let slice = &buf[..bytes_read];
        log::debug!("Got Report ID: {report_id}");
        log::debug!("Got Report Size: {bytes_read}");

        let events = match report_id {
            REPORT_ID_INPUT => {
                log::trace!("Got input data.");
                if bytes_read != PACKET_SIZE {
                    return Err("Invalid packet size for input data.".into());
                }
                // Handle the incoming input report
                let sized_buf = slice.try_into()?;
                log::debug!("Got Report Raw Data: {:?}", sized_buf);

                self.handle_input_report(sized_buf)?
            }
            _ => {
                log::debug!("Invalid Report ID.");
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
        let input_report = PackedInputDataReport::unpack(&buf)?;

        // Print input report for debugging
        log::debug!("--- Input report ---");
        log::debug!("{input_report}");
        log::debug!("---- End Report ----");

        // Update the state
        let old_dinput_state = self.update_state(input_report);

        // Translate the state into a stream of input events
        let events = self.translate_events(old_dinput_state);

        Ok(events)
    }

    /// Update touchinput state
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
        let Some(state) = self.state.as_ref() else {
            return events;
        };

        let Some(old_state) = old_state else {
            return events;
        };

        // Translate state changes into events if they have changed
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
        if state.button_view != old_state.button_view {
            events.push(Event::Button(ButtonEvent::View(BinaryInput {
                pressed: state.button_view,
            })));
        }
        if state.button_menu != old_state.button_menu {
            events.push(Event::Button(ButtonEvent::Menu(BinaryInput {
                pressed: state.button_menu,
            })));
        }
        if state.button_guide != old_state.button_guide {
            events.push(Event::Button(ButtonEvent::Guide(BinaryInput {
                pressed: state.button_guide,
            })));
        }
        if state.button_r1 != old_state.button_r1 {
            events.push(Event::Button(ButtonEvent::R1(BinaryInput {
                pressed: state.button_r1,
            })));
        }
        if state.button_l1 != old_state.button_l1 {
            events.push(Event::Button(ButtonEvent::L1(BinaryInput {
                pressed: state.button_l1,
            })));
        }
        if state.button_r2 != old_state.button_r2 {
            events.push(Event::Button(ButtonEvent::R2(BinaryInput {
                pressed: state.button_r2,
            })));
        }
        if state.button_l2 != old_state.button_l2 {
            events.push(Event::Button(ButtonEvent::L2(BinaryInput {
                pressed: state.button_l2,
            })));
        }
        if state.button_r3 != old_state.button_r3 {
            events.push(Event::Button(ButtonEvent::R3(BinaryInput {
                pressed: state.button_r3,
            })));
        }
        if state.button_l3 != old_state.button_l3 {
            events.push(Event::Button(ButtonEvent::L3(BinaryInput {
                pressed: state.button_l3,
            })));
        }
        if state.button_l4 != old_state.button_l4 {
            events.push(Event::Button(ButtonEvent::L4(BinaryInput {
                pressed: state.button_l4,
            })));
        }
        if state.button_r4 != old_state.button_r4 {
            events.push(Event::Button(ButtonEvent::R4(BinaryInput {
                pressed: state.button_r4,
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
                DPadDirection::UpLeft,
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
        if state.joystick_l_x != old_state.joystick_l_x
            || state.joystick_l_y != old_state.joystick_l_y
        {
            events.push(Event::Axis(AxisEvent::LStick(JoyAxisInput {
                x: state.joystick_l_x,
                y: state.joystick_l_y,
            })));
        }
        if state.joystick_r_x != old_state.joystick_r_x
            || state.joystick_r_y != old_state.joystick_r_y
        {
            events.push(Event::Axis(AxisEvent::RStick(JoyAxisInput {
                x: state.joystick_r_x,
                y: state.joystick_r_y,
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

        if state.accel_x != old_state.accel_x
            || state.accel_y != old_state.accel_y
            || state.accel_z != old_state.accel_z
        {
            events.push(Event::Inertia(InertialEvent::Accelerometer(
                InertialInput {
                    x: i16::from(state.accel_x),
                    y: i16::from(state.accel_y),
                    z: i16::from(state.accel_z),
                },
            )))
        };

        if state.gyro_x != old_state.gyro_x
            || state.gyro_y != old_state.gyro_y
            || state.gyro_z != old_state.gyro_z
        {
            events.push(Event::Inertia(InertialEvent::Gyro(InertialInput {
                x: i16::from(state.gyro_x),
                y: i16::from(state.gyro_y),
                z: i16::from(state.gyro_z),
            })))
        };

        log::trace!("Got events: {events:?}");
        events
    }
}
