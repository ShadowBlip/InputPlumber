// 8BitDo Ultimate 2 Wireless DInput HID report structures.
// Layouts derived from SDL_hidapi_8bitdo.c.
//
// Input report (0x04), 34 bytes:
//   [0]     report_id       (always 0x04)
//   [1]     dpad            (0-7 = direction, other = centered)
//   [2]     joystick_l_x    (0x00-0xFF, center 0x7F)
//   [3]     joystick_l_y
//   [4]     joystick_r_x
//   [5]     joystick_r_y
//   [6]     rt_analog       (right trigger, 0x00-0xFF)
//   [7]     lt_analog       (left trigger,  0x00-0xFF)
//   [8]     buttons_1       bit0=A bit1=B bit2=PR bit3=X bit4=Y bit5=PL bit6=LB bit7=RB
//   [9]     buttons_2       bit2=Select bit3=Start bit4=Guide bit5=L3 bit6=R3
//   [10]    extra_buttons   bit0=L4 bit1=R4
//   [11-13] padding
//   [14]    battery         bit7=charging bit0-6=percent
//   [15-20] accel_x/y/z    (i16 little-endian each)
//   [21-26] gyro_x/y/z     (i16 little-endian each)
//   [27-30] timestamp       (u32 little-endian, microseconds)
//   [31-33] padding
//
// Rumble output report (0x05), 5 bytes:
//   [0]     report_id       (always 0x05)
//   [1]     strong_magnitude high byte (left motor)
//   [2]     weak_magnitude high byte   (right motor)
//   [3-4]   padding

use packed_struct::prelude::*;

/// 8BitDo Ultimate 2 Wireless rumble output report (5 bytes).
#[derive(PackedStruct, Debug, Copy, Clone, PartialEq)]
#[packed_struct(bit_numbering = "msb0", size_bytes = "5")]
pub struct PackedRumbleOutputReport {
    #[packed_field(bytes = "0")]
    pub report_id: u8,
    #[packed_field(bytes = "1")]
    pub strong_magnitude: u8,
    #[packed_field(bytes = "2")]
    pub weak_magnitude: u8,
    #[packed_field(bytes = "3")]
    pub _padding_3: u8,
    #[packed_field(bytes = "4")]
    pub _padding_4: u8,
}

impl Default for PackedRumbleOutputReport {
    fn default() -> Self {
        Self {
            report_id: super::REPORT_ID_RUMBLE,
            strong_magnitude: 0,
            weak_magnitude: 0,
            _padding_3: 0,
            _padding_4: 0,
        }
    }
}

#[derive(PrimitiveEnum_u8, Clone, Copy, PartialEq, Debug, Default)]
pub enum DPadDirection {
    Up = 0,
    UpRight = 1,
    Right = 2,
    DownRight = 3,
    Down = 4,
    DownLeft = 5,
    Left = 6,
    UpLeft = 7,
    #[default]
    None = 15,
}

impl DPadDirection {
    pub fn as_bitflag(&self) -> u8 {
        match *self {
            Self::Up => 0x01,        // 00000001
            Self::UpRight => 0x03,   // 00000011
            Self::Right => 0x02,     // 00000010
            Self::DownRight => 0x06, // 00000110
            Self::Down => 0x04,      // 00000100
            Self::DownLeft => 0x0c,  // 00001100
            Self::Left => 0x08,      // 00001000
            Self::UpLeft => 0x09,    // 00001001
            Self::None => 0x0f,      // 00000000
        }
    }

    fn from_bitflag(bits: u8) -> Self {
        match bits {
            0x01 => Self::Up,
            0x03 => Self::UpRight,
            0x02 => Self::Right,
            0x06 => Self::DownRight,
            0x04 => Self::Down,
            0x0c => Self::DownLeft,
            0x08 => Self::Left,
            0x09 => Self::UpLeft,
            _ => Self::None,
        }
    }

    pub fn change(&self, direction: DPadDirection, pressed: bool) -> DPadDirection {
        let old = self.as_bitflag();
        let new = if pressed {
            old | direction.as_bitflag()
        } else {
            old & !direction.as_bitflag()
        };
        DPadDirection::from_bitflag(new)
    }
}

// Thumb L
// # ReportID: 1 / Hat switch:  15 | # | X:  127 | Y:  127 | Z:  127 | Rz:  127 | Accelerator:    0 | Brake:    0 | Button: 0  0  0  0  0  0  0  0  0  0  0  0  0  1  0  0  0  0  0  0  0  0  0  0 | 0xff000020:    0 ,    0 ,    0 ,   47 ,   47 ,   15 ,  157 ,  255 ,  169 ,  251 ,    2 ,    0 ,   29 ,    0 ,  250 ,  255 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0
// E: 000138.814799 34 01 0f 7f 7f 7f 7f 00 00 00 20 00 00 00 00 2f 2f 0f 9d ff a9 fb 02 00 1d 00 fa ff 00 00 00 00 00 00 00

// Thumb R
// # ReportID: 1 / Hat switch:  15 | # | X:  127 | Y:  127 | Z:  127 | Rz:  127 | Accelerator:    0 | Brake:    0 | Button: 0  0  0  0  0  0  0  0  0  0  0  0  0  0  1  0  0  0  0  0  0  0  0  0 | 0xff000020:    0 ,    0 ,    0 ,   47 ,    5 ,   14 ,  196 ,  255 ,   13 ,    8 ,  239 ,  255 ,  239 ,  255 ,   38 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0
// E: 000006.499840 34 01 0f 7f 7f 7f 7f 00 00 00 40 00 00 00 00 2f 05 0e c4 ff 0d 08 ef ff ef ff 26 00 00 00 00 00 00 00 00

