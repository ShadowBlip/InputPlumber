pub mod driver;
pub mod event;
pub mod serial_report;

use packed_struct::prelude::*;

/// Button IDs shared by OXP HID and TTY drivers.
#[derive(PrimitiveEnum_u8, Clone, Copy, PartialEq, Debug, Default)]
pub enum ButtonId {
    #[default]
    None = 0x00,
    A = 0x01,
    B = 0x02,
    X = 0x03,
    Y = 0x04,
    LeftBumper = 0x05,
    RightBumper = 0x06,
    LeftTrigger = 0x07,
    RightTrigger = 0x08,
    Menu = 0x09,
    View = 0x0a,
    LeftStick = 0x0b,
    RightStick = 0x0c,
    DpadUp = 0x0d,
    DpadDown = 0x0e,
    DpadLeft = 0x0f,
    DpadRight = 0x10,
    Guide = 0x21,
    M1 = 0x22,
    M2 = 0x23,
    Keyboard = 0x24,
}

const TAKEOVER_COMMAND: [u8; 64] = [
    0x00, 0xA1, 0x3F, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x3F, 0xA1,
];
pub const TTY_TIMEOUT: u64 = 4;

// Hardware ID's
pub const OXFLY_SERIAL_PORT: u16 = 0x03E8;
pub const X1_VID: u16 = 0x1a86;
pub const X1_PID: u16 = 0x7523;
pub const X1_IF_NO: u16 = 0x00;

// Report ID's
const BUTTON_DATA_REPORT: u8 = 0x1a;
const JOYSTICK_DATA_REPORT: u8 = 0x1b;
const TAKEOVER_DATA_REPORT: u8 = 0xef;

// Input report sizes
const INPUT_REPORT_SIZE: usize = 15;

// Input report axis ranges
pub const STICK_MIN: f64 = -32767.0;
pub const STICK_MAX: f64 = 32767.0;
pub const TRIGG_MAX: f64 = 255.0;

pub enum OxpDriverType {
    Unknown,
    OneXFly,
    OxpX1,
}

/// Returns the [SerialDriverType] after matching on the provided data.
pub fn get_driver_type(port: u16, vid: u16, pid: u16, interface: u16) -> OxpDriverType {
    if port == OXFLY_SERIAL_PORT {
        return OxpDriverType::OneXFly;
    }
    if vid == X1_VID && pid == X1_PID && interface == X1_IF_NO {
        return OxpDriverType::OxpX1;
    }
    OxpDriverType::Unknown
}
