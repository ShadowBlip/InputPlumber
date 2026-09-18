pub mod driver;
pub mod event;
pub mod hid_report;
pub mod report_descriptor;

// Flydigi Vader 4 Pro IMU hardware limits
// Source: https://github.com/libsdl-org/SDL/pull/12874 (SDL_hidapi_flydigi.c)
// Accelerometer: 256 raw units = 1g
pub const VADER_ACCEL_RAW_TO_MPS2: f64 = 9.80665 / 256.0;
// Gyro X/Y axes report over a +/-65536 deg/s range, the Z axis over a
// +/-1024 deg/s range
pub const VADER_GYRO_XY_RAW_TO_RAD_S: f64 =
    65536.0 * std::f64::consts::PI / 180.0 / i16::MAX as f64;
pub const VADER_GYRO_Z_RAW_TO_RAD_S: f64 =
    1024.0 * std::f64::consts::PI / 180.0 / i16::MAX as f64;
