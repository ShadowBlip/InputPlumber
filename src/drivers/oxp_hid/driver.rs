use std::error::Error;
use std::ffi::CString;

use hidapi::HidDevice;

use super::event::{BinaryInput, ButtonEvent, Event};

pub const VID: u16 = 0x1a86;
pub const PID: u16 = 0xfe00;
pub const IID: i32 = 0x02;

const PACKET_SIZE: usize = 64;

// HID buffer read timeout
const HID_TIMEOUT: i32 = 10;

// HID command IDs
const CMD_BUTTON: u8 = 0xB2;

// Button codes for vendor HID report mode events
const BTN_GUIDE: u8 = 0x21;
const BTN_M1: u8 = 0x22;
const BTN_M2: u8 = 0x23;
const BTN_KEYBOARD: u8 = 0x24;

// B4 button mapping commands: configure M1→F14, M2→F13 keyboard keycodes
// to get independent back paddle HID reports without Xbox gamepad mirroring.
// Keyboard entry format: [btn_id, 0x02(kbd), 0x01, oxp_keycode, 0x00, 0x00]
// OXP key encoding: F(n) = 0x59 + n, so F13=0x66, F14=0x67.
const INIT_CMD_1: [u8; PACKET_SIZE] = gen_cmd(
    0xB4,
    &[
        0x02, 0x38, 0x20, 0x01, 0x01, 0x01, 0x01, 0x01, 0x00, 0x00, 0x00, 0x02, 0x01, 0x02, 0x00,
        0x00, 0x00, 0x03, 0x01, 0x03, 0x00, 0x00, 0x00, 0x04, 0x01, 0x04, 0x00, 0x00, 0x00, 0x05,
        0x01, 0x05, 0x00, 0x00, 0x00, 0x06, 0x01, 0x06, 0x00, 0x00, 0x00, 0x07, 0x01, 0x07, 0x00,
        0x00, 0x00, 0x08, 0x01, 0x08, 0x00, 0x00, 0x00, 0x09, 0x01, 0x09, 0x00, 0x00, 0x00,
    ],
);

const INIT_CMD_2: [u8; PACKET_SIZE] = gen_cmd(
    0xB4,
    &[
        0x02, 0x38, 0x20, 0x02, 0x01, 0x0a, 0x01, 0x0a, 0x00, 0x00, 0x00, 0x0b, 0x01, 0x0b, 0x00,
        0x00, 0x00, 0x0c, 0x01, 0x0c, 0x00, 0x00, 0x00, 0x0d, 0x01, 0x0d, 0x00, 0x00, 0x00, 0x0e,
        0x01, 0x0e, 0x00, 0x00, 0x00, 0x0f, 0x01, 0x0f, 0x00, 0x00, 0x00, 0x10, 0x01, 0x10, 0x00,
        0x00, 0x00, 0x22, 0x02, 0x01, 0x67, 0x00, 0x00, 0x23, 0x02, 0x01, 0x66, 0x00, 0x00,
    ],
);

/// Generate a command with 0x3F framing: [cid, 0x3F, 0x01, ...data, 0x3F, cid]
const fn gen_cmd(cid: u8, data: &[u8]) -> [u8; PACKET_SIZE] {
    let mut buf = [0u8; PACKET_SIZE];
    buf[0] = cid;
    buf[1] = 0x3F;
    buf[2] = 0x01;

    let mut i = 0;
    while i < data.len() && (i + 3) < PACKET_SIZE - 2 {
        buf[i + 3] = data[i];
        i += 1;
    }

    buf[PACKET_SIZE - 2] = 0x3F;
    buf[PACKET_SIZE - 1] = cid;
    buf
}

// B3 vibration intensity: set to max (5) so Xbox gamepad rumble works.
// MCU does not persist this across reboots, so it must be sent every init.
// Payload: 15-byte header + 35 zero padding + 9-byte tail = 59 bytes.
const B3_VIBRATION: [u8; PACKET_SIZE] = gen_cmd(
    0xB3,
    &[
        0x02, 0x38, 0x02, 0xE3, 0x39, 0xE3, 0x39, 0xE3, 0x39, 0x01, 0x05, 0x05,
        0xE3, 0x39, 0xE3, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x39, 0xE3, 0x39, 0xE3, 0xE3, 0x02, 0x04, 0x39, 0x39,
    ],
);

// B2 report mode activation: ENABLE then DISABLE cycle.
// Required on Apex; harmless on X1 Mini (tested: both phases produce events).
const B2_ENABLE: [u8; PACKET_SIZE] = gen_cmd(CMD_BUTTON, &[0x03, 0x01, 0x02]);
const B2_DISABLE: [u8; PACKET_SIZE] = gen_cmd(CMD_BUTTON, &[0x00, 0x01, 0x02]);

