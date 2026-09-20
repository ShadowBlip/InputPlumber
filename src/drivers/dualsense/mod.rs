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
pub const DS_FEATURE_REPORT_CALIBRATION_SIZE: usize = 41;

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
// Source: DS_ACC_RES_PER_G (8192) and DS_GYRO_RES_PER_DEG_S (1024) in
// https://github.com/torvalds/linux/blob/master/drivers/hid/hid-playstation.c
pub const DS5_ACCEL_RAW_TO_MPS2: f64 = 0.00119710083;
pub const DS5_MPS2_TO_ACCEL_RAW: f64 = 1.0 / DS5_ACCEL_RAW_TO_MPS2;
pub const DS5_GYRO_RAW_TO_RAD_S: f64 = 0.0010652644360316954;
pub const DS5_RAD_S_TO_GYRO_RAW: f64 = 1.0 / DS5_GYRO_RAW_TO_RAD_S;
pub const DS5_TOUCHPAD_WIDTH: f64 = 1919.0;
pub const DS5_TOUCHPAD_HEIGHT: f64 = 1079.0;

// Calibration report (0x05) values for the emulated target.
// From: https://github.com/torvalds/linux/blob/master/drivers/hid/hid-playstation.c
pub const DS5_CALIB_GYRO_SPEED: i16 = 125;
pub const DS5_CALIB_GYRO_PLUS: i16 = 2048;
pub const DS5_CALIB_ACCEL_PLUS: i16 = 8192;
