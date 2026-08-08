use std::{error::Error, ffi::CString, time::{Duration, Instant}};

use hidapi::HidDevice;
use packed_struct::PackedStruct;

use crate::udev::device::UdevDevice;

use super::{
    event::{BinaryInput, Event, GamepadButtonEvent},
    hid_report::{MacroKeyboardDataReport}
};

pub const VID: u16 = 0x2f24;
pub const PID: u16 = 0x0135;
pub const IID: i32 = 0x01;

const RELEASE_DELAY: Duration = Duration::from_millis(40);

pub const L4_CODE: u8 = 0x46;
pub const R4_CODE: u8 = 0x48;

// Input report size
const KEYBOARD_PACKET_SIZE: usize = 8;

// HID buffer read timeout
const HID_TIMEOUT: i32 = 10;

pub struct MacroKeyboardDriver {
    /// HIDRAW device instance
    device: HidDevice,
    /// Whether or not we are currently holding L4
    l4_pressed: bool,
    /// Whether or not we are currently holding R4
    r4_pressed: bool,
    /// Timestamp of the last L4 event.
    l4_last_pressed: Instant,
    /// Timestamp of the last R4 event.
    r4_last_pressed: Instant,
    /// State for the keyboard device
    keyboard_state: Option<MacroKeyboardDataReport>,
}

impl MacroKeyboardDriver {
    pub fn new(udevice: UdevDevice) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let path = udevice.devnode();
        let cs_path = CString::new(path.clone())?;
        let api = hidapi::HidApi::new()?;
        let device = api.open_path(&cs_path)?;
        let info = device.get_device_info()?;
        if info.vendor_id() != VID || info.product_id() != PID {
            return Err(format!("Device '{path}' is not a GPD Win Mini macro keyboard").into());
        }

        Ok(Self {
            device,
            l4_pressed: false,
            r4_pressed: false,
            l4_last_pressed: Instant::now(),
            r4_last_pressed: Instant::now(),
            keyboard_state: None,
        })
    }

    /// Poll the device and read input reports
    pub fn poll(&mut self) -> Result<Vec<Event>, Box<dyn Error + Send + Sync>> {
        // Read data from the device into a buffer
        let mut buf = [0; KEYBOARD_PACKET_SIZE];
        let bytes_read = self.device.read_timeout(&mut buf[..], HID_TIMEOUT)?;

        let slice = &buf[..bytes_read];

        let mut events = match bytes_read {
            KEYBOARD_PACKET_SIZE => {
                log::trace!("Got macro event");
                let sized_buf = slice.try_into()?;
                self.handle_input_report(sized_buf)?
            }
            0 => {
                // Timed out, replay previous events to renew last_pressed
                self.translate_events(self.keyboard_state)
            }
            _ => {
                return Err("Invalid packet size for macro keyboard data.".into());
            }
        };

        // Check for release conditions
        if self.l4_pressed && self.l4_last_pressed.elapsed() > RELEASE_DELAY {
            log::trace!("Ended L4 event");
            events.push(
                Event::GamepadButton(GamepadButtonEvent::L4(BinaryInput { pressed: false}))
            );
            self.l4_pressed = false;
        }

        if self.r4_pressed && self.r4_last_pressed.elapsed() > RELEASE_DELAY {
            log::trace!("Ended R4 event");
            events.push(
                Event::GamepadButton(GamepadButtonEvent::R4(BinaryInput { pressed: false}))
            );
            self.r4_pressed = false;
        }

        Ok(events)
    }

    /// Unpacks the buffer into a [MacroKeyboardData] structure and updates
    /// the internal keyboard_state
    fn handle_input_report(
        &mut self,
        buf: [u8; KEYBOARD_PACKET_SIZE],
    ) -> Result<Vec<Event>, Box<dyn Error + Send + Sync>> {
        let input_report = MacroKeyboardDataReport::unpack(&buf)?;

        // Update the state
        let old_state = self.update_keyboard_state(input_report);

        // Translate the state into a stream of input events
        let events = self.translate_events(old_state);

        Ok(events)
    }

    /// Update macro keyboard state
    fn update_keyboard_state(
        &mut self,
        input_report: MacroKeyboardDataReport,
    ) -> Option<MacroKeyboardDataReport> {
        let old_state = self.keyboard_state;
        self.keyboard_state = Some(input_report);
        old_state
    }

    /// Translate the state into individual events
    fn translate_events(&mut self, old_state: Option<MacroKeyboardDataReport>) -> Vec<Event> {
        let mut events = Vec::new();
        let Some(state) = self.keyboard_state else {
            return events;
        };

        // Translate state changes into events if they have changed
        let Some(_) = old_state else {
            return events;
        };

        if state.has_key(L4_CODE) {
            self.l4_last_pressed = Instant::now();
            if !self.l4_pressed {
                log::trace!("Started L4 event");
                events.push(
                    Event::GamepadButton(GamepadButtonEvent::L4(BinaryInput { pressed: true}))
                );
                self.l4_pressed = true;
            }
        }

        if state.has_key(R4_CODE) {
            self.r4_last_pressed = Instant::now();
            if !self.r4_pressed {
                log::trace!("Started R4 event");
                events.push(
                    Event::GamepadButton(GamepadButtonEvent::R4(BinaryInput { pressed: true}))
                );
                self.r4_pressed = true;
            }
        }

        events
    }
}
