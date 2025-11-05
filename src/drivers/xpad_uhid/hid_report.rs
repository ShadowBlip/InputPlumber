//! Reference: https://gitlab.com/open-sd/opensd/-/blob/main/src/opensdd/drivers/gamepad/hid_reports.hpp
#![allow(warnings)]
use packed_struct::prelude::*;

/// Different report types
pub enum ReportType {
    DInputData = 0x11,
}

impl ReportType {
    pub fn to_u8(&self) -> u8 {
        match self {
            ReportType::DInputData => ReportType::DInputData as u8,
        }
    }
}

//DInput report

//No input
//# ReportID: 11 / X:  32768 | Y:  32768
//#              | Rx:  32768 | Ry:  32768 | Z:     0 | # | Rz:     0 | # | Button: 0  0  0  0  0  0  0  0  0  0  0  0  0  0 | # | Hat switch:   0 | #
//E: 000000.185948 16 0b 00 80 00 80 00 80 00 80 00 00 00 00 00 00 00

//Buttons
//A
//# ReportID: 11 / X:  32768 | Y:  32768
//#              | Rx:  32768 | Ry:  32768 | Z:     0 | # | Rz:     0 | # | Button: 1  0  0  0  0  0  0  0  0  0  0  0  0  0 | # | Hat switch:   0 | #
//E: 000822.252028 16 0b 00 80 00 80 00 80 00 80 00 00 00 00 01 00 00

//B
//# ReportID: 11 / X:  32768 | Y:  32768
//#              | Rx:  32768 | Ry:  32768 | Z:     0 | # | Rz:     0 | # | Button: 0  1  0  0  0  0  0  0  0  0  0  0  0  0 | # | Hat switch:   0 | #
//E: 000869.699748 16 0b 00 80 00 80 00 80 00 80 00 00 00 00 02 00 00

//X
//# ReportID: 11 / X:  32768 | Y:  32768
//#              | Rx:  32768 | Ry:  32768 | Z:     0 | # | Rz:     0 | # | Button: 0  0  1  0  0  0  0  0  0  0  0  0  0  0 | # | Hat switch:   0 | #
//E: 000926.840929 16 0b 00 80 00 80 00 80 00 80 00 00 00 00 04 00 00

//Y
//# ReportID: 11 / X:  32768 | Y:  32768
//#              | Rx:  32768 | Ry:  32768 | Z:     0 | # | Rz:     0 | # | Button: 0  0  0  1  0  0  0  0  0  0  0  0  0  0 | # | Hat switch:   0 | #
//E: 000951.389567 16 0b 00 80 00 80 00 80 00 80 00 00 00 00 08 00 00

//RB
//# ReportID: 11 / X:  32768 | Y:  32768
//#              | Rx:  32768 | Ry:  32768 | Z:     0 | # | Rz:     0 | # | Button: 0  0  0  0  1  0  0  0  0  0  0  0  0  0 | # | Hat switch:   0 | #
//E: 001018.371599 16 0b 00 80 00 80 00 80 00 80 00 00 00 00 10 00 00

//LB
//# ReportID: 11 / X:  32768 | Y:  32768
//#              | Rx:  32768 | Ry:  32768 | Z:     0 | # | Rz:     0 | # | Button: 0  0  0  0  0  1  0  0  0  0  0  0  0  0 | # | Hat switch:   0 | #
//E: 001020.468650 16 0b 00 80 00 80 00 80 00 80 00 00 00 00 20 00 00

//VIEW(Start)
//# ReportID: 11 / X:  32768 | Y:  32768
//#              | Rx:  32768 | Ry:  32768 | Z:     0 | # | Rz:     0 | # | Button: 0  0  0  0  0  0  1  0  0  0  0  0  0  0 | # | Hat switch:   0 | #
//E: 000010.277107 16 0b 00 80 00 80 00 80 00 80 00 00 00 00 40 00 00