pub struct Driver {
    device: HidDevice,
    btn_state: [bool; 0x25],
    initialized: bool,
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
        log::info!(
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
            initialized: false,
        })
    }

    /// Drain ACK responses from the device, logging each one.
    fn drain_responses(&self, phase: &str, buf: &mut [u8]) -> Result<u32, Box<dyn Error + Send + Sync>> {
        let mut count = 0u32;
        for _ in 0..10 {
            let n = self.device.read_timeout(buf, 50)?;
            if n == 0 {
                break;
            }
            count += 1;
            let cid = buf[0];
            log::info!(
                "OXP HID: {phase} ACK #{count}: CID=0x{cid:02X} ({n}B) [{}]",
                hex_prefix(buf, 16)
            );
        }
        if count == 0 {
            log::warn!("OXP HID: {phase} — no ACK received");
        }
        Ok(count)
    }

    /// Send initialization commands: B4 button mapping → B2 report mode → B3 vibration.
    /// B3 must be sent AFTER the B2 cycle because B2 ENABLE resets vibration intensity.
    fn initialize(&mut self) -> Result<(), Box<dyn Error + Send + Sync>> {
        log::info!("OXP HID: starting initialization sequence");
        let mut drain_buf = [0u8; PACKET_SIZE];

        // Phase 1: B4 button mappings
        let w1 = self.device.write(&INIT_CMD_1)?;
        log::info!("OXP HID: B4 page1 sent ({w1}B)");
        std::thread::sleep(std::time::Duration::from_millis(50));

        let w2 = self.device.write(&INIT_CMD_2)?;
        log::info!("OXP HID: B4 page2 sent ({w2}B) — M1→F14(0x67), M2→F13(0x66)");
        std::thread::sleep(std::time::Duration::from_millis(50));

        self.drain_responses("B4", &mut drain_buf)?;

        // Phase 2: B2 report mode ENABLE→DISABLE cycle
        let w3 = self.device.write(&B2_ENABLE)?;
        log::info!("OXP HID: B2 ENABLE sent ({w3}B)");
        std::thread::sleep(std::time::Duration::from_millis(200));
        self.drain_responses("B2-EN", &mut drain_buf)?;

        let w4 = self.device.write(&B2_DISABLE)?;
        log::info!("OXP HID: B2 DISABLE sent ({w4}B)");
        std::thread::sleep(std::time::Duration::from_millis(100));
        self.drain_responses("B2-DIS", &mut drain_buf)?;

        // Phase 3: B3 vibration (must be AFTER B2 cycle)
        let w5 = self.device.write(&B3_VIBRATION)?;
        log::info!("OXP HID: B3 vibration sent ({w5}B) — intensity=5");
        std::thread::sleep(std::time::Duration::from_millis(50));
        self.drain_responses("B3", &mut drain_buf)?;

        log::info!(
            "OXP HID: initialization complete — B4({w1}+{w2}B) B2({w3}+{w4}B) B3({w5}B)"
        );
        self.initialized = true;
        Ok(())
    }

    /// Poll the device and read input reports
    pub fn poll(&mut self) -> Result<Vec<Event>, Box<dyn Error + Send + Sync>> {
        if !self.initialized {
            self.initialize()?;
        }

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

        let cid = buf[0];
        let valid = buf[1] == 0x3F && buf[PACKET_SIZE - 2] == 0x3F;

        if !valid {
            log::warn!(
                "OXP HID: invalid frame (byte1=0x{:02x}, byte62=0x{:02x}): [{}]",
                buf[1],
                buf[PACKET_SIZE - 2],
                hex_prefix(&buf, 16)
            );
            return Ok(Vec::new());
        }

        if cid != CMD_BUTTON {
            log::info!(
                "OXP HID: non-B2 packet CID=0x{cid:02X}: [{}]",
                hex_prefix(&buf, 16)
            );
            return Ok(Vec::new());
        }

        let pkt_type = buf[3];
        let flag = buf[5];
        let btn = buf[6];
        let func_code = buf[7];
        let pressed = buf[12] == 1;

        let event = match btn {
            BTN_M1 => ButtonEvent::M1(BinaryInput { pressed }),
            BTN_M2 => ButtonEvent::M2(BinaryInput { pressed }),
            BTN_KEYBOARD => ButtonEvent::Keyboard(BinaryInput { pressed }),
            BTN_GUIDE => ButtonEvent::Guide(BinaryInput { pressed }),
            0x00 => return Ok(Vec::new()),
            _ => {
                log::warn!(
                    "OXP HID: unknown btn=0x{btn:02x} type=0x{pkt_type:02x} \
                     flag=0x{flag:02x} func=0x{func_code:02x} state=0x{:02x}: [{}]",
                    buf[12],
                    hex_prefix(&buf, 16)
                );
                return Ok(Vec::new());
            }
        };

        // Debounce
        if let Some(prev) = self.btn_state.get_mut(btn as usize) {
            if *prev == pressed {
                return Ok(Vec::new());
            }
            *prev = pressed;
        }

        log::info!(
            "OXP HID: btn=0x{btn:02x} {} (type=0x{pkt_type:02x} flag=0x{flag:02x} func=0x{func_code:02x})",
            if pressed { "PRESSED" } else { "RELEASED" }
        );

        Ok(vec![Event::Button(event)])
    }
}
