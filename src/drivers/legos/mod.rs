use std::time::Duration;

pub mod event;
pub mod hid_report;
pub mod config_driver;
pub mod imu_driver;
pub mod touchpad_driver;
pub mod xinput_driver;

// Hardware ID's
pub const VID: u16 = 0x1a86;
pub const XINPUT_PID: u16 = 0xe310;
pub const DINPUT_PID: u16 = 0xe311;
pub const PIDS: [u16; 2] = [XINPUT_PID, DINPUT_PID];

pub const CFG_IID: i32 = 0x3;
pub const TP_IID: i32 = 0x2;
pub const IMU_IID: i32 = 0x5;
pub const GP_IID: i32 = 0x6;

// Input report sizes
const INERTIAL_PACKET_SIZE: usize = 9;
const TOUCH_PACKET_SIZE: usize = 10;
const XINPUT_PACKET_SIZE: usize = 32;

// Input report axis ranges
pub const GYRO_SCALE: i16 = 2;
pub const PAD_FORCE_MAX: f64 = 127.0;
pub const PAD_FORCE_NORMAL: u8 = 32; /* Simulated average pressure */
pub const PAD_X_MAX: f64 = 400.0;
pub const PAD_Y_MAX: f64 = 400.0;
pub const STICK_X_MAX: f64 = 127.0;
pub const STICK_X_MIN: f64 = -127.0;
pub const STICK_Y_MAX: f64 = 127.0;
pub const STICK_Y_MIN: f64 = -127.0;
pub const TRIGG_MAX: f64 = 255.0;

// Report ID's
const TOUCH_REPORT_ID: u8 = 0x31;

// Timeouts
const HID_TIMEOUT: i32 = 10;
pub const PAD_RELEASE_DELAY: Duration = Duration::from_millis(25);
