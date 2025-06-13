use packed_struct::prelude::*;

use super::driver::REPORT_ID;

#[derive(PrimitiveEnum_u8, Clone, Copy, PartialEq, Debug, Default)]
pub enum Direction {
    Up = 1,
    UpRight = 3,
    Right = 2,
    DownRight = 6,
    Down = 4,
    DownLeft = 12,
    Left = 8,
    UpLeft = 9,
    #[default]
    None = 0,
}

/// report for dinput via dongle
#[derive(PackedStruct, Debug, Copy, Clone, PartialEq)]
#[packed_struct(bit_numbering = "msb0", size_bytes = "32")]
pub struct PackedInputDataReport {
    // byte 0
    #[packed_field(bytes = "0", endian = "lsb")]
    pub report_id: u8, // report always 04

    // byte 7
    #[packed_field(bits = "58")]
    pub l5: bool,
    #[packed_field(bits = "59")]
    pub r5: bool,
    #[packed_field(bits = "60")]
    pub l4: bool,
    #[packed_field(bits = "61")]
    pub r4: bool,
    #[packed_field(bits = "62")]
    pub m2: bool,
    #[packed_field(bits = "63")]
    pub m1: bool,

    // byte 8
    #[packed_field(bits = "71")]
    pub quick: bool,
    #[packed_field(bits = "68")]
    pub steam: bool,

    // byte 9
    #[packed_field(bits = "76..=79", ty = "enum")]
    pub dpad: Direction, // Directional buttons
    #[packed_field(bits = "75")]
    pub a: bool,
    #[packed_field(bits = "74")]
    pub b: bool,
    #[packed_field(bits = "73")]
    pub view: bool,
    #[packed_field(bits = "72")]
    pub y: bool,

    // byte 10
    #[packed_field(bits = "87")]
    pub x: bool,
    #[packed_field(bits = "86")]
    pub menu: bool,
    #[packed_field(bits = "85")]
    pub lb: bool,
    #[packed_field(bits = "84")]
    pub rb: bool,
    #[packed_field(bits = "83")]
    pub ltdigital: bool,
    #[packed_field(bits = "82")]
    pub rtdigital: bool,
    #[packed_field(bits = "81")]
    pub lsclick: bool,
    #[packed_field(bits = "80")]
    pub rsclick: bool,

    // bytes 11-16 // Accelerometer X Y Z
    #[packed_field(bytes = "11..=12", endian = "lsb")]
    pub accel_x: Integer<i16, packed_bits::Bits<16>>,
    #[packed_field(bytes = "13..-14", endian = "lsb")]
    pub accel_y: Integer<i16, packed_bits::Bits<16>>,
    #[packed_field(bytes = "15..=16", endian = "lsb")]
    pub accel_z: Integer<i16, packed_bits::Bits<16>>,

    // byte 17
    #[packed_field(bytes = "17", endian = "lsb")]
    pub joystick_l_x: u8,

    // byte 18 // Half of Gyro Y
    #[packed_field(bytes = "18", endian = "lsb")]
    pub gyro_y_lo: u8,

    // byte 19
    #[packed_field(bytes = "19", endian = "lsb")]
    pub joystick_l_y: u8,

    // Byte 20 // Half of Gyro Y
    #[packed_field(bytes = "20", endian = "lsb")]
    pub gyro_y_hi: u8,

    // byte 21-24
    #[packed_field(bytes = "21", endian = "lsb")]
    pub joystick_r_x: u8,
    #[packed_field(bytes = "22", endian = "lsb")]
    pub joystick_r_y: u8,
    #[packed_field(bytes = "23", endian = "lsb")]
    pub lt_analog: u8,
    #[packed_field(bytes = "24", endian = "lsb")]
    pub rt_analog: u8,

    // bytes 26-27 // Gyro X
    #[packed_field(bytes = "26..=27", endian = "lsb")]
    pub gyro_x: Integer<i16, packed_bits::Bits<16>>,

    // bytes 29-30 // Gyro Z
    #[packed_field(bytes = "29..=30", endian = "lsb")]
    pub gyro_z: Integer<i16, packed_bits::Bits<16>>,
}

impl PackedInputDataReport {
    pub fn get_y(&self) -> i16 {
        let gyro_y_lo = self.gyro_y_lo as i16;
        let gyro_y_hi = (self.gyro_y_hi as i16).rotate_left(8);
        gyro_y_hi | gyro_y_lo
    }
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
            rtdigital: false,
            ltdigital: false,
            rb: false,
            lb: false,
            y: false,
            r4: false,
            l4: false,
            r5: false,
            l5: false,
            m1: false,
            m2: false,
            rsclick: false,
            lsclick: false,
            steam: false,
            rt_analog: 0,
            lt_analog: 0,
            gyro_y_lo: Default::default(),
            gyro_y_hi: Default::default(),
            gyro_x: Integer::from_primitive(0),
            gyro_z: Integer::from_primitive(0),
            accel_z: Integer::from_primitive(0),
            accel_y: Integer::from_primitive(0),
            accel_x: Integer::from_primitive(0),
        }
    }
}
