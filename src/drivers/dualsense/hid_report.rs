//! Structures derived from the great work of the community of the Game Controller
//! Collective Wiki.
//! Source: https://controllers.fandom.com/wiki/Sony_DualSense
use std::{error::Error, fmt::Display};

use packed_struct::prelude::*;

use super::driver::*;

/// DualSense input report for USB and Bluetooth
#[derive(Debug, Copy, Clone)]
pub enum PackedInputDataReport {
    Usb(USBPackedInputDataReport),
    Bluetooth(BluetoothPackedInputDataReport),
}

impl PackedInputDataReport {
    pub fn unpack(buf: &[u8], size: usize) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let report_id = buf[0];
        match report_id {
            INPUT_REPORT_USB => {
                // Validate the size of the report
                if size != INPUT_REPORT_USB_SIZE {
                    let err = format!(
                        "Invalid report size for USB: Expected {INPUT_REPORT_USB_SIZE}, Got {size}"
                    );
                    return Err(err.into());
                }
                log::trace!("Got USB input report");

                // Get a subslice of the buffer
                let buffer = buf.try_into()?;
                let data = USBPackedInputDataReport::unpack(buffer)?;
                Ok(Self::Usb(data))
            }
            INPUT_REPORT_BT => {
                // Validate the size of the report
                if size != INPUT_REPORT_BT_SIZE {
                    let err = format!(
                        "Invalid report size for BT: Expected {INPUT_REPORT_BT_SIZE}, Got {size}"
                    );
                    return Err(err.into());
                }
                log::trace!("Got Bluetooth input report");

                // Get a subslice of the buffer
                let buffer = buf.try_into()?;
                let data = BluetoothPackedInputDataReport::unpack(buffer)?;
                Ok(Self::Bluetooth(data))
            }
            _ => Err(format!("Invalid report id: {report_id}").into()),
        }
    }

    /// Return the underlying input report. Both USB and Bluetooth implementations
    /// share the same USB input state.
    pub fn state(&self) -> &InputState {
        match self {
            PackedInputDataReport::Usb(report) => &report.state,
            PackedInputDataReport::Bluetooth(report) => &report.state,
        }
    }

    /// Return a mutable reference to the underlying input report. Both USB and
    /// Bluetooth implementations share the same USB input state.
    pub fn state_mut(&mut self) -> &mut InputState {
        match self {
            PackedInputDataReport::Usb(ref mut report) => &mut report.state,
            PackedInputDataReport::Bluetooth(ref mut report) => &mut report.state,
        }
    }
}

impl Display for PackedInputDataReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PackedInputDataReport::Usb(data) => {
                write!(f, "{}", data)
            }
            PackedInputDataReport::Bluetooth(data) => {
                write!(f, "{}", data)
            }
        }
    }
}

#[derive(PrimitiveEnum_u8, Clone, Copy, PartialEq, Debug, Default)]
pub enum Direction {
    North = 0,
    NorthEast = 1,
    East = 2,
    SouthEast = 3,
    South = 4,
    SouthWest = 5,
    West = 6,
    NorthWest = 7,
    #[default]
    None = 8,
}

impl Direction {
    pub fn as_bitflag(&self) -> u8 {
        match *self {
            Self::North => 1,                   // 00000001
            Self::NorthEast => 1 | 1 << 1,      // 00000011
            Self::East => 1 << 1,               // 00000010
            Self::SouthEast => 1 << 2 | 1 << 1, // 00000110
            Self::South => 1 << 2,              // 00000100
            Self::SouthWest => 1 << 2 | 1 << 3, // 00001100
            Self::West => 1 << 3,               // 00001000
            Self::NorthWest => 1 | 1 << 3,      // 00001001
            Self::None => 0,                    // 00000000
        }
    }
}

#[derive(PrimitiveEnum_u8, Clone, Copy, PartialEq, Debug, Default)]
pub enum PowerState {
    Disharging = 0x00,
    Charging = 0x01,
    #[default]
    Complete = 0x02,
    AbnormalVoltage = 0x0A,
    AbnormalTemperature = 0x0B,
    ChargingError = 0x0F,
}

