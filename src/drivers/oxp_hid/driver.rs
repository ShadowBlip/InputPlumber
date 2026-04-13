use std::error::Error;
use std::ffi::CString;

use hidapi::HidDevice;
use packed_struct::prelude::*;

use super::event::{BinaryInput, ButtonEvent, Event};
use super::hid_report::{ButtonId, InputDataReport};

pub const VID: u16 = 0x1a86;
pub const PID: u16 = 0xfe00;
pub const IID: i32 = 0x02;

const PACKET_SIZE: usize = 64;

// HID buffer read timeout
const HID_TIMEOUT: i32 = 10;

// HID command IDs
const CMD_BUTTON: u8 = 0xB2;

pub struct Driver {
    device: HidDevice,
    btn_state: [bool; 0x25],
}

/// Format first N bytes of a buffer as hex string for logging.
fn hex_prefix(buf: &[u8], n: usize) -> String {
    buf[..n.min(buf.len())]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

impl Driver {
    pub fn new(path: String) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let cs_path = CString::new(path.clone())?;
        let api = hidapi::HidApi::new()?;
        let device = api.open_path(&cs_path)?;
        let info = device.get_device_info()?;
        log::debug!(
            "OXP HID: opened {path} (VID:{:04x} PID:{:04x} iface:{})",
            info.vendor_id(),
            info.product_id(),
            info.interface_number(),
        );
        if info.vendor_id() != VID || info.product_id() != PID {
            return Err(format!("Device '{path}' is not an OXP HID controller").into());
        }

        Ok(Self {
            device,
            btn_state: [false; 0x25],
        })
    }

    /// Poll the device and read input reports
    pub fn poll(&mut self) -> Result<Vec<Event>, Box<dyn Error + Send + Sync>> {
        let mut buf = [0u8; PACKET_SIZE];
        let bytes_read = self.device.read_timeout(&mut buf[..], HID_TIMEOUT)?;

        if bytes_read == 0 {
            return Ok(Vec::new());
        }

        if bytes_read < PACKET_SIZE {
            log::warn!(
                "OXP HID: short read ({bytes_read}B < {PACKET_SIZE}B): [{}]",
                hex_prefix(&buf, bytes_read)
            );
            return Ok(Vec::new());
        }

        // Pre-filter before unpack: check frame markers and CID using raw
        // bytes to avoid PackingError from enum fields in non-button packets
        // (e.g. B3/B4/B8 ACKs from the kernel hid-oxp driver).
        if buf[1] != 0x3F || buf[PACKET_SIZE - 2] != 0x3F {
            log::warn!(
                "OXP HID: invalid frame (byte1=0x{:02x}, byte62=0x{:02x}): [{}]",
                buf[1],
                buf[PACKET_SIZE - 2],
                hex_prefix(&buf, 16)
            );
            return Ok(Vec::new());
        }

        if buf[0] != CMD_BUTTON {
            return Ok(Vec::new());
        }

        let report = match InputDataReport::unpack(&buf) {
            Ok(r) => r,
            Err(e) => {
                log::warn!(
                    "OXP HID: failed to parse B2 report: {e}: [{}]",
                    hex_prefix(&buf, 16)
                );
                return Ok(Vec::new());
            }
        };

        let btn = report.btn;
        let pressed = report.press_state == 1;

        // Debounce: skip if state is unchanged
        if let Some(prev) = self.btn_state.get_mut(btn.to_primitive() as usize) {
            if *prev == pressed {
                return Ok(Vec::new());
            }
            *prev = pressed;
        }

        let event = match btn {
            ButtonId::M1 => ButtonEvent::M1(BinaryInput { pressed }),
            ButtonId::M2 => ButtonEvent::M2(BinaryInput { pressed }),
            ButtonId::Keyboard => ButtonEvent::Keyboard(BinaryInput { pressed }),
            ButtonId::Guide => ButtonEvent::Guide(BinaryInput { pressed }),
            _ => return Ok(Vec::new()),
        };

        log::debug!(
            "OXP HID: btn=0x{:02x} {} (type=0x{:02x} flag=0x{:02x} func=0x{:02x})",
            btn.to_primitive(),
            if pressed { "PRESSED" } else { "RELEASED" },
            report.pkt_type,
            report.flag,
            report.func_code,
        );

        Ok(vec![Event::Button(event)])
    }
}
