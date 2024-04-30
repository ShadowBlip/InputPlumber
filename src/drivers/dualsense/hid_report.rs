//! Structures derived from the great work of the community of the Game Controller
//! Collective Wiki.
//! Source: https://controllers.fandom.com/wiki/Sony_DualSense
use packed_struct::prelude::*;

use super::driver::*;

#[derive(Debug, Copy, Clone)]
pub enum PackedInputDataReport {
    Usb(USBPackedInputDataReport),
    Bluetooth(BluetoothPackedInputDataReport),
}

#[derive(PrimitiveEnum_u8, Clone, Copy, PartialEq, Debug)]
pub enum Direction {
    North = 0,
    NorthEast = 1,
    East = 2,
    SouthEast = 3,
    South = 4,
    SouthWest = 5,
    West = 6,
    NorthWest = 7,
    None = 8,
}

#[derive(PrimitiveEnum_u8, Clone, Copy, PartialEq, Debug)]
pub enum PowerState {
    Disharging = 0x00,
    Charging = 0x01,
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

impl TouchFingerData {
    pub fn set_x(&mut self, x_raw: u16) {
        self.x_lo = (x_raw & 0x00FF) as u8;
        self.x_hi = Integer::from_primitive((x_raw & 0x0F00).rotate_right(8) as u8);
    }
    pub fn set_y(&mut self, y_raw: u16) {
        self.y_lo = Integer::from_primitive((y_raw & 0x000F) as u8);
        self.y_hi = (y_raw & 0x0FF0).rotate_right(4) as u8;
    }
}

#[derive(PackedStruct, Debug, Copy, Clone, PartialEq)]
#[packed_struct(bit_numbering = "msb0", size_bytes = "9")]
pub struct TouchData {
    #[packed_field(element_size_bytes = "4")]
    pub touch_finger_data: [TouchFingerData; 2],
    pub timestamp: u8,
}

#[derive(PackedStruct, Debug, Copy, Clone, PartialEq)]
#[packed_struct(bit_numbering = "msb0", size_bytes = "64")]
pub struct USBPackedInputDataReport {
    // byte 0
    #[packed_field(bytes = "0")]
    pub report_id: u8, // Report ID (always 0x01)

    // byte 1-7
    #[packed_field(bytes = "1")]
    pub joystick_l_x: u8, // left stick X axis
    #[packed_field(bytes = "2")]
    pub joystick_l_y: u8, // left stick Y axis
    #[packed_field(bytes = "3")]
    pub joystick_r_x: u8, // right stick X axis
    #[packed_field(bytes = "4")]
    pub joystick_r_y: u8, // right stick Y axis
    #[packed_field(bytes = "5")]
    pub l2_trigger: u8, // L2 trigger axis
    #[packed_field(bytes = "6")]
    pub r2_trigger: u8, // R2 trigger axis
    #[packed_field(bytes = "7")]
    pub seq_number: u8, // Sequence number, always 0x01 on BT

    // byte 8
    #[packed_field(bits = "64")]
    pub triangle: bool, // Button cluster, x, ◯, □, ∆
    #[packed_field(bits = "65")]
    pub circle: bool,
    #[packed_field(bits = "66")]
    pub cross: bool,
    #[packed_field(bits = "67")]
    pub square: bool,
    #[packed_field(bits = "68..=71", ty = "enum")]
    pub dpad: Direction, // Directional buttons

    // byte 9
    #[packed_field(bits = "72")]
    pub r3: bool,
    #[packed_field(bits = "73")]
    pub l3: bool,
    #[packed_field(bits = "74")]
    pub options: bool, // Options button ☰
    #[packed_field(bits = "75")]
    pub create: bool, // Create button ⚟
    #[packed_field(bits = "76")]
    pub r2: bool, // Triggers
    #[packed_field(bits = "77")]
    pub l2: bool,
    #[packed_field(bits = "78")]
    pub r1: bool,
    #[packed_field(bits = "79")]
    pub l1: bool,

    // byte 10
    #[packed_field(bits = "80")]
    pub right_paddle: bool, // Right paddle button (DualSense Edge)
    #[packed_field(bits = "81")]
    pub left_paddle: bool, // Left paddle button (DualSense Edge)
    #[packed_field(bits = "82")]
    pub right_fn: bool, // Right function button (DualSense Edge)
    #[packed_field(bits = "83")]
    pub left_fn: bool, // Left function button (DualSense Edge)
    #[packed_field(bits = "84")]
    pub _unkn_0: bool, // Appears unused
    #[packed_field(bits = "85")]
    pub mute: bool, // Mic mute button 🔇
    #[packed_field(bits = "86")]
    pub touchpad: bool, // Touchpad button
    #[packed_field(bits = "87")]
    pub ps: bool, // PS button

