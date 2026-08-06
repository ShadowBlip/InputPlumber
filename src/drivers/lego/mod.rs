pub mod event;
pub mod go1_driver;
pub mod go2_driver;
pub mod go_touchpad_driver;
pub mod hid_report;

use std::time::Duration;

use crate::input::capability::{Capability, Source};

// Hardware ID's
pub const VID: u16 = 0x17ef;
pub const TP_IID: i32 = 0x01;
pub const GP_IID: i32 = 0x02;

// Go 1
const LEGO_1_XINPUT_PID: u16 = 0x6182;
const LEGO_1_DINPUT_ATTACHED_PID: u16 = 0x6183;
const LEGO_1_DINPUT_DETACHED_PID: u16 = 0x6184;
const LEGO_1_FPS_PID: u16 = 0x6185;

pub const GO1_PIDS: [u16; 4] = [
    LEGO_1_XINPUT_PID,
    LEGO_1_DINPUT_ATTACHED_PID,
    LEGO_1_DINPUT_DETACHED_PID,
    LEGO_1_FPS_PID,
];

// Go 2
const LEGO_2_XINPUT_PID: u16 = 0x61eb;
const LEGO_2_DINPUT_ATTACHED_PID: u16 = 0x61ec;
const LEGO_2_DINPUT_DETACHED_PID: u16 = 0x61ed;
const LEGO_2_FPS_PID: u16 = 0x61ee;

pub const GO2_PIDS: [u16; 4] = [
    LEGO_2_XINPUT_PID,
    LEGO_2_DINPUT_ATTACHED_PID,
    LEGO_2_DINPUT_DETACHED_PID,
    LEGO_2_FPS_PID,
];

pub const GO_TOUCHPAD_D_PIDS: [u16; 4] = [
    LEGO_1_DINPUT_ATTACHED_PID,
    LEGO_1_DINPUT_DETACHED_PID,
    LEGO_2_DINPUT_ATTACHED_PID,
    LEGO_2_DINPUT_DETACHED_PID,
];
pub const GO_TOUCHPAD_X_PIDS: [u16; 2] = [LEGO_1_XINPUT_PID, LEGO_2_XINPUT_PID];

const DRAG_TIMEOUT_DINPUT: Duration = Duration::from_millis(100);
const DRAG_TIMEOUT_XINPUT: Duration = Duration::from_millis(50);
const RELEASE_TIMEOUT_DINPUT: Duration = Duration::from_millis(300);
const RELEASE_TIMEOUT_XINPUT: Duration = Duration::from_millis(165);
const TAP_MAX_DISTANCE_SQ: i64 = 40 * 40;

// Report ID's
pub const TOUCHPAD_DATA: u8 = 0x01;
pub const XINPUT_DATA: u8 = 0x04;

// Input report sizes
const TOUCHPAD_PACKET_SIZE: usize = 20;
const XINPUT_PACKET_SIZE: usize = 60;

const GP_TIMEOUT: i32 = 10;
const TP_TIMEOUT: i32 = 5;

// HID Command ID's
const XINPUT_COMMAND_ID: u8 = 0x74;

// Input report axis ranges
pub const PAD_FORCE_MAX: f64 = 127.0;
pub const PAD_FORCE_NORMAL: u8 = 32; /* Simulated average pressure */
pub const PAD_X_MAX: f64 = 1024.0;
pub const PAD_Y_MAX: f64 = 1024.0;
pub const STICK_X_MAX: f64 = 255.0;
pub const STICK_X_MIN: f64 = 0.0;
pub const STICK_Y_MAX: f64 = 255.0;
pub const STICK_Y_MIN: f64 = 0.0;
pub const TRIGG_MAX: f64 = 255.0;

const DEFAULT_EVENT_FILTER: [Capability; 6] = [
    Capability::Accelerometer(Source::Left),
    Capability::Accelerometer(Source::Right),
    Capability::Accelerometer(Source::Center),
    Capability::Gyroscope(Source::Left),
    Capability::Gyroscope(Source::Right),
    Capability::Gyroscope(Source::Center),
];