#[derive(PackedStruct, Debug, Copy, Clone, PartialEq)]
#[packed_struct(bit_numbering = "msb0", size_bytes = "4")]
pub struct TouchFingerData {
    // byte 0
    // Set to 127 when touching and 128 when not.
    #[packed_field(bytes = "0")]
    pub context: u8,
    // byte 1
    #[packed_field(bytes = "1")]
    pub x_lo: u8,
    // byte 2
    #[packed_field(bits = "16..=19")]
    pub y_lo: Integer<u8, packed_bits::Bits<4>>,
    #[packed_field(bits = "20..=23")]
    pub x_hi: Integer<u8, packed_bits::Bits<4>>,
    // byte 3
    #[packed_field(bytes = "3")]
    pub y_hi: u8,
}

impl Default for TouchFingerData {
    fn default() -> Self {
        Self {
            context: 128,
            x_lo: Default::default(),
            y_lo: Default::default(),
            x_hi: Default::default(),
            y_hi: Default::default(),
        }
    }
}

impl TouchFingerData {
    pub fn is_touching(&self) -> bool {
        self.context != 128
    }

    pub fn get_x(&self) -> u16 {
        let x_hi = self.x_hi.to_primitive() as u16;
        let x_hi = x_hi.rotate_left(8);
        x_hi | self.x_lo as u16
    }

    pub fn get_y(&self) -> u16 {
        let y_lo = self.y_lo.to_primitive() as u16;
        let y_hi = (self.y_hi as u16).rotate_left(4);
        y_hi | y_lo
    }

    pub fn set_x(&mut self, x_raw: u16) {
        self.x_lo = (x_raw & 0x00FF) as u8;
        self.x_hi = Integer::from_primitive((x_raw & 0x0F00).rotate_right(8) as u8);
    }

    pub fn set_y(&mut self, y_raw: u16) {
        self.y_lo = Integer::from_primitive((y_raw & 0x000F) as u8);
        self.y_hi = (y_raw & 0x0FF0).rotate_right(4) as u8;
    }
}

#[derive(PackedStruct, Debug, Copy, Clone, PartialEq, Default)]
#[packed_struct(bit_numbering = "msb0", size_bytes = "9")]
pub struct TouchData {
    #[packed_field(element_size_bytes = "4")]
    pub touch_finger_data: [TouchFingerData; 2],
    pub timestamp: u8,
}

impl TouchData {
    /// Returns true if any touches are detected
    pub fn has_touches(&self) -> bool {
        self.touch_finger_data[0].is_touching() || self.touch_finger_data[1].is_touching()
    }
}

#[derive(PackedStruct, Debug, Copy, Clone, PartialEq)]
#[packed_struct(bit_numbering = "msb0", size_bytes = "63")]
pub struct InputState {
    // byte 0-6
    #[packed_field(bytes = "0")]
    pub joystick_l_x: u8, // left stick X axis
    #[packed_field(bytes = "1")]
    pub joystick_l_y: u8, // left stick Y axis
    #[packed_field(bytes = "2")]
    pub joystick_r_x: u8, // right stick X axis
    #[packed_field(bytes = "3")]
    pub joystick_r_y: u8, // right stick Y axis
    #[packed_field(bytes = "4")]
    pub l2_trigger: u8, // L2 trigger axis
    #[packed_field(bytes = "5")]
    pub r2_trigger: u8, // R2 trigger axis
    #[packed_field(bytes = "6")]
    pub seq_number: u8, // Sequence number, always 0x01 on BT

    // byte 7
    #[packed_field(bits = "56")]
    pub triangle: bool, // Button cluster, x, ◯, □, ∆
    #[packed_field(bits = "57")]
    pub circle: bool,
    #[packed_field(bits = "58")]
    pub cross: bool,
    #[packed_field(bits = "59")]
    pub square: bool,
    #[packed_field(bits = "60..=63", ty = "enum")]
    pub dpad: Direction, // Directional buttons

    // byte 8
    #[packed_field(bits = "64")]
    pub r3: bool,
    #[packed_field(bits = "65")]
    pub l3: bool,
    #[packed_field(bits = "66")]
    pub options: bool, // Options button ☰
    #[packed_field(bits = "67")]
    pub create: bool, // Create button ⚟
    #[packed_field(bits = "68")]
    pub r2: bool, // Triggers
    #[packed_field(bits = "69")]
    pub l2: bool,
    #[packed_field(bits = "70")]
    pub r1: bool,
    #[packed_field(bits = "71")]
    pub l1: bool,