    // byte 11
    #[packed_field(bytes = "11")]
    pub _unkn_1: u8, // Appears unused

    // byte 12-15
    #[packed_field(bytes = "12..=15", endian = "lsb")]
    pub _unkn_counter: Integer<u32, packed_bits::Bits<32>>, // Linux driver calls this reserved

    // byte 16-27
    #[packed_field(bytes = "16..=17", endian = "lsb")]
    pub gyro_x: Integer<i16, packed_bits::Bits<16>>, // Gyro
    #[packed_field(bytes = "18..=19", endian = "lsb")]
    pub gyro_y: Integer<i16, packed_bits::Bits<16>>,
    #[packed_field(bytes = "20..=21", endian = "lsb")]
    pub gyro_z: Integer<i16, packed_bits::Bits<16>>,
    #[packed_field(bytes = "22..=23", endian = "lsb")]
    pub accel_x: Integer<i16, packed_bits::Bits<16>>, // Accelerometer
    #[packed_field(bytes = "24..=25", endian = "lsb")]
    pub accel_y: Integer<i16, packed_bits::Bits<16>>,
    #[packed_field(bytes = "26..=27", endian = "lsb")]
    pub accel_z: Integer<i16, packed_bits::Bits<16>>,

    // byte 28
    #[packed_field(bytes = "28..=31", endian = "lsb")]
    pub sensor_timestamp: Integer<u32, packed_bits::Bits<32>>,
    #[packed_field(bytes = "32")]
    pub temperature: u8, // reserved2 in Linux driver

    // byte 33-41
    #[packed_field(bytes = "33..=41")]
    pub touch_data: TouchData,

    // byte 42-43
    #[packed_field(bits = "336..=339", endian = "lsb")]
    pub trigger_left_status: Integer<u8, packed_bits::Bits<4>>,
    #[packed_field(bits = "340..=343", endian = "lsb")]
    pub trigger_left_stop_location: Integer<u8, packed_bits::Bits<4>>, // Can range from 0-9
    #[packed_field(bits = "344..=347", endian = "lsb")]
    pub trigger_right_status: Integer<u8, packed_bits::Bits<4>>,
    #[packed_field(bits = "348..=351", endian = "lsb")]
    pub trigger_right_stop_location: Integer<u8, packed_bits::Bits<4>>, // Can range from 0-9

    // byte 44-47
    #[packed_field(bytes = "44..=47", endian = "lsb")]
    pub host_timestamp: Integer<u32, packed_bits::Bits<32>>, // Mirrors data from report write

    // byte 48
    #[packed_field(bits = "384..=387", endian = "lsb")]
    pub trigger_left_effect: Integer<u8, packed_bits::Bits<4>>,
    #[packed_field(bits = "388..=391", endian = "lsb")]
    pub trigger_right_effect: Integer<u8, packed_bits::Bits<4>>,

    // byte 49-52
    #[packed_field(bytes = "49..=52", endian = "lsb")]
    pub device_timestamp: Integer<u32, packed_bits::Bits<32>>,

    // byte 53
    #[packed_field(bits = "424..=427", ty = "enum")]
    pub power_state: PowerState,
    #[packed_field(bits = "428..=431", endian = "lsb")]
    pub power_percent: Integer<u8, packed_bits::Bits<4>>, // 0x00 - 0x0A

    // byte 54
    #[packed_field(bits = "432..434", endian = "lsb")]
    pub _plugged_unkn_0: Integer<u8, packed_bits::Bits<3>>,
    #[packed_field(bits = "435")]
    pub plugged_usb_power: bool,
    #[packed_field(bits = "436")]
    pub plugged_usb_data: bool,
    #[packed_field(bits = "437")]
    pub mic_mutes: bool,
    #[packed_field(bits = "438")]
    pub plugged_mic: bool,
    #[packed_field(bits = "439")]
    pub plugged_headphones: bool,

    // byte 55
    #[packed_field(bits = "440..=445", endian = "lsb")]
    pub _plugged_unkn_1: Integer<u8, packed_bits::Bits<6>>,
    #[packed_field(bits = "446")]
    pub haptic_low_pass_filter: bool,
    #[packed_field(bits = "447")]
    pub plugged_external_mic: bool,

