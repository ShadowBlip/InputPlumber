use std::f64::consts::PI;

pub mod driver;
pub mod event;
pub mod hid_report;
pub mod report_descriptor;

// 8BitDo Ultimate 2 Wireless Controller (DInput mode)
pub const VID: u16 = 0x2DC8;
pub const PID: u16 = 0x6012;

// Axis ranges (0x00-0xFF, sticks centered at 0x7F)
pub const JOY_AXIS_MAX: f64 = 255.0;
pub const JOY_AXIS_MIN: f64 = 0.0;
pub const TRIGGER_AXIS_MAX: f64 = 255.0;

pub const GRAVITY_MPS2: f64 = 9.80665;

// Accel scale: 4096 raw units = 1G (derived from SDL_hidapi_8bitdo.c)
pub const ACCEL_RAW_PER_G: f64 = 4096.0;
pub const ULTIMATE_2_MPS2_TO_ACCEL_RAW: f64 = ACCEL_RAW_PER_G / GRAVITY_MPS2;
pub const ULTIMATE_2_ACCEL_RAW_TO_MPS2: f64 = 1.0 / ULTIMATE_2_MPS2_TO_ACCEL_RAW;

// Gyro range: raw i16 full scale (INT16_MAX) maps to +/-2000 degrees/second
// (derived from SDL_hidapi_8bitdo.c: gyroScale = DEG2RAD(2000) / INT16_MAX)
pub const GYRO_MAX_DEGREES_PER_SECOND: f64 = 2000.0;
pub const GYRO_MAX_RADIANS_PER_SECOND: f64 = GYRO_MAX_DEGREES_PER_SECOND * (PI / 180.0);
pub const ULTIMATE_2_RAD_S_TO_GYRO_RAW: f64 = i16::MAX as f64 / GYRO_MAX_RADIANS_PER_SECOND;
pub const ULTIMATE_2_GYRO_RAW_TO_RAD_S: f64 = 1.0 / ULTIMATE_2_RAD_S_TO_GYRO_RAW;

pub const REPORT_ID_INPUT: u8 = 0x01;
pub const REPORT_ID_RUMBLE: u8 = 0x05;
