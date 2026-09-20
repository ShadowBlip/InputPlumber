pub mod driver;
pub mod event;
pub mod hid_report;
#[cfg(test)]
pub mod hid_report_test;
pub mod report_descriptor;

// Report ID
pub const REPORT_ID: u8 = 0x07;
pub const REPORT_ID_BT: u8 = 0x00;

// Input report size
pub const PACKET_SIZE: usize = 287;

// HID buffer read timeout
pub const HID_TIMEOUT: i32 = 4;

// Input report axis ranges
pub const JOY_AXIS_MAX: f64 = 255.0;
pub const JOY_AXIS_MIN: f64 = 0.0;
pub const TRIGGER_AXIS_MAX: f64 = 255.0;

pub const VID: u16 = 0x0F0D;
pub const PIDS: [u16; 2] = [0x0196, 0x01AB];

// Source: https://github.com/libsdl-org/SDL/blob/main/src/joystick/hidapi/SDL_hidapi_steam_hori.c
pub const HORIPAD_ACCEL_RAW_TO_MPS2: f64 = 0.00239420166015625;
pub const HORIPAD_MPS2_TO_ACCEL_RAW: f64 = 1.0 / HORIPAD_ACCEL_RAW_TO_MPS2;
pub const HORIPAD_GYRO_RAW_TO_RAD_S: f64 = 0.001090830782496456;
pub const HORIPAD_RAD_S_TO_GYRO_RAW: f64 = 1.0 / HORIPAD_GYRO_RAW_TO_RAD_S;