    // byte 9
    #[packed_field(bits = "72")]
    pub right_paddle: bool, // Right paddle button (DualSense Edge)
    #[packed_field(bits = "73")]
    pub left_paddle: bool, // Left paddle button (DualSense Edge)
    #[packed_field(bits = "74")]
    pub right_fn: bool, // Right function button (DualSense Edge)
    #[packed_field(bits = "75")]
    pub left_fn: bool, // Left function button (DualSense Edge)
    #[packed_field(bits = "76")]
    pub _unkn_0: bool, // Appears unused
    #[packed_field(bits = "77")]
    pub mute: bool, // Mic mute button 🔇
    #[packed_field(bits = "78")]
    pub touchpad: bool, // Touchpad button
    #[packed_field(bits = "79")]
    pub ps: bool, // PS button

    // byte 10
    #[packed_field(bytes = "10")]
    pub _unkn_1: u8, // Appears unused

    // byte 11-14
    #[packed_field(bytes = "11..=14", endian = "lsb")]
    pub _unkn_counter: Integer<u32, packed_bits::Bits<32>>, // Linux driver calls this reserved

    // byte 15-26
    #[packed_field(bytes = "15..=16", endian = "lsb")]
    pub gyro_x: Integer<i16, packed_bits::Bits<16>>, // Gyro
    #[packed_field(bytes = "17..=18", endian = "lsb")]
    pub gyro_y: Integer<i16, packed_bits::Bits<16>>,
    #[packed_field(bytes = "19..=20", endian = "lsb")]
    pub gyro_z: Integer<i16, packed_bits::Bits<16>>,
    #[packed_field(bytes = "21..=22", endian = "lsb")]
    pub accel_x: Integer<i16, packed_bits::Bits<16>>, // Accelerometer
    #[packed_field(bytes = "23..=24", endian = "lsb")]
    pub accel_y: Integer<i16, packed_bits::Bits<16>>,
    #[packed_field(bytes = "25..=26", endian = "lsb")]
    pub accel_z: Integer<i16, packed_bits::Bits<16>>,

    // byte 27
    #[packed_field(bytes = "27..=30", endian = "lsb")]
    pub sensor_timestamp: Integer<u32, packed_bits::Bits<32>>,
    #[packed_field(bytes = "31")]
    pub temperature: u8, // reserved2 in Linux driver

    // byte 32-40
    #[packed_field(bytes = "32..=40")]
    pub touch_data: TouchData,

    // byte 41-42
    #[packed_field(bits = "328..=331", endian = "lsb")]
    pub trigger_left_status: Integer<u8, packed_bits::Bits<4>>,
    #[packed_field(bits = "332..=335", endian = "lsb")]
    pub trigger_left_stop_location: Integer<u8, packed_bits::Bits<4>>, // Can range from 0-9
    #[packed_field(bits = "336..=339", endian = "lsb")]
    pub trigger_right_status: Integer<u8, packed_bits::Bits<4>>,
    #[packed_field(bits = "340..=343", endian = "lsb")]
    pub trigger_right_stop_location: Integer<u8, packed_bits::Bits<4>>, // Can range from 0-9

    // byte 43-46
    #[packed_field(bytes = "43..=46", endian = "lsb")]
    pub host_timestamp: Integer<u32, packed_bits::Bits<32>>, // Mirrors data from report write

    // byte 47
    #[packed_field(bits = "376..=379", endian = "lsb")]
    pub trigger_left_effect: Integer<u8, packed_bits::Bits<4>>,
    #[packed_field(bits = "380..=383", endian = "lsb")]
    pub trigger_right_effect: Integer<u8, packed_bits::Bits<4>>,

    // byte 48-51
    #[packed_field(bytes = "48..=51", endian = "lsb")]
    pub device_timestamp: Integer<u32, packed_bits::Bits<32>>,

    // byte 52
    #[packed_field(bits = "416..=419", ty = "enum")]
    pub power_state: PowerState,
    #[packed_field(bits = "420..=423", endian = "lsb")]
    pub power_percent: Integer<u8, packed_bits::Bits<4>>, // 0x00 - 0x0A

