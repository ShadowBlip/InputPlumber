pub mod driver;
pub mod event;
pub mod serial_report;

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
