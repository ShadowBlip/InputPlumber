pub mod touchpad_driver_2023;
pub mod touchpad_driver_2024;
pub mod macro_keyboard_driver;
pub mod event;
pub mod hid_report;
#[cfg(test)]
pub mod hid_report_test;

// GPD Win Mini Touchpad (2023)
pub const TOUCHPAD_2023_VID: u16 = 0x093A;
pub const TOUCHPAD_2023_PID: u16 = 0x0255;
pub const TOUCHPAD_2023_IID: i32 = 0x00;
pub const TOUCHPAD_2023_TOUCH_DATA: u8 = 0x01;
pub const TOUCHPAD_2023_X_MAX: f64 = 2559.0;
pub const TOUCHPAD_2023_Y_MAX: f64 = 1535.0;
pub const TOUCHPAD_2023_PAD_FORCE_MAX: f64 = 127.0;
pub const TOUCHPAD_2023_PAD_FORCE_NORMAL: u8 = 32;

// GPD Win Mini Touchpad (2024+)
pub const TOUCHPAD_2024_VID: u16 = 0x0911;
pub const TOUCHPAD_2024_PID: u16 = 0x5288;
pub const TOUCHPAD_2024_DEVICE_NAME_PREFIX: &str = "HTIX5288";
pub const TOUCHPAD_2024_X_MAX: f64 = 2628.0;
pub const TOUCHPAD_2024_Y_MAX: f64 = 1332.0;
pub const TOUCHPAD_2024_PAD_FORCE_MAX: f64 = 127.0;
pub const TOUCHPAD_2024_PAD_FORCE_NORMAL: u8 = 32;
