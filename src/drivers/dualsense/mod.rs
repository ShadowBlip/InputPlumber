pub mod driver;
pub mod event;
pub mod hid_report;
#[cfg(test)]
mod hid_report_test;
pub mod report_descriptor;

// Source: https://github.com/torvalds/linux/blob/master/drivers/hid/hid-playstation.c
pub const DS5_EDGE_NAME: &str = "Sony Interactive Entertainment DualSense Edge Wireless Controller";
pub const DS5_EDGE_VERSION: u16 = 256;
pub const DS5_EDGE_VID: u16 = 0x054c;
pub const DS5_EDGE_PID: u16 = 0x0df2;

pub const DS5_NAME: &str = "Sony Interactive Entertainment DualSense Wireless Controller";
pub const DS5_VERSION: u16 = 0x8111;
pub const DS5_VID: u16 = 0x054c;
pub const DS5_PID: u16 = 0x0ce6;

pub const PIDS: [u16; 2] = [DS5_EDGE_PID, DS5_PID];

pub const FEATURE_REPORT_PAIRING_INFO: u8 = 0x09;
pub const FEATURE_REPORT_FIRMWARE_INFO: u8 = 0x20;
pub const FEATURE_REPORT_CALIBRATION: u8 = 0x05;

pub const INPUT_REPORT_USB: u8 = 0x01;
pub const INPUT_REPORT_USB_SIZE: usize = 64;
pub const INPUT_REPORT_BT: u8 = 0x31;
pub const INPUT_REPORT_BT_SIZE: usize = 78;
pub const OUTPUT_REPORT_USB: u8 = 0x02;
pub const OUTPUT_REPORT_USB_SIZE: usize = 63;
pub const OUTPUT_REPORT_USB_SHORT_SIZE: usize = 48;
pub const OUTPUT_REPORT_BT: u8 = 0x31;
pub const OUTPUT_REPORT_BT_SIZE: usize = 78;

// Input report axis ranges
pub const STICK_X_MIN: f64 = u8::MIN as f64;
pub const STICK_X_MAX: f64 = u8::MAX as f64;
pub const STICK_Y_MIN: f64 = u8::MIN as f64;
pub const STICK_Y_MAX: f64 = u8::MAX as f64;
pub const TRIGGER_MAX: f64 = u8::MAX as f64;

// DualSense hardware limits
pub const DS5_ACCEL_TO_SI: f64 = 0.00119710083;
pub const DS5_SI_TO_ACCEL: f64 = 0.101971621;
pub const DS5_GYRO_TO_RADS: f64 = 0.00001706026;
pub const DS5_RADS_TO_GYRO: f64 = 57.29577951;
pub const DS5_TOUCHPAD_WIDTH: f64 = 1919.0;
pub const DS5_TOUCHPAD_HEIGHT: f64 = 1079.0;
