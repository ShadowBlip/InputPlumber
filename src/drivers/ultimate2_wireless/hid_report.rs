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

use super::driver::{REPORT_ID_INPUT, REPORT_ID_RUMBLE};

#[derive(PrimitiveEnum_u8, Clone, Copy, PartialEq, Debug, Default)]
pub enum DpadDirection {
    Up = 0,
    UpRight = 1,
    Right = 2,
    DownRight = 3,
    Down = 4,
    DownLeft = 5,
    Left = 6,
    UpLeft = 7,
    #[default]
    None = 8,
}

impl DpadDirection {
    fn as_bitflag(&self) -> u8 {
        match *self {
            Self::Up => 0b0001,
            Self::UpRight => 0b0011,
            Self::Right => 0b0010,
            Self::DownRight => 0b0110,
            Self::Down => 0b0100,
            Self::DownLeft => 0b1100,
            Self::Left => 0b1000,
            Self::UpLeft => 0b1001,
            Self::None => 0b0000,
        }
    }

    fn from_bitflag(bits: u8) -> Self {
        match bits {
            0b0001 => Self::Up,
            0b0011 => Self::UpRight,
            0b0010 => Self::Right,
            0b0110 => Self::DownRight,
            0b0100 => Self::Down,
            0b1100 => Self::DownLeft,
            0b1000 => Self::Left,
            0b1001 => Self::UpLeft,
            _ => Self::None,
        }
    }

    pub fn change(&self, direction: DpadDirection, pressed: bool) -> DpadDirection {
        let old = self.as_bitflag();
        let new = if pressed {
            old | direction.as_bitflag()
        } else {
            old & !direction.as_bitflag()
        };
        DpadDirection::from_bitflag(new)
    }
}

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
            report_id: REPORT_ID_RUMBLE,
            strong_magnitude: 0,
            weak_magnitude: 0,
            _padding_3: 0,
            _padding_4: 0,
        }
    }
}

/// 8BitDo Ultimate 2 Wireless DInput input report (34 bytes total).
#[derive(PackedStruct, Debug, Copy, Clone, PartialEq)]
#[packed_struct(bit_numbering = "msb0", size_bytes = "34")]
pub struct PackedInputDataReport {
    // byte 0: report ID
    #[packed_field(bytes = "0")]
    pub report_id: u8,

    // byte 1: D-pad (raw value 0-7 or 8+ for centered)
    #[packed_field(bytes = "1")]
    pub dpad_raw: u8,

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
    pub rt_analog: u8,
    #[packed_field(bytes = "7")]
    pub lt_analog: u8,

    // byte 8: buttons group 1
    // data[8]: 0x80=RB 0x40=LB 0x20=PL 0x10=X 0x08=Y 0x04=PR 0x02=B 0x01=A
    // (SDL source labels 0x10=Y, 0x08=X, but Steam maps them reversed)
    #[packed_field(bits = "64")]
    pub btn_rb: bool,   // mask 0x80
    #[packed_field(bits = "65")]
    pub btn_lb: bool,   // mask 0x40
    #[packed_field(bits = "66")]
    pub btn_pl: bool,   // mask 0x20
    #[packed_field(bits = "67")]
    pub btn_x: bool,    // mask 0x10
    #[packed_field(bits = "68")]
    pub btn_y: bool,    // mask 0x08
    #[packed_field(bits = "69")]
    pub btn_pr: bool,   // mask 0x04
    #[packed_field(bits = "70")]
    pub btn_b: bool,    // mask 0x02
    #[packed_field(bits = "71")]
    pub btn_a: bool,    // mask 0x01

    // byte 9: buttons group 2
    // data[9]: 0x40=R3 0x20=L3 0x10=Guide 0x08=Start 0x04=Select
    #[packed_field(bits = "72")]
    pub _unused_9_7: bool,   // mask 0x80
    #[packed_field(bits = "73")]
    pub btn_r3: bool,        // mask 0x40
    #[packed_field(bits = "74")]
    pub btn_l3: bool,        // mask 0x20
    #[packed_field(bits = "75")]
    pub btn_guide: bool,     // mask 0x10
    #[packed_field(bits = "76")]
    pub btn_start: bool,     // mask 0x08
    #[packed_field(bits = "77")]
    pub btn_select: bool,    // mask 0x04
    #[packed_field(bits = "78")]
    pub _unused_9_1: bool,   // mask 0x02
    #[packed_field(bits = "79")]
    pub _unused_9_0: bool,   // mask 0x01

