pub mod driver;
pub mod event;
pub mod hid_report;
#[cfg(test)]
pub mod hid_report_test;
pub mod report_descriptor;

// Report ID
pub const REPORT_ID: u8 = 0x07;

// Input report size
pub const PACKET_SIZE: usize = 287;

// HID buffer read timeout
pub const HID_TIMEOUT: i32 = 10;

// Input report axis ranges
pub const JOY_AXIS_MAX: f64 = 255.0;
pub const JOY_AXIS_MIN: f64 = 0.0;
pub const TRIGGER_AXIS_MAX: f64 = 255.0;

pub const VID: u16 = 0x0F0D;
pub const PIDS: [u16; 2] = [0x0196, 0x01AB];

pub const HORIPAD_ACCEL_TO_SI: f64 = 0.00059855041;
pub const HORIPAD_SI_TO_ACCEL: f64 = 0.101971621;
pub const HORIPAD_GYRO_TO_RADS: f64 = 0.00106422393;
pub const HORIPAD_RADS_TO_GYRO: f64 = 57.29577951;
