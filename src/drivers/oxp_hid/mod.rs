pub mod driver;
pub mod event;
pub mod hid_report;

pub const VID: u16 = 0x1a86;
pub const PID: u16 = 0xfe00;

const PACKET_SIZE: usize = 64;

// HID buffer read timeout
const HID_TIMEOUT: i32 = 10;

// HID command IDs
const CMD_BUTTON: u8 = 0xB2;