//MENU (Select)
//# ReportID: 11 / X:  32768 | Y:  32768
//#              | Rx:  32768 | Ry:  32768 | Z:     0 | # | Rz:     0 | # | Button: 0  0  0  0  0  0  0  1  0  0  0  0  0  0 | # | Hat switch:   0 | #
//E: 000011.117172 16 0b 00 80 00 80 00 80 00 80 00 00 00 00 80 00 00

//LSTICK
//# ReportID: 11 / X:  32768 | Y:  32768
//#              | Rx:  32768 | Ry:  32768 | Z:     0 | # | Rz:     0 | # | Button: 0  0  0  0  0  0  0  0  1  0  0  0  0  0 | # | Hat switch:   0 | #
//E: 000097.965542 16 0b 00 80 00 80 00 80 00 80 00 00 00 00 00 01 00

//RSTICK
//# ReportID: 11 / X:  32768 | Y:  32768
//#              | Rx:  32768 | Ry:  32768 | Z:     0 | # | Rz:     0 | # | Button: 0  0  0  0  0  0  0  0  0  1  0  0  0  0 | # | Hat switch:   0 | #
//E: 000099.092591 16 0b 00 80 00 80 00 80 00 80 00 00 00 00 00 02 00

//D_UP
//# ReportID: 11 / X:  32768 | Y:  32768
//#              | Rx:  32768 | Ry:  32768 | Z:     0 | # | Rz:     0 | # | Button: 0  0  0  0  0  0  0  0  0  0  0  0  0  0 | # | Hat switch:   1 | #
//E: 000173.988783 16 0b 00 80 00 80 00 80 00 80 00 00 00 00 00 00 01

//D_RIGHT
//# ReportID: 11 / X:  32768 | Y:  32768
//#              | Rx:  32768 | Ry:  32768 | Z:     0 | # | Rz:     0 | # | Button: 0  0  0  0  0  0  0  0  0  0  0  0  0  0 | # | Hat switch:   3 | #
//E: 000415.533953 16 0b 00 80 00 80 00 80 00 80 00 00 00 00 00 00 03

//D_DOWN
//# ReportID: 11 / X:  32768 | Y:  32768
//#              | Rx:  32768 | Ry:  32768 | Z:     0 | # | Rz:     0 | # | Button: 0  0  0  0  0  0  0  0  0  0  0  0  0  0 | # | Hat switch:   5 | #
//E: 000363.256465 16 0b 00 80 00 80 00 80 00 80 00 00 00 00 00 00 05

//D_LEFT
//# ReportID: 11 / X:  32768 | Y:  32768
//#              | Rx:  32768 | Ry:  32768 | Z:     0 | # | Rz:     0 | # | Button: 0  0  0  0  0  0  0  0  0  0  0  0  0  0 | # | Hat switch:   7 | #
//E: 000396.549012 16 0b 00 80 00 80 00 80 00 80 00 00 00 00 00 00 07

//Axes
//TRIGGER_LEFT
//# ReportID: 11 / X:  32768 | Y:  32768
//#              | Rx:  32768 | Ry:  32768 | Z:  1023 | # | Rz:     0 | # | Button: 0  0  0  0  0  0  0  0  0  0  0  0  0  0 | # | Hat switch:   0 | #
//E: 000441.163042 16 0b 00 80 00 80 00 80 00 80 ff 03 00 00 00 00 00

//TRIGGER_RIGHT
//# ReportID: 11 / X:  32768 | Y:  32768
//#              | Rx:  32768 | Ry:  32768 | Z:     0 | # | Rz:  1023 | # | Button: 0  0  0  0  0  0  0  0  0  0  0  0  0  0 | # | Hat switch:   0 | #
//E: 000469.060383 16 0b 00 80 00 80 00 80 00 80 00 00 ff 03 00 00 00

//LEFTSTICK_UP
//# ReportID: 11 / X:  32768 | Y:      0
//#              | Rx:  32768 | Ry:  32768 | Z:     0 | # | Rz:     0 | # | Button: 0  0  0  0  0  0  0  0  0  0  0  0  0  0 | # | Hat switch:   0 | #
//E: 000000.091835 16 0b 00 80 00 00 00 80 00 80 00 00 00 00 00 00 00

