pub mod capability;
pub mod driver;
pub mod event;
pub mod reports;
pub mod value;

/// Unified Gamepad's accelerometer report is +/-2g over a 16-bit signed range.
pub const UNIFIED_GAMEPAD_ACCEL_RAW_TO_MPS2: f64 = 9.80665 / 16384.0;
pub const UNIFIED_GAMEPAD_MPS2_TO_ACCEL_RAW: f64 = 1.0 / UNIFIED_GAMEPAD_ACCEL_RAW_TO_MPS2;
/// Unified Gamepad's gyroscope is +/-2000 deg/s over a 16-bit signed range.
pub const UNIFIED_GAMEPAD_GYRO_RAW_TO_RAD_S: f64 =
    (2000.0 / 32768.0) * (std::f64::consts::PI / 180.0);
pub const UNIFIED_GAMEPAD_RAD_S_TO_GYRO_RAW: f64 = 1.0 / UNIFIED_GAMEPAD_GYRO_RAW_TO_RAD_S;