    // byte 53
    #[packed_field(bits = "424..426", endian = "lsb")]
    pub _plugged_unkn_0: Integer<u8, packed_bits::Bits<3>>,
    #[packed_field(bits = "427")]
    pub plugged_usb_power: bool,
    #[packed_field(bits = "428")]
    pub plugged_usb_data: bool,
    #[packed_field(bits = "429")]
    pub mic_mutes: bool,
    #[packed_field(bits = "430")]
    pub plugged_mic: bool,
    #[packed_field(bits = "431")]
    pub plugged_headphones: bool,

    // byte 54
    #[packed_field(bits = "432..=437", endian = "lsb")]
    pub _plugged_unkn_1: Integer<u8, packed_bits::Bits<6>>,
    #[packed_field(bits = "438")]
    pub haptic_low_pass_filter: bool,
    #[packed_field(bits = "439")]
    pub plugged_external_mic: bool,

    // byte 55-63
    #[packed_field(bytes = "55..=62")]
    pub aes_cmac: [u8; 8],
}

impl Default for InputState {
    fn default() -> Self {
        Self {
            joystick_l_x: 127,
            joystick_l_y: 127,
            joystick_r_x: 127,
            joystick_r_y: 127,
            power_percent: Integer::from_primitive(100),
            plugged_usb_data: true,
            plugged_usb_power: true,
            l2_trigger: Default::default(),
            r2_trigger: Default::default(),
            seq_number: Default::default(),
            triangle: Default::default(),
            circle: Default::default(),
            cross: Default::default(),
            square: Default::default(),
            dpad: Default::default(),
            r3: Default::default(),
            l3: Default::default(),
            options: Default::default(),
            create: Default::default(),
            r2: Default::default(),
            l2: Default::default(),
            r1: Default::default(),
            l1: Default::default(),
            right_paddle: Default::default(),
            left_paddle: Default::default(),
            right_fn: Default::default(),
            left_fn: Default::default(),
            _unkn_0: Default::default(),
            mute: Default::default(),
            touchpad: Default::default(),
            ps: Default::default(),
            _unkn_1: Default::default(),
            _unkn_counter: Default::default(),
            gyro_x: Default::default(),
            gyro_y: Default::default(),
            gyro_z: Default::default(),
            accel_x: Default::default(),
            accel_y: Default::default(),
            accel_z: Default::default(),
            sensor_timestamp: Default::default(),
            temperature: Default::default(),
            touch_data: Default::default(),
            trigger_left_status: Default::default(),
            trigger_left_stop_location: Default::default(),
            trigger_right_status: Default::default(),
            trigger_right_stop_location: Default::default(),
            host_timestamp: Default::default(),
            trigger_left_effect: Default::default(),
            trigger_right_effect: Default::default(),
            device_timestamp: Default::default(),
            power_state: Default::default(),
            _plugged_unkn_0: Default::default(),
            mic_mutes: Default::default(),
            plugged_mic: Default::default(),
            plugged_headphones: Default::default(),
            _plugged_unkn_1: Default::default(),
            haptic_low_pass_filter: Default::default(),
            plugged_external_mic: Default::default(),
            aes_cmac: Default::default(),
        }
    }
}

#[derive(PackedStruct, Debug, Copy, Clone, PartialEq)]
#[packed_struct(bit_numbering = "msb0", size_bytes = "64")]
pub struct USBPackedInputDataReport {
    // byte 0
    #[packed_field(bytes = "0")]
    pub report_id: u8, // Report ID (always 0x01)

    // byte 1-64
    #[packed_field(bytes = "1..=63")]
    pub state: InputState,
}

impl USBPackedInputDataReport {
    /// Return a new empty input data report
    pub fn new() -> Self {
        Self::default()
    }
}

impl Default for USBPackedInputDataReport {
    fn default() -> Self {
        Self {
            report_id: INPUT_REPORT_USB,
            state: Default::default(),
        }
    }
}