//LEFTSTICK_DOWN
//# ReportID: 11 / X:  32768 | Y:  65535
//#              | Rx:  32768 | Ry:  32768 | Z:     0 | # | Rz:     0 | # | Button: 0  0  0  0  0  0  0  0  0  0  0  0  0  0 | # | Hat switch:   0 | #
//E: 000000.068929 16 0b 00 80 ff ff 00 80 00 80 00 00 00 00 00 00 00

//LEFTSTICK_LEFT
//# ReportID: 11 / X:      0 | Y:  32768
//#              | Rx:  32768 | Ry:  32768 | Z:     0 | # | Rz:     0 | # | Button: 0  0  0  0  0  0  0  0  0  0  0  0  0  0 | # | Hat switch:   0 | #
//E: 000003.902828 16 0b 00 00 00 80 00 80 00 80 00 00 00 00 00 00 00

//LEFTSTICK_RIGHT
//# ReportID: 11 / X:  65535 | Y:  30464
//#              | Rx:  32768 | Ry:  32768 | Z:     0 | # | Rz:     0 | # | Button: 0  0  0  0  0  0  0  0  0  0  0  0  0  0 | # | Hat switch:   0 | #
//E: 000006.892939 16 0b ff ff 00 77 00 80 00 80 00 00 00 00 00 00 00

//RIGHTSTICK_UP
//# ReportID: 11 / X:  32768 | Y:  32768
//#              | Rx:  32768 | Ry:      0 | Z:     0 | # | Rz:     0 | # | Button: 0  0  0  0  0  0  0  0  0  0  0  0  0  0 | # | Hat switch:   0 | #
//E: 000000.088943 16 0b 00 80 00 80 00 80 00 00 00 00 00 00 00 00 00

//RIGHTSTICK_DOWN
//# ReportID: 11 / X:  32768 | Y:  32768
//#              | Rx:  32768 | Ry:  65535 | Z:     0 | # | Rz:     0 | # | Button: 0  0  0  0  0  0  0  0  0  0  0  0  0  0 | # | Hat switch:   0 | #
//E: 000000.078940 16 0b 00 80 00 80 00 80 ff ff 00 00 00 00 00 00 00

//RIGHTSTICK_LEFT
// ReportID: 11 / X:  32768 | Y:  32768
//#              | Rx:      0 | Ry:  32768 | Z:     0 | # | Rz:     0 | # | Button: 0  0  0  0  0  0  0  0  0  0  0  0  0  0 | # | Hat switch:   0 | #
//E: 000000.228948 16 0b 00 80 00 80 00 00 00 80 00 00 00 00 00 00 00

//RIGHTSTICK_RIGHT
//# ReportID: 11 / X:  32768 | Y:  32768
//#              | Rx:  64512 | Ry:  33792 | Z:     0 | # | Rz:     0 | # | Button: 0  0  0  0  0  0  0  0  0  0  0  0  0  0 | # | Hat switch:   0 | #
//E: 000000.732994 16 0b 00 80 00 80 00 fc 00 84 00 00 00 00 00 00 00

/// DInput Data Report
#[derive(PackedStruct, Debug, Copy, Clone, PartialEq)]
#[packed_struct(bit_numbering = "msb0", size_bytes = "16")]
pub struct DInputDataReport {
    // BYTE 0
    #[packed_field(bytes = "0")]
    pub report_id: u8,

    // Axes
    // BYTES 1-2
    #[packed_field(bytes = "1..=2", endian = "lsb")]
    pub l_stick_x: u16,
    // BYTES 3-4
    #[packed_field(bytes = "3..=4", endian = "lsb")]
    pub l_stick_y: u16,
    // BYTES 5-6
    #[packed_field(bytes = "5..=6", endian = "lsb")]
    pub r_stick_x: u16,
    // BYTES 7-8
    #[packed_field(bytes = "7..=8", endian = "lsb")]
    pub r_stick_y: u16,
    // BYTES 9-10
    #[packed_field(bytes = "9..=10", endian = "lsb")]
    pub trigger_l: u16,
    // BYTES 11-12
    #[packed_field(bytes = "11..=12", endian = "lsb")]
    pub trigger_r: u16,

