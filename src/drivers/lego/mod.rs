pub mod event;
pub mod hid_report;
pub mod touchpad_driver;
pub mod xinput_driver;

// Hardware ID's
const LEGO_1_XINPUT_PID: u16 = 0x6182;
const LEGO_1_DINPUT_ATTACHED_PID: u16 = 0x6183;
const LEGO_1_DINPUT_DETACHED_PID: u16 = 0x6184;
const LEGO_1_FPS_PID: u16 = 0x6185;
const LEGO_2_XINPUT_PID: u16 = 0x61eb;
const LEGO_2_DINPUT_ATTACHED_PID: u16 = 0x61ec;
const LEGO_2_DINPUT_DETATCHED_PID: u16 = 0x61ed;
const LEGO_2_FPS_PID: u16 = 0x61ee;

pub const PIDS: [u16; 8] = [
    LEGO_1_XINPUT_PID,
    LEGO_1_DINPUT_ATTACHED_PID,
    LEGO_1_DINPUT_DETACHED_PID,
    LEGO_1_FPS_PID,
    LEGO_2_XINPUT_PID,
    LEGO_2_DINPUT_ATTACHED_PID,
    LEGO_2_DINPUT_DETATCHED_PID,
    LEGO_2_FPS_PID,
];

pub const TP_IID: i32 = 0x01;
pub const GP_IID: i32 = 0x02;

// Report ID's
pub const TOUCHPAD_DATA: u8 = 0x01;
pub const XINPUT_DATA: u8 = 0x04;

// Input report sizes
const XINPUT_PACKET_SIZE: usize = 60;
const TOUCHPAD_PACKET_SIZE: usize = 20;
const HID_TIMEOUT: i32 = 10;

// Input report axis ranges
pub const MOUSE_WHEEL_MAX: f64 = 120.0;
pub const PAD_X_MAX: f64 = 1024.0;
pub const PAD_Y_MAX: f64 = 1024.0;
pub const STICK_X_MAX: f64 = 255.0;
pub const STICK_X_MIN: f64 = 0.0;
pub const STICK_Y_MAX: f64 = 255.0;
pub const STICK_Y_MIN: f64 = 0.0;
pub const TRIGG_MAX: f64 = 255.0;

pub const VID: u16 = 0x17ef;