#[derive(PackedStruct, Debug, Copy, Clone, PartialEq)]
#[packed_struct(bit_numbering = "msb0", size_bytes = "10")]
pub struct BluetoothSimplePackedInputDataReport {
    // byte 0
    #[packed_field(bytes = "0")]
    pub report_id: u8, // Report ID (always 0x01)

    // byte 1-4
    #[packed_field(bytes = "1")]
    pub joystick_l_x: u8, // left stick X axis
    #[packed_field(bytes = "2")]
    pub joystick_l_y: u8, // left stick Y axis
    #[packed_field(bytes = "3")]
    pub joystick_r_x: u8, // right stick X axis
    #[packed_field(bytes = "4")]
    pub joystick_r_y: u8, // right stick Y axis

    // byte 5
    #[packed_field(bits = "40")]
    pub triangle: bool, // Button cluster, x, ◯, □, ∆
    #[packed_field(bits = "41")]
    pub circle: bool,
    #[packed_field(bits = "42")]
    pub cross: bool,
    #[packed_field(bits = "43")]
    pub square: bool,
    #[packed_field(bits = "44..=47", ty = "enum")]
    pub dpad: Direction, // Directional buttons

    // byte 6
    #[packed_field(bits = "48")]
    pub l1: bool, // Triggers
    #[packed_field(bits = "49")]
    pub r1: bool,
    #[packed_field(bits = "50")]
    pub l2: bool,
    #[packed_field(bits = "51")]
    pub r2: bool,
    #[packed_field(bits = "52")]
    pub create: bool, // Create button ⚟
    #[packed_field(bits = "53")]
    pub options: bool, // Options button ☰
    #[packed_field(bits = "54")]
    pub l3: bool,
    #[packed_field(bits = "55")]
    pub r3: bool,

    // byte 7
    #[packed_field(bits = "56")]
    pub ps: bool, // PS button
    #[packed_field(bits = "57")]
    pub touchpad: bool, // Touchpad button
    #[packed_field(bits = "58..=63")]
    pub counter: [u8; 6],

    // byte 8-9
    #[packed_field(bytes = "8")]
    pub l2_trigger: u8, // L2 trigger axis
    #[packed_field(bytes = "9")]
    pub r2_trigger: u8, // R2 trigger axis
}

#[derive(PackedStruct, Debug, Copy, Clone, PartialEq)]
#[packed_struct(bit_numbering = "msb0", size_bytes = "78")]
pub struct BluetoothPackedInputDataReport {
    // byte 0
    #[packed_field(bytes = "0")]
    pub report_id: u8, // Report ID (always 0x31)

    // byte 1
    #[packed_field(bits = "8..=11", endian = "lsb")]
    pub seq_number: Integer<u8, packed_bits::Bits<4>>,
    #[packed_field(bits = "12..=13", endian = "lsb")]
    pub _unkn_0: Integer<u8, packed_bits::Bits<2>>,
    /// Present for mic data
    #[packed_field(bits = "14")]
    pub has_mic: bool,
    /// Present for packets with state data
    #[packed_field(bits = "15")]
    pub has_hid: bool,

    // byte 2-65
    #[packed_field(bytes = "2..=64")]
    pub state: InputState,

    // byte 66
    #[packed_field(bytes = "65")]
    pub _unkn_1: u8,

    // byte 67
    #[packed_field(bytes = "66")]
    pub bt_crc_fail_count: u8,
}

impl Default for BluetoothPackedInputDataReport {
    fn default() -> Self {
        Self {
            report_id: INPUT_REPORT_BT,
            has_hid: true,
            has_mic: false,
            _unkn_0: Default::default(),
            seq_number: Default::default(),
            state: InputState::default(),
            _unkn_1: 0,
            bt_crc_fail_count: Default::default(),
        }
    }
}

#[derive(PrimitiveEnum_u8, Clone, Copy, PartialEq, Debug, Default)]
pub enum MuteLight {
    #[default]
    Off = 0,
    On = 1,
    Breathing = 2,
    DoNothing = 3,
    NoAction4 = 4,
    NoAction5 = 5,
    NoAction6 = 6,
    NoAction7 = 7,
}

#[derive(PrimitiveEnum_u8, Clone, Copy, PartialEq, Debug, Default)]
pub enum LightFadeAnimation {
    #[default]
    Nothing = 0,
    FadeIn = 1,
    FadeOut = 2,
}