    // Buttons
    //BYTE 13
    #[packed_field(bits = "104")]
    pub menu: bool,
    #[packed_field(bits = "105")]
    pub view: bool,
    #[packed_field(bits = "106")]
    pub rb: bool,
    #[packed_field(bits = "107")]
    pub lb: bool,
    #[packed_field(bits = "108")]
    pub y: bool,
    #[packed_field(bits = "109")]
    pub x: bool,
    #[packed_field(bits = "110")]
    pub b: bool,
    #[packed_field(bits = "111")]
    pub a: bool,
    //BYTE 14
    #[packed_field(bits = "118")]
    pub thumb_r: bool,
    #[packed_field(bits = "119")]
    pub thumb_l: bool,
    //BYTE 15
    #[packed_field(bytes = "15")]
    pub dpad_state: u8,
}

impl Default for DInputDataReport {
    fn default() -> Self {
        Self {
            report_id: 0x11,
            l_stick_x: Default::default(),
            l_stick_y: Default::default(),
            r_stick_x: Default::default(),
            r_stick_y: Default::default(),
            trigger_l: Default::default(),
            trigger_r: Default::default(),
            a: Default::default(),
            b: Default::default(),
            x: Default::default(),
            y: Default::default(),
            rb: Default::default(),
            lb: Default::default(),
            view: Default::default(),
            menu: Default::default(),
            thumb_l: Default::default(),
            thumb_r: Default::default(),
            dpad_state: Default::default(),
        }
    }
}

#[derive(PackedStruct, Debug, Copy, Clone, PartialEq, Default)]
#[packed_struct(bit_numbering = "msb0", size_bytes = "3")]
pub struct ButtonState {
    // byte 1
    #[packed_field(bits = "0")]
    pub rb: bool,
    #[packed_field(bits = "1")]
    pub lb: bool,
    #[packed_field(bits = "2")]
    pub _unkn2: bool,
    #[packed_field(bits = "3")]
    pub y: bool,
    #[packed_field(bits = "4")]
    pub x: bool,
    #[packed_field(bits = "5")]
    pub _unkn5: bool,
    #[packed_field(bits = "6")]
    pub b: bool,
    #[packed_field(bits = "7")]
    pub a: bool,

    // byte 2 0001 0000
    #[packed_field(bits = "9")]
    pub thumb_r: bool,
    #[packed_field(bits = "10")]
    pub thumb_l: bool,
    #[packed_field(bits = "11")]
    pub guide: bool,
    #[packed_field(bits = "12")]
    pub menu: bool,
    #[packed_field(bits = "13")]
    pub view: bool,

    // byte 3
    #[packed_field(bits = "23")]
    pub share: bool,
}

#[derive(PrimitiveEnum_u8, Clone, Copy, PartialEq, Debug, Default)]
pub enum DPadDirection {
    #[default]
    None = 0,
    Up = 1,
    UpRight = 2,
    Right = 3,
    DownRight = 4,
    Down = 5,
    DownLeft = 6,
    Left = 7,
    UpLeft = 8,
}

#[derive(PackedStruct, Debug, Copy, Clone, PartialEq)]
#[packed_struct(bit_numbering = "msb0", size_bytes = "17")]
pub struct XBoxSeriesInputDataReport {
    // BYTE 0
    #[packed_field(bytes = "0")]
    pub report_id: u8,

    // Axes
    // BYTES 1-2
    #[packed_field(bytes = "1..=2", endian = "lsb")]
    pub l_stick_x: u16,
    // BYTES 3-4
    #[packed_field(bytes = "3..=4", endian = "lsb")]
    pub l_stick_y: u16,
    // BYTES 5-6
    #[packed_field(bytes = "5..=6", endian = "lsb")]
    pub r_stick_x: u16,
    // BYTES 7-8
    #[packed_field(bytes = "7..=8", endian = "lsb")]
    pub r_stick_y: u16,
    // BYTES 9-10
    #[packed_field(bytes = "9..=10", endian = "lsb")]
    pub trigger_l: u16,
    // BYTES 11-12
    #[packed_field(bytes = "11..=12", endian = "lsb")]
    pub trigger_r: u16,

