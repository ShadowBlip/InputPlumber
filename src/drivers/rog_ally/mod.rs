use std::time::Duration;

pub mod driver;
pub mod event;
pub mod hid_report;

/// Board names of ROG Ally models with haptic trigger motors
pub const HAPTIC_TRIGGER_BOARD_NAMES: [&str; 2] = ["RC73XA", "RC74XA"];

/// Checks DMI to determine if this machine is a ROG Ally model with haptic
/// trigger motors
pub fn has_haptic_triggers() -> bool {
    let board_name = std::fs::read_to_string("/sys/class/dmi/id/board_name")
        .unwrap_or_default()
        .replace('\n', "");
    HAPTIC_TRIGGER_BOARD_NAMES.contains(&board_name.as_str())
}

// Hardware ID's
pub const VID: u16 = 0x0b05;
const ALLY_PID: u16 = 0x1abe;
const ALLYX_PID: u16 = 0x1b4c;
pub const PIDS: [u16; 2] = [ALLY_PID, ALLYX_PID];

// Report ID's
pub const RUMBLE_REPORT_ID: u8 = 0x0d;

// Timeouts
// Rumble report duration and delay fields are in 10 ms units
pub const GAMEPAD_WRITE_TIMEOUT: Duration = Duration::from_millis(10);