#[derive(PrimitiveEnum_u8, Clone, Copy, PartialEq, Debug, Default)]
pub enum LightBrightness {
    #[default]
    Bright = 0,
    Mid = 1,
    Dim = 2,
    NoAction3 = 3,
    NoAction4 = 4,
    NoAction5 = 5,
    NoAction6 = 6,
    NoAction7 = 7,
}

/// State data can be emitted from Output events to change data such as LED
/// colors.
#[derive(PackedStruct, Debug, Copy, Clone, PartialEq, Default)]
#[packed_struct(bit_numbering = "msb0", size_bytes = "47")]
pub struct SetStatePackedOutputData {
    // byte 0
    #[packed_field(bits = "0")]
    pub allow_audio_control: bool,
    #[packed_field(bits = "1")]
    pub allow_mic_volume: bool,
    #[packed_field(bits = "2")]
    pub allow_speaker_volume: bool,
    #[packed_field(bits = "3")]
    pub allow_headphone_volume: bool,
    #[packed_field(bits = "4")]
    pub allow_left_trigger_ffb: bool,
    #[packed_field(bits = "5")]
    pub allow_right_trigger_ffb: bool,
    #[packed_field(bits = "6")]
    pub use_rumble_not_haptics: bool,
    #[packed_field(bits = "7")]
    pub enable_rumble_emulation: bool,

    // byte 1
    #[packed_field(bits = "8")]
    pub allow_audio_control2: bool,
    #[packed_field(bits = "9")]
    pub allow_motor_power_level: bool,
    #[packed_field(bits = "10")]
    pub allow_haptic_low_pass_filter: bool,
    #[packed_field(bits = "11")]
    pub allow_player_indicators: bool,
    #[packed_field(bits = "12")]
    pub reset_lights: bool,
    #[packed_field(bits = "13")]
    pub allow_led_color: bool, // Enable RGB LED section
    #[packed_field(bits = "14")]
    pub allow_audio_mute: bool, // Enable setting MuteControl
    #[packed_field(bits = "15")]
    pub allow_mute_light: bool, // Enable setting MuteLightMode

    // byte 2-6
    #[packed_field(bytes = "2")]
    pub rumble_emulation_right: u8,
    #[packed_field(bytes = "3")]
    pub rumble_emulation_left: u8,
    #[packed_field(bytes = "4")]
    pub volume_headphones: u8,
    #[packed_field(bytes = "5")]
    pub volume_speakers: u8,
    #[packed_field(bytes = "6")]
    pub volume_mic: u8,

    // byte 7
    #[packed_field(bits = "56..=57", endian = "lsb")]
    pub input_path_select: Integer<u8, packed_bits::Bits<2>>, // 0 CHAT ASR, 1 CHAT_CHAT, 2 ASR_ASR
    #[packed_field(bits = "58..=59", endian = "lsb")]
    pub output_path_select: Integer<u8, packed_bits::Bits<2>>, // 0 L_R_X, 1 L_L_X, 2 L_L_R, 3 X_X_R
    #[packed_field(bits = "60")]
    pub noise_cancel_enable: bool,
    #[packed_field(bits = "61")]
    pub echo_cancel_enable: bool,
    #[packed_field(bits = "62..=63", endian = "lsb")]
    pub mic_select: Integer<u8, packed_bits::Bits<2>>, // 0 auto, 1 internal, 2 external

    // byte 8
    #[packed_field(bytes = "8", ty = "enum")]
    pub mute_light_mode: MuteLight,

    // byte 9
    #[packed_field(bits = "72")]
    pub haptic_mute: bool,
    #[packed_field(bits = "73")]
    pub headphone_mute: bool,
    #[packed_field(bits = "74")]
    pub speaker_mute: bool,
    #[packed_field(bits = "75")]
    pub mic_mute: bool,
    #[packed_field(bits = "76")]
    pub audio_power_save: bool,
    #[packed_field(bits = "77")]
    pub haptic_power_save: bool,
    #[packed_field(bits = "78")]
    pub motion_power_save: bool,
    #[packed_field(bits = "79")]
    pub touch_power_save: bool,

