pub mod driver;
pub mod event;
pub mod hid_report;

use std::time::Duration;

use crate::input::capability::{Capability, Source};

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
pub const VID: u16 = 0x17ef;
pub const GP_IID: i32 = 0x02;

const CLICK_DELAY: Duration = Duration::from_millis(150);
const RELEASE_DELAY: Duration = Duration::from_millis(30);

// Report ID's
pub const XINPUT_DATA: u8 = 0x04;

// Input report sizes
const XINPUT_PACKET_SIZE: usize = 60;
const HID_TIMEOUT: i32 = 10;

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