    // byte 56-64
    #[packed_field(bytes = "56..=63")]
    pub aes_cmac: [u8; 8],
}

impl USBPackedInputDataReport {
    /// Return a new empty input data report
    pub fn new() -> Self {
        Self {
            report_id: INPUT_REPORT_USB,
            joystick_l_x: 127,
            joystick_l_y: 127,
            joystick_r_x: 127,
            joystick_r_y: 127,
            l2_trigger: 0,
            r2_trigger: 0,
            seq_number: 0,
            dpad: Direction::None,
            square: false,
            cross: false,
            circle: false,
            triangle: false,
            l1: false,
            r1: false,
            l2: false,
            r2: false,
            create: false,
            options: false,
            l3: false,
            r3: false,
            ps: false,
            touchpad: false,
            mute: false,
            _unkn_0: false,
            left_fn: false,
            right_fn: false,
            left_paddle: false,
            right_paddle: false,
            _unkn_1: 0,
            _unkn_counter: Integer::from_primitive(0),
            gyro_x: Integer::from_primitive(0),
            gyro_y: Integer::from_primitive(0),
            gyro_z: Integer::from_primitive(0),
            accel_x: Integer::from_primitive(0),
            accel_y: Integer::from_primitive(0),
            accel_z: Integer::from_primitive(0),
            sensor_timestamp: Integer::from_primitive(0),
            temperature: 0,
            touch_data: TouchData {
                touch_finger_data: [
                    TouchFingerData {
                        context: 128,
                        x_lo: 0,
                        y_lo: Integer::from_primitive(0),
                        x_hi: Integer::from_primitive(0),
                        y_hi: 0,
                    },
                    TouchFingerData {
                        context: 128,
                        x_lo: 0,
                        y_lo: Integer::from_primitive(0),
                        x_hi: Integer::from_primitive(0),
                        y_hi: 0,
                    },
                ],
                timestamp: 0,
            },
            trigger_right_stop_location: Integer::from_primitive(0),
            trigger_right_status: Integer::from_primitive(0),
            trigger_left_stop_location: Integer::from_primitive(0),
            trigger_left_status: Integer::from_primitive(0),
            host_timestamp: Integer::from_primitive(0),
            trigger_right_effect: Integer::from_primitive(0),
            trigger_left_effect: Integer::from_primitive(0),
            device_timestamp: Integer::from_primitive(0),
            power_percent: Integer::from_primitive(100),
            power_state: PowerState::Complete,
            plugged_headphones: false,
            plugged_mic: false,
            mic_mutes: false,
            plugged_usb_data: true,
            plugged_usb_power: true,
            _plugged_unkn_0: Integer::from_primitive(0),
            plugged_external_mic: false,
            haptic_low_pass_filter: false,
            _plugged_unkn_1: Integer::from_primitive(0),
            aes_cmac: [0, 0, 0, 0, 0, 0, 0, 0],
        }
    }
}

impl Default for USBPackedInputDataReport {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(PackedStruct, Debug, Copy, Clone, PartialEq)]
#[packed_struct(bit_numbering = "msb0", size_bytes = "10")]
pub struct BluetoothPackedInputDataReport {
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

impl BluetoothPackedInputDataReport {
    /// Return a new empty input data report
    pub fn new() -> Self {
        Self {
            report_id: INPUT_REPORT_BT,
            joystick_l_x: 127,
            joystick_l_y: 127,
            joystick_r_x: 127,
            joystick_r_y: 127,
            l2_trigger: 0,
            r2_trigger: 0,
            counter: [0, 0, 0, 0, 0, 0],
            dpad: Direction::None,
            square: false,
            cross: false,
            circle: false,
            triangle: false,
            l1: false,
            r1: false,
            l2: false,
            r2: false,
            create: false,
            options: false,
            l3: false,
            r3: false,
            ps: false,
            touchpad: false,
        }
    }
}

impl Default for BluetoothPackedInputDataReport {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(PrimitiveEnum_u8, Clone, Copy, PartialEq, Debug)]
pub enum MuteLight {
    Off = 0,
    On = 1,
    Breathing = 2,
    DoNothing = 3,
    NoAction4 = 4,
    NoAction5 = 5,
    NoAction6 = 6,
    NoAction7 = 7,
}

#[derive(PrimitiveEnum_u8, Clone, Copy, PartialEq, Debug)]
pub enum LightFadeAnimation {
    Nothing = 0,
    FadeIn = 1,
    FadeOut = 2,
}

#[derive(PrimitiveEnum_u8, Clone, Copy, PartialEq, Debug)]
pub enum LightBrightness {
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
#[derive(PackedStruct, Debug, Copy, Clone, PartialEq)]
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
