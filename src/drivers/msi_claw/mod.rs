pub mod driver;
pub mod event;
pub mod hid_report;

// Hardware ID's
pub const VID: u16 = 0x0db0;
pub const PID: u16 = 0x1902;

// Input report sizes
const INPUT_PACKET_SIZE: usize = 64;

// Input report axis ranges
pub const AXIS_MAX: f64 = 255.0;

// Timeouts
const HID_TIMEOUT: i32 = 10;