    #[packed_field(bytes = "13", ty = "enum")]
    pub dpad_state: DPadDirection,

    #[packed_field(bytes = "14..=16")]
    pub button_state: ButtonState,
}

impl Default for XBoxSeriesInputDataReport {
    fn default() -> Self {
        Self {
            report_id: 0x01,
            l_stick_x: Default::default(),
            l_stick_y: Default::default(),
            r_stick_x: Default::default(),
            r_stick_y: Default::default(),
            trigger_l: Default::default(),
            trigger_r: Default::default(),
            button_state: Default::default(),
            dpad_state: Default::default(),
        }
    }
}

/// All possible output reports that can be written
pub enum OutputReport {
    XboxOneRumble(XboxOneForceFeedbackOutputReport),
}

#[derive(PrimitiveEnum_u8, Clone, Copy, PartialEq, Debug, Default)]
pub enum OutputReportType {
    #[default]
    ForceFeedback = 0x09,
}

impl OutputReportType {
    pub fn to_u8(&self) -> u8 {
        match self {
            Self::ForceFeedback => Self::ForceFeedback as u8,
        }
    }
}

/// Rumble state output report for Xbox One gamepads
/// Reference: https://github.com/paroj/xpad/blob/master/xpad.c#L1830-L1846
/// Reference: https://github.com/paroj/xpad/blob/master/xpad.c#L716-L734
/// Reference: https://github.com/quantus/xbox-one-controller-protocol?tab=readme-ov-file#0x09-activate-rumble
#[derive(PackedStruct, Debug, Copy, Clone, PartialEq)]
#[packed_struct(bit_numbering = "msb0", size_bytes = "13")]
pub struct XboxOneForceFeedbackOutputReport {
    /// Report ID
    #[packed_field(bytes = "0", ty = "enum")]
    pub report_id: OutputReportType,
    /// Counter that should be incremented with each force feedback request
    #[packed_field(bytes = "2")]
    pub frame: u8,
    /// Length of the command payload encoded with LEB128
    #[packed_field(bytes = "3")]
    pub command_payload_length: u8,
    /// Motors to activate for rumble
    #[packed_field(bits = "47")]
    pub motor_right_enabled: bool,
    #[packed_field(bits = "46")]
    pub motor_left_enabled: bool,
    #[packed_field(bits = "45")]
    pub motor_right_trigger_enabled: bool,
    #[packed_field(bits = "44")]
    pub motor_left_trigger_enabled: bool,
    /// Left trigger actuator force
    #[packed_field(bytes = "6")]
    pub left_trigger_force: u8,
    /// Right trigger actuator force
    #[packed_field(bytes = "7")]
    pub right_trigger_force: u8,
    /// Left actuator force
    #[packed_field(bytes = "8")]
    pub left_force: u8,
    /// Right actuator force
    #[packed_field(bytes = "9")]
    pub right_force: u8,
    /// On period duration
    #[packed_field(bytes = "10")]
    duration: u8,
    /// Start delay duration
    #[packed_field(bytes = "11")]
    start_delay: u8,
    /// Repeat count
    #[packed_field(bytes = "12")]
    loop_count: u8,
}

impl XboxOneForceFeedbackOutputReport {
    pub fn new(frame: u8, left_force: u8, right_force: u8) -> Self {
        Self {
            frame,
            left_force,
            right_force,
            ..Default::default()
        }
    }
}

impl Default for XboxOneForceFeedbackOutputReport {
    fn default() -> Self {
        Self {
            report_id: OutputReportType::ForceFeedback,
            frame: 0,
            command_payload_length: 0x09,
            motor_right_enabled: true,
            motor_left_enabled: true,
            motor_right_trigger_enabled: true,
            motor_left_trigger_enabled: true,
            left_trigger_force: 0,
            right_trigger_force: 0,
            left_force: u8::MAX,
            right_force: u8::MAX,
            duration: u8::MAX,
            start_delay: 0,
            loop_count: u8::MAX,
        }
    }
}
