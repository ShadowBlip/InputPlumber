pub mod driver;
pub mod event;
pub mod hid_report;
#[cfg(test)]
pub mod hid_report_test;

/// Vendor and product reported by the controller's Bluetooth HID interface.
///
/// Apple MFi accessories do not expose a real USB VID/PID over Bluetooth;
/// Linux reports the generic `0005:0103:0103` identity for this pad. The
/// device name is used to disambiguate it from other MFi controllers.
pub const VID: u16 = 0x0103;
pub const PID: u16 = 0x0103;
pub const DEVICE_NAME: &str = "MOGA XP5-i Plus";

/// Input report size in bytes. The report has no report ID:
/// four stick axes, two trigger bytes, and two button bytes.
pub const PACKET_SIZE: usize = 8;

/// HID buffer read timeout in milliseconds.
pub const HID_TIMEOUT: i32 = 4;

/// Raw joystick range from the report descriptor (signed int8).
pub const JOY_AXIS_MIN: f64 = -127.0;
pub const JOY_AXIS_MAX: f64 = 127.0;

/// Raw trigger range from the report descriptor (unsigned int8).
pub const TRIGGER_AXIS_MAX: f64 = 255.0;

/// Deadzone for the sticks, in raw units. This unit's left stick idles with
/// occasional excursions up to ~14 raw units, so values at or below this are
/// treated as centered and the remaining range is rescaled to full scale.
pub const STICK_DEADZONE: i8 = 16;

/// Deadzone for the triggers, in raw units. On this unit RT idles between
/// 3 and 18 and LT flickers between 0 and 1, so small values are sensor
/// noise rather than input.
pub const TRIGGER_DEADZONE: u8 = 24;