    // byte 10-31
    #[packed_field(bytes = "10..=20")]
    pub right_trigger_ffb: [u8; 11],
    #[packed_field(bytes = "21..=31")]
    pub left_trigger_ffb: [u8; 11],

    // byte 32
    #[packed_field(bytes = "32..=35", endian = "lsb")]
    pub host_timestamp: Integer<u32, packed_bits::Bits<32>>, // Mirrors data from report read

    // byte 36
    #[packed_field(bits = "288..=291", endian = "lsb")]
    pub rumble_motor_power_reduction: Integer<u8, packed_bits::Bits<4>>,
    #[packed_field(bits = "292..=295", endian = "lsb")]
    pub trigger_motor_power_reduction: Integer<u8, packed_bits::Bits<4>>,

    // byte 37
    #[packed_field(bits = "296..=299", endian = "lsb")]
    pub _unkn_audio_control_2: Integer<u8, packed_bits::Bits<4>>,
    #[packed_field(bits = "300")]
    pub beamforming_enable: bool,
    #[packed_field(bits = "301..=303", endian = "lsb")]
    pub speaker_comp_pre_gain: Integer<u8, packed_bits::Bits<3>>,

    // byte 38
    #[packed_field(bits = "304..=308", endian = "lsb")]
    pub _unkn_bitc: Integer<u8, packed_bits::Bits<5>>,
    #[packed_field(bits = "309")]
    pub enable_improved_rumble_emulation: bool,
    #[packed_field(bits = "310")]
    pub allow_color_light_fade_animation: bool,
    #[packed_field(bits = "311")]
    pub allow_light_brightness_change: bool,

    // byte 39
    #[packed_field(bits = "312..=318", endian = "lsb")]
    pub _unkn_bit_1: Integer<u8, packed_bits::Bits<7>>,
    #[packed_field(bits = "319")]
    pub haptic_low_pass_filter: bool,

    // byte 40 - 42
    #[packed_field(bytes = "40")]
    pub _unkn_bit_2: u8,
    #[packed_field(bytes = "41", ty = "enum")]
    pub light_fade_animation: LightFadeAnimation,
    #[packed_field(bytes = "42", ty = "enum")]
    pub light_brightness: LightBrightness,

    // byte 43
    #[packed_field(bits = "344..=345", endian = "lsb")]
    pub _player_light_unkn: Integer<u8, packed_bits::Bits<2>>,
    #[packed_field(bits = "346")]
    pub player_light_fade: bool,
    #[packed_field(bits = "347")]
    pub player_light_1: bool,
    #[packed_field(bits = "348")]
    pub player_light_2: bool,
    #[packed_field(bits = "349")]
    pub player_light_3: bool,
    #[packed_field(bits = "350")]
    pub player_light_4: bool,
    #[packed_field(bits = "351")]
    pub player_light_5: bool,

    // byte 44-46
    #[packed_field(bytes = "44")]
    pub led_red: u8,
    #[packed_field(bytes = "45")]
    pub led_green: u8,
    #[packed_field(bytes = "46")]
    pub led_blue: u8,
}

#[derive(PackedStruct, Debug, Copy, Clone, PartialEq)]
#[packed_struct(bit_numbering = "msb0", size_bytes = "63")]
pub struct UsbPackedOutputReport {
    // byte 0
    #[packed_field(bytes = "0")]
    pub report_id: u8, // Report ID (always 0x02)

    // byte 1-47
    #[packed_field(bytes = "1..=47")]
    pub state: SetStatePackedOutputData,
}

impl Default for UsbPackedOutputReport {
    fn default() -> Self {
        Self {
            report_id: 0x02,
            state: Default::default(),
        }
    }
}

#[derive(PackedStruct, Debug, Copy, Clone, PartialEq)]
#[packed_struct(bit_numbering = "msb0", size_bytes = "48")]
pub struct UsbPackedOutputReportShort {
    // byte 0
    #[packed_field(bytes = "0")]
    pub report_id: u8, // Report ID (always 0x02)

    // byte 1-47
    #[packed_field(bytes = "1..=47")]
    pub state: SetStatePackedOutputData,
}

impl Default for UsbPackedOutputReportShort {
    fn default() -> Self {
        Self {
            report_id: 0x02,
            state: Default::default(),
        }
    }
}
