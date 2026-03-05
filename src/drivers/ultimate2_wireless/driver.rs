// 8BitDo Ultimate 2 Wireless Controller (DInput mode)

pub const VID: u16 = 0x2DC8;
pub const PID: u16 = 0x6012;

// Axis ranges (0x00-0xFF, sticks centered at 0x7F)
pub const JOY_AXIS_MAX: f64 = 255.0;
pub const JOY_AXIS_MIN: f64 = 0.0;
pub const TRIGGER_AXIS_MAX: f64 = 255.0;

// Accel scale: 4096 raw units = 1G (derived from SDL_hidapi_8bitdo.c)
pub const ACCEL_SCALE: f64 = 4096.0;

pub const REPORT_ID_INPUT: u8 = 0x04;
pub const REPORT_ID_RUMBLE: u8 = 0x05;
