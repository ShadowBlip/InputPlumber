use std::{error::Error, ffi::CString};

use hidapi::HidDevice;
use packed_struct::PackedStruct;

use crate::udev::device::UdevDevice;

use super::{
    event::{BinaryInput, Event, GamepadButtonEvent},
    hid_report::{GpdWin5ButtonReport, PACKET_SIZE},
};

// GPD Win 5 vendor HID device (Usage Page 0xFF00)
pub const VID: u16 = 0x2f24;
pub const PID: u16 = 0x0137;
pub const IID: i32 = 0x00;

const HID_TIMEOUT: i32 = 10;

pub struct GpdWin5ButtonDriver {
    device: HidDevice,
    state: Option<GpdWin5ButtonReport>,
}

impl GpdWin5ButtonDriver {
    pub fn new(udevice: UdevDevice) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let path = udevice.devnode();
        let cs_path = CString::new(path.clone())?;
        let api = hidapi::HidApi::new()?;
        let device = api.open_path(&cs_path)?;
        let info = device.get_device_info()?;
        if info.vendor_id() != VID || info.product_id() != PID {
            return Err(
                format!("Device '{path}' is not a GPD Win 5 button controller").into(),
            );
        }

        Ok(Self {
            device,
            state: None,
        })
    }

    /// Poll the device and read input reports
    pub fn poll(&mut self) -> Result<Vec<Event>, Box<dyn Error + Send + Sync>> {
        let mut buf = [0; PACKET_SIZE];
        let bytes_read = self.device.read_timeout(&mut buf[..], HID_TIMEOUT)?;

        let events = match bytes_read {
            PACKET_SIZE => {
                log::trace!("Got GPD Win 5 button event");
                let sized_buf = buf[..PACKET_SIZE].try_into()?;
                self.handle_input_report(sized_buf)?
            }
            0 => Vec::new(),
            _ => {
                log::trace!(
                    "Unexpected packet size for GPD Win 5 button data: {bytes_read}"
                );
                Vec::new()
            }
        };

        Ok(events)
    }

    fn handle_input_report(
        &mut self,
        buf: [u8; PACKET_SIZE],
    ) -> Result<Vec<Event>, Box<dyn Error + Send + Sync>> {
        let report = GpdWin5ButtonReport::unpack(&buf)?;
        let old_state = self.state;
        self.state = Some(report);

        let Some(old_state) = old_state else {
            return Ok(Vec::new());
        };

        Ok(self.translate_events(old_state))
    }

    fn translate_events(&self, old_state: GpdWin5ButtonReport) -> Vec<Event> {
        let mut events = Vec::new();
        let Some(state) = self.state else {
            return events;
        };

        if state.mode_switch != old_state.mode_switch {
            let pressed = state.mode_switch != 0;
            log::trace!("Mode switch button: {}", if pressed { "pressed" } else { "released" });
            events.push(Event::GamepadButton(GamepadButtonEvent::QuickAccess(
                BinaryInput { pressed },
            )));
        }

        if state.left_back != old_state.left_back {
            let pressed = state.left_back != 0;
            log::trace!("Left back button: {}", if pressed { "pressed" } else { "released" });
            events.push(Event::GamepadButton(GamepadButtonEvent::L4(
                BinaryInput { pressed },
            )));
        }

        if state.right_back != old_state.right_back {
            let pressed = state.right_back != 0;
            log::trace!("Right back button: {}", if pressed { "pressed" } else { "released" });
            events.push(Event::GamepadButton(GamepadButtonEvent::R4(
                BinaryInput { pressed },
            )));
        }

        events
    }
}
