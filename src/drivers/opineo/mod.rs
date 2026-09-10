use std::time::Duration;

pub mod driver;
pub mod event;
pub mod hid_report;
#[cfg(test)]
pub mod hid_report_test;

// Hardware IDs
pub const VID: u16 = 0x0911;
pub const PID: u16 = 0x5288;
pub const LPAD_NAMES: [&str; 2] = ["OPI0001:00", "SYNA3602:00"];
pub const RPAD_NAMES: [&str; 2] = ["OPI0002:00", "SYNA3602:01"];

// Report ID
pub const TOUCH_DATA: u8 = 0x04;

// Input report size
const TOUCHPAD_PACKET_SIZE: usize = 10;

// HID buffer read timeout
const TOUCHPAD_TIMEOUT: i32 = 8;

// Input report axis ranges
pub const PAD_X_MAX: f64 = 512.0;
pub const PAD_Y_MAX: f64 = 512.0;
pub const PAD_FORCE_MAX: f64 = 127.0;
pub const PAD_FORCE_NORMAL: u8 = 32; /* Simulated average */

const CLICK_DELAY: Duration = Duration::from_millis(75);
