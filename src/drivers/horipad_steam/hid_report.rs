use packed_struct::prelude::*;

use super::driver::REPORT_ID;

#[derive(PrimitiveEnum_u8, Clone, Copy, PartialEq, Debug, Default)]
pub enum Direction {
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

impl Direction {
    pub fn as_bitflag(&self) -> u8 {
        match *self {
            Self::Up => 0b00000001,
            Self::UpRight => 0b00000011,
            Self::Right => 0b00000010,
            Self::DownRight => 0b00000110,
            Self::Down => 0b00000100,
            Self::DownLeft => 0b00001100,
            Self::Left => 0b00001000,
            Self::UpLeft => 0b00001001,
            Self::None => 0b00000000,
        }
    }

    pub fn from_bitflag(bits: u8) -> Self {
        match bits {
            0b00000001 => Self::Up,
            0b00000011 => Self::UpRight,
            0b00000010 => Self::Right,
            0b00000110 => Self::DownRight,
            0b00000100 => Self::Down,
            0b00001100 => Self::DownLeft,
            0b00001000 => Self::Left,
            0b00001001 => Self::UpLeft,
            0b00000000 => Self::None,
            _ => Self::None,
        }
    }

    pub fn change(&self, direction: Direction, pressed: bool) -> Direction {
        let old_direction = self.as_bitflag();
        let new_direction = if pressed {
            old_direction | direction.as_bitflag()
        } else {
            old_direction ^ direction.as_bitflag()
        };
        Direction::from_bitflag(new_direction)
    }
}

/// Horipad Steam input report for Bluetooth
#[derive(PackedStruct, Debug, Copy, Clone, PartialEq)]
#[packed_struct(bit_numbering = "msb0", size_bytes = "287")]
pub struct PackedInputDataReport {
    // byte 0
    #[packed_field(bytes = "0")]
    pub report_id: u8, // Report ID (always 0x07)

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
    pub x: bool,
    #[packed_field(bits = "41")]
    pub quick: bool, // Quick Access ... Button
    #[packed_field(bits = "42")]
    pub b: bool,
    #[packed_field(bits = "43")]
    pub a: bool,
    #[packed_field(bits = "44..=47", ty = "enum")]
    pub dpad: Direction, // Directional buttons

    // byte 6
    #[packed_field(bits = "48")]
    pub menu: bool, // ☰ Button
    #[packed_field(bits = "49")]
    pub view: bool, // ⧉  Button
    #[packed_field(bits = "50")]
    pub rt_digital: bool,
    #[packed_field(bits = "51")]
    pub lt_digital: bool,
    #[packed_field(bits = "52")]
    pub rb: bool,
    #[packed_field(bits = "53")]
    pub lb: bool,
    #[packed_field(bits = "54")]
    pub m1: bool,
    #[packed_field(bits = "55")]
    pub y: bool, // Triggers

    // byte 7
    #[packed_field(bits = "56")]
    pub r4: bool,
    #[packed_field(bits = "57")]
    pub l4: bool,
    #[packed_field(bits = "58")]
    pub rs_touch: bool,
    #[packed_field(bits = "59")]
    pub ls_touch: bool,
    #[packed_field(bits = "60")]
    pub m2: bool,
    #[packed_field(bits = "61")]
    pub rs_click: bool,
    #[packed_field(bits = "62")]
    pub ls_click: bool,
    #[packed_field(bits = "63")]
    pub steam: bool, // Steam Button

    // byte 8-9
    #[packed_field(bytes = "8")]
    pub rt_analog: u8, // L2 trigger axis
    #[packed_field(bytes = "9")]
    pub lt_analog: u8, // R2 trigger axis

    // byte 10-11
    #[packed_field(bytes = "10..=11", endian = "lsb")]
    pub tick: Integer<u16, packed_bits::Bits<16>>,

    // bytes 12-17 // Gyro
    #[packed_field(bytes = "12..=13", endian = "lsb")]
    pub gyro_z: Integer<i16, packed_bits::Bits<16>>,
    #[packed_field(bytes = "14..=15", endian = "lsb")]
    pub gyro_y: Integer<i16, packed_bits::Bits<16>>,
    #[packed_field(bytes = "16..=17", endian = "lsb")]
    pub gyro_x: Integer<i16, packed_bits::Bits<16>>,
    // bytes 18-23 // Accelerometer
    #[packed_field(bytes = "18..=19", endian = "lsb")]
    pub accel_z: Integer<i16, packed_bits::Bits<16>>,
    #[packed_field(bytes = "20..=21", endian = "lsb")]
    pub accel_y: Integer<i16, packed_bits::Bits<16>>,
    #[packed_field(bytes = "22..=23", endian = "lsb")]
    pub accel_x: Integer<i16, packed_bits::Bits<16>>,

    // byte 24
    #[packed_field(bits = "195", endian = "lsb")]
    pub charging: bool,
    #[packed_field(bits = "196..=199", endian = "lsb")]
    pub charge_percent: Integer<u8, packed_bits::Bits<4>>,
}

impl Default for PackedInputDataReport {
    fn default() -> Self {
        Self {
            report_id: REPORT_ID,
            joystick_l_x: 128,
            joystick_l_y: 128,
            joystick_r_x: 128,
            joystick_r_y: 128,
            x: false,
            quick: false,
            b: false,
            a: false,
            dpad: Direction::None,
            menu: false,
            view: false,
            rt_digital: false,
            lt_digital: false,
            rb: false,
            lb: false,
            m1: false,
            y: false,
            r4: false,
            l4: false,
            rs_touch: false,
            ls_touch: false,
            m2: false,
            rs_click: false,
            ls_click: false,
            steam: false,
            rt_analog: 0,
            lt_analog: 0,
            tick: Integer::from_primitive(0),
            gyro_z: Integer::from_primitive(0),
            gyro_y: Integer::from_primitive(0),
            gyro_x: Integer::from_primitive(0),
            accel_z: Integer::from_primitive(0),
            accel_y: Integer::from_primitive(0),
            accel_x: Integer::from_primitive(0),
            charging: false,
            charge_percent: Integer::from_primitive(0),
        }
    }
}