/// 8BitDo Ultimate 2 Wireless DInput input report (34 bytes total).
#[derive(PackedStruct, Debug, Copy, Clone, PartialEq)]
#[packed_struct(bit_numbering = "msb0", size_bytes = "34")]
pub struct PackedInputDataReport {
    // byte 0: report ID
    #[packed_field(bytes = "0")]
    pub report_id: u8,

    // byte 1: D-pad
    #[packed_field(bytes = "1", ty = "enum")]
    pub dpad_state: DPadDirection,

    // bytes 2-5: analog sticks
    #[packed_field(bytes = "2")]
    pub joystick_l_x: u8,
    #[packed_field(bytes = "3")]
    pub joystick_l_y: u8,
    #[packed_field(bytes = "4")]
    pub joystick_r_x: u8,
    #[packed_field(bytes = "5")]
    pub joystick_r_y: u8,

    // bytes 6-7: triggers (SDL: [6]=RT, [7]=LT)
    #[packed_field(bytes = "6")]
    pub trigger_r: u8,
    #[packed_field(bytes = "7")]
    pub trigger_l: u8,

    #[packed_field(bits = "64")]
    pub button_r1: bool,
    #[packed_field(bits = "65")]
    pub button_l1: bool,
    #[packed_field(bits = "66")]
    pub button_l3: bool,
    #[packed_field(bits = "67")]
    pub button_y: bool,
    #[packed_field(bits = "68")]
    pub button_x: bool,
    #[packed_field(bits = "69")]
    pub button_r3: bool,
    #[packed_field(bits = "70")]
    pub button_b: bool,
    #[packed_field(bits = "71")]
    pub button_a: bool,

    //#[packed_field(bits = "72")]
    //pub _unused_9_7: bool,
    #[packed_field(bits = "73")]
    pub button_r2: bool,
    #[packed_field(bits = "74")]
    pub button_l2: bool,
    #[packed_field(bits = "75")]
    pub button_guide: bool,
    #[packed_field(bits = "76")]
    pub button_view: bool,
    #[packed_field(bits = "77")]
    pub button_menu: bool,
    //#[packed_field(bits = "78")]
    //pub _unknown_9_1: bool,
    //#[packed_field(bits = "79")]
    //pub _unknown_9_0: bool,

    //#[packed_field(bits = "80")]
    //pub _unused_10_7: bool,
    //#[packed_field(bits = "81")]
    //pub _unused_10_6: bool,
    //#[packed_field(bits = "82")]
    //pub _unused_10_5: bool,
    //#[packed_field(bits = "83")]
    //pub _unused_10_4: bool,
    //#[packed_field(bits = "84")]
    //pub _unused_10_3: bool,
    //#[packed_field(bits = "85")]
    //pub _unused_10_2: bool,
    #[packed_field(bits = "86")]
    pub button_r4: bool,
    #[packed_field(bits = "87")]
    pub button_l4: bool,

    //#[packed_field(bytes = "11")]
    //pub _padding_11: u8,
    //#[packed_field(bytes = "12")]
    //pub _padding_12: u8,
    //#[packed_field(bytes = "13")]
    //pub _padding_13: u8,
    #[packed_field(bytes = "14")]
    pub battery: u8,

    #[packed_field(bytes = "15..=16", endian = "lsb")]
    pub accel_x: Integer<i16, packed_bits::Bits<16>>,
    #[packed_field(bytes = "17..=18", endian = "lsb")]
    pub accel_y: Integer<i16, packed_bits::Bits<16>>,
    #[packed_field(bytes = "19..=20", endian = "lsb")]
    pub accel_z: Integer<i16, packed_bits::Bits<16>>,
    #[packed_field(bytes = "21..=22", endian = "lsb")]
    pub gyro_x: Integer<i16, packed_bits::Bits<16>>,
    #[packed_field(bytes = "23..=24", endian = "lsb")]
    pub gyro_y: Integer<i16, packed_bits::Bits<16>>,
    #[packed_field(bytes = "25..=26", endian = "lsb")]
    pub gyro_z: Integer<i16, packed_bits::Bits<16>>,

    #[packed_field(bytes = "27..=30", endian = "lsb")]
    pub timestamp: Integer<u32, packed_bits::Bits<32>>,
    //#[packed_field(bytes = "31")]
    //pub _padding_31: u8,
    //#[packed_field(bytes = "32")]
    //pub _padding_32: u8,
    //#[packed_field(bytes = "33")]
    //pub _padding_33: u8,
}

impl PackedInputDataReport {}

impl Default for PackedInputDataReport {
    fn default() -> Self {
        Self {
            report_id: super::REPORT_ID_INPUT,
            dpad_state: Default::default(),
            joystick_l_x: 0x7f,
            joystick_l_y: 0x7f,
            joystick_r_x: 0x7f,
            joystick_r_y: 0x7f,
            trigger_r: 0,
            trigger_l: 0,
            button_a: false,
            button_b: false,
            button_r3: false,
            button_y: false,
            button_x: false,
            button_l3: false,
            button_l1: false,
            button_r1: false,
            button_l2: false,
            button_r2: false,
            button_menu: false,
            button_view: false,
            button_guide: false,
            button_l4: false,
            button_r4: false,
            battery: 100,
            accel_x: Integer::from_primitive(0),
            accel_y: Integer::from_primitive(0),
            accel_z: Integer::from_primitive(0),
            gyro_x: Integer::from_primitive(0),
            gyro_y: Integer::from_primitive(0),
            gyro_z: Integer::from_primitive(0),
            timestamp: Integer::from_primitive(0),
        }
    }
}

impl PackedInputDataReport {
    pub fn set_dpad(&mut self, direction: DPadDirection, pressed: bool) {
        let current = self.dpad_state;
        let updated = current.change(direction, pressed);
        self.dpad_state = updated;
    }
}