    // byte 10: extra back buttons (data[10]: 0x02=R4 0x01=L4)
    #[packed_field(bits = "80")]
    pub _unused_10_7: bool,
    #[packed_field(bits = "81")]
    pub _unused_10_6: bool,
    #[packed_field(bits = "82")]
    pub _unused_10_5: bool,
    #[packed_field(bits = "83")]
    pub _unused_10_4: bool,
    #[packed_field(bits = "84")]
    pub _unused_10_3: bool,
    #[packed_field(bits = "85")]
    pub _unused_10_2: bool,
    #[packed_field(bits = "86")]
    pub btn_r4: bool,        // mask 0x02
    #[packed_field(bits = "87")]
    pub btn_l4: bool,        // mask 0x01

    // bytes 11-13: padding
    #[packed_field(bytes = "11")]
    pub _padding_11: u8,
    #[packed_field(bytes = "12")]
    pub _padding_12: u8,
    #[packed_field(bytes = "13")]
    pub _padding_13: u8,

    // byte 14: battery status (bit7=charging, bits0-6=percent)
    #[packed_field(bytes = "14")]
    pub battery: u8,

    // bytes 15-26: IMU sensor data (i16 little-endian each)
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

    // bytes 27-30: timestamp (u32 little-endian, microseconds)
    #[packed_field(bytes = "27..=30", endian = "lsb")]
    pub timestamp: Integer<u32, packed_bits::Bits<32>>,

    // bytes 31-33: padding
    #[packed_field(bytes = "31")]
    pub _padding_31: u8,
    #[packed_field(bytes = "32")]
    pub _padding_32: u8,
    #[packed_field(bytes = "33")]
    pub _padding_33: u8,
}

impl Default for PackedInputDataReport {
    fn default() -> Self {
        Self {
            report_id: REPORT_ID_INPUT,
            dpad_raw: 8, // centered
            joystick_l_x: 0x7f,
            joystick_l_y: 0x7f,
            joystick_r_x: 0x7f,
            joystick_r_y: 0x7f,
            rt_analog: 0,
            lt_analog: 0,
            btn_a: false,
            btn_b: false,
            btn_pr: false,
            btn_x: false,
            btn_y: false,
            btn_pl: false,
            btn_lb: false,
            btn_rb: false,
            _unused_9_0: false,
            _unused_9_1: false,
            btn_select: false,
            btn_start: false,
            btn_guide: false,
            btn_l3: false,
            btn_r3: false,
            _unused_9_7: false,
            btn_l4: false,
            btn_r4: false,
            _unused_10_2: false,
            _unused_10_3: false,
            _unused_10_4: false,
            _unused_10_5: false,
            _unused_10_6: false,
            _unused_10_7: false,
            _padding_11: 0,
            _padding_12: 0,
            _padding_13: 0,
            battery: 100,
            accel_x: Integer::from_primitive(0),
            accel_y: Integer::from_primitive(0),
            accel_z: Integer::from_primitive(0),
            gyro_x: Integer::from_primitive(0),
            gyro_y: Integer::from_primitive(0),
            gyro_z: Integer::from_primitive(0),
            timestamp: Integer::from_primitive(0),
            _padding_31: 0,
            _padding_32: 0,
            _padding_33: 0,
        }
    }
}

impl PackedInputDataReport {
    pub fn set_dpad(&mut self, direction: DpadDirection, pressed: bool) {
        let current = DpadDirection::from_bitflag(self.dpad_as_bitflag());
        let updated = current.change(direction, pressed);
        self.dpad_raw = match updated {
            DpadDirection::None => 8,
            d => d as u8,
        };
    }

    fn dpad_as_bitflag(&self) -> u8 {
        let dir = match self.dpad_raw {
            0 => DpadDirection::Up,
            1 => DpadDirection::UpRight,
            2 => DpadDirection::Right,
            3 => DpadDirection::DownRight,
            4 => DpadDirection::Down,
            5 => DpadDirection::DownLeft,
            6 => DpadDirection::Left,
            7 => DpadDirection::UpLeft,
            _ => DpadDirection::None,
        };
        dir.as_bitflag()
    }
}
