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

/* No Input
# ReportID: 4 / 0xffa00003:   -2 ,  102 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,   -1 ,   -1 ,    0 ,    1 ,  127 ,    0 ,  127 ,    0 ,  127 ,  127 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0
E: 000656.420862 32 04 fe 66 00 00 00 00 00 00 00 00 00 00 ff ff 00 01 7f 00 7f 00 7f 7f 00 00 00 00 00 00 00 00 00
Axes
Left Stick Right
# ReportID: 4 / 0xffa00003:   -2 ,  102 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,   -1 ,   -1 ,    0 ,    1 ,   -1 ,    0 , -104 ,    0 ,  127 ,  127 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,   -1 ,   -1 ,    0
E: 000109.259324 32 04 fe 66 00 00 00 00 00 00 00 00 00 00 ff ff 00 01 ff 00 98 00 7f 7f 00 00 00 00 00 00 ff ff 00
Left Stick Left
# ReportID: 4 / 0xffa00003:   -2 ,  102 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,   -1 ,   -1 ,    1 ,    0 ,   -1 ,    0 ,    0 ,    0 , -111 ,    0 ,  127 ,  127 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    1 ,    0 ,    0
E: 000052.417154 32 04 fe 66 00 00 00 00 00 00 00 00 ff ff 01 00 ff 00 00 00 91 00 7f 7f 00 00 00 00 00 00 01 00 00
Left Stick Up
# ReportID: 4 / 0xffa00003:   -2 ,  102 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,   -1 ,   -1 ,    0 ,    0 ,    0 ,    1 ,  126 ,    0 ,    0 ,    0 ,  127 ,  127 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,   -1 ,   -1 ,    0
E: 000008.126338 32 04 fe 66 00 00 00 00 00 00 00 00 ff ff 00 00 00 01 7e 00 00 00 7f 7f 00 00 00 00 00 00 ff ff 00
Left Stick Down
# ReportID: 4 / 0xffa00003:   -2 ,  102 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,   -1 ,   -1 ,   -1 ,   -1 ,    1 ,    1 ,  114 ,    0 ,   -1 ,    0 ,  127 ,  127 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    1 ,    0 ,    0
E: 000003.707317 32 04 fe 66 00 00 00 00 00 00 00 00 ff ff ff ff 01 01 72 00 ff 00 7f 7f 00 00 00 00 00 00 01 00 00
Right Stick Right
# ReportID: 4 / 0xffa00003:   -2 ,  102 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,   -1 ,   -1 ,    0 ,    1 ,  127 ,    0 ,  127 ,    0 ,   -1 , -114 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,   -1 ,   -1 ,    0
E: 000005.187013 32 04 fe 66 00 00 00 00 00 00 00 00 00 00 ff ff 00 01 7f 00 7f 00 ff 8e 00 00 00 00 00 00 ff ff 00
Right Stick Left
# ReportID: 4 / 0xffa00003:   -2 ,  102 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,   -1 ,   -1 ,    0 ,    1 ,  127 ,    0 ,  127 ,    0 ,    0 ,  110 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    1 ,    0 ,    0
E: 000003.744066 32 04 fe 66 00 00 00 00 00 00 00 00 00 00 ff ff 00 01 7f 00 7f 00 00 6e 00 00 00 00 00 00 01 00 00
Right Stick Up
# ReportID: 4 / 0xffa00003:   -2 ,  102 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,   -1 ,    0 ,  127 ,    0 ,  127 ,    0 , -121 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0
E: 000002.917458 32 04 fe 66 00 00 00 00 00 00 00 00 00 00 00 00 ff 00 7f 00 7f 00 87 00 00 00 00 00 00 00 00 00 00
Right Stick Down
# ReportID: 4 / 0xffa00003:   -2 ,  102 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,   -1 ,   -1 ,   -1 ,    0 ,  127 ,    0 ,  127 ,    0 ,  116 ,   -1 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    1 ,    0 ,    0
E: 000002.757633 32 04 fe 66 00 00 00 00 00 00 00 00 00 00 ff ff ff 00 7f 00 7f 00 74 ff 00 00 00 00 00 00 01 00 00
Left Trigger
# ReportID: 4 / 0xffa00003:   -2 ,  102 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,   16 ,   -1 ,   -1 ,    0 ,    0 ,    0 ,    1 ,  126 ,    0 ,  127 ,    0 ,  127 ,  127 ,   -1 ,    0 ,    0 ,    0 ,    0 ,    0 ,    1 ,    0 ,    0
E: 000009.594140 32 04 fe 66 00 00 00 00 00 00 00 10 ff ff 00 00 00 01 7e 00 7f 00 7f 7f ff 00 00 00 00 00 01 00 00
Right Trigger
# ReportID: 4 / 0xffa00003:   -2 ,  102 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,   32 ,   -1 ,   -1 ,    0 ,    0 ,   -1 ,    0 ,  127 ,    0 ,  127 ,    0 ,  127 ,  127 ,    0 ,   -1 ,    0 ,    0 ,    0 ,    0 ,    2 ,    0 ,    0
E: 000003.136588 32 04 fe 66 00 00 00 00 00 00 00 20 ff ff 00 00 ff 00 7f 00 7f 00 7f 7f 00 ff 00 00 00 00 02 00 00
Buttons
South
# ReportID: 4 / 0xffa00003:   -2 ,  102 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,   16 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    1 ,  127 ,    0 ,  127 ,    0 ,  127 ,  127 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    2 ,    0 ,    0
E: 000006.258012 32 04 fe 66 00 00 00 00 00 00 10 00 00 00 00 00 00 01 7f 00 7f 00 7f 7f 00 00 00 00 00 00 02 00 00
East
# ReportID: 4 / 0xffa00003:   -2 ,  102 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,   32 ,    0 ,    0 ,    0 ,    1 ,    0 ,   -1 ,    0 ,  127 ,    0 ,  127 ,    0 ,  127 ,  127 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    1 ,    0 ,    0
E: 000002.349215 32 04 fe 66 00 00 00 00 00 00 20 00 00 00 01 00 ff 00 7f 00 7f 00 7f 7f 00 00 00 00 00 00 01 00 00
West
# ReportID: 4 / 0xffa00003:   -2 ,  102 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 , -128 ,    0 ,   -1 ,   -1 ,    0 ,    0 ,   -1 ,    0 ,  127 ,    0 ,  127 ,    0 ,  127 ,  127 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0
E: 000001.954331 32 04 fe 66 00 00 00 00 00 00 80 00 ff ff 00 00 ff 00 7f 00 7f 00 7f 7f 00 00 00 00 00 00 00 00 00
North
# ReportID: 4 / 0xffa00003:   -2 ,  102 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    1 ,    0 ,    0 ,    1 ,    0 ,    0 ,    1 ,  127 ,    0 ,  127 ,    0 ,  127 ,  127 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0
E: 000004.489848 32 04 fe 66 00 00 00 00 00 00 00 01 00 00 01 00 00 01 7f 00 7f 00 7f 7f 00 00 00 00 00 00 00 00 00
Start
# ReportID: 4 / 0xffa00003:   -2 ,  102 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    2 ,   -1 ,   -1 ,    0 ,    0 ,   -1 ,    0 ,  127 ,    0 ,  127 ,    0 ,  127 ,  127 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    1 ,    0 ,    0
E: 000001.711024 32 04 fe 66 00 00 00 00 00 00 00 02 ff ff 00 00 ff 00 7f 00 7f 00 7f 7f 00 00 00 00 00 00 01 00 00
Select
# ReportID: 4 / 0xffa00003:   -2 ,  102 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,   64 ,    0 ,   -1 ,   -1 ,    1 ,    0 ,   -1 ,    0 ,  127 ,    0 ,  127 ,    0 ,  127 ,  127 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,   -2 ,   -1 ,    0
E: 000002.145533 32 04 fe 66 00 00 00 00 00 00 40 00 ff ff 01 00 ff 00 7f 00 7f 00 7f 7f 00 00 00 00 00 00 fe ff 00
Home
# ReportID: 4 / 0xffa00003:   -2 ,  102 ,    0 ,    0 ,    0 ,    0 ,    0 ,    8 ,    0 ,    0 ,   -1 ,   -1 ,    0 ,    0 ,   -1 ,    0 ,  127 ,    0 ,  127 ,    0 ,  127 ,  127 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    2 ,    0 ,    0
E: 000010.383148 32 04 fe 66 00 00 00 00 00 08 00 00 ff ff 00 00 ff 00 7f 00 7f 00 7f 7f 00 00 00 00 00 00 02 00 00
Fn
# ReportID: 4 / 0xffa00003:   -2 ,  102 ,    0 ,    0 ,    0 ,    0 ,    0 ,    1 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,   -1 ,    0 ,  127 ,    0 ,  127 ,    0 ,  127 ,  127 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0
E: 000002.236405 32 04 fe 66 00 00 00 00 00 01 00 00 00 00 00 00 ff 00 7f 00 7f 00 7f 7f 00 00 00 00 00 00 00 00 00
DPadUp
# ReportID: 4 / 0xffa00003:   -2 ,  102 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    1 ,    0 ,   -1 ,   -1 ,    0 ,    0 ,   -1 ,    0 ,  127 ,    0 ,  127 ,    0 ,  127 ,  127 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0
E: 000002.584032 32 04 fe 66 00 00 00 00 00 00 01 00 ff ff 00 00 ff 00 7f 00 7f 00 7f 7f 00 00 00 00 00 00 00 00 00
DPadLeft
# ReportID: 4 / 0xffa00003:   -2 ,  102 , -128 ,    0 ,    0 ,    0 ,    0 ,    0 ,    8 ,    0 ,   -1 ,   -1 ,    0 ,    0 ,    0 ,    1 ,  127 ,    0 ,  127 ,    0 ,  127 ,  127 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0
E: 000003.504922 32 04 fe 66 80 00 00 00 00 00 08 00 ff ff 00 00 00 01 7f 00 7f 00 7f 7f 00 00 00 00 00 00 00 00 00
DPadRight
# ReportID: 4 / 0xffa00003:   -2 ,  102 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    2 ,    0 ,   -1 ,   -1 ,    0 ,    0 ,   -1 ,    0 ,  127 ,    0 ,  127 ,    0 ,  127 ,  127 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,   -1 ,   -1 ,    0
E: 000003.211043 32 04 fe 66 00 00 00 00 00 00 02 00 ff ff 00 00 ff 00 7f 00 7f 00 7f 7f 00 00 00 00 00 00 ff ff 00
DPadDown
# ReportID: 4 / 0xffa00003:   -2 ,  102 , -128 ,    0 ,    0 ,    0 ,    0 ,    0 ,    4 ,    0 ,   -1 ,   -1 ,    0 ,    0 ,   -1 ,    0 ,  127 ,    0 ,  127 ,    0 ,  127 ,  127 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,   -1 ,   -1 ,    0
E: 000002.955382 32 04 fe 66 80 00 00 00 00 00 04 00 ff ff 00 00 ff 00 7f 00 7f 00 7f 7f 00 00 00 00 00 00 ff ff 00
Left Bumper
# ReportID: 4 / 0xffa00003:   -2 ,  102 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    4 ,    0 ,    0 ,    0 ,    0 ,    0 ,    1 ,  127 ,    0 ,  127 ,    0 ,  127 ,  127 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,   -1 ,   -1 ,    0
E: 000002.911595 32 04 fe 66 00 00 00 00 00 00 00 04 00 00 00 00 00 01 7f 00 7f 00 7f 7f 00 00 00 00 00 00 ff ff 0
Right Bumper
# ReportID: 4 / 0xffa00003:   -2 ,  102 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    8 ,   -1 ,   -1 ,    0 ,    0 ,   -1 ,    0 ,  127 ,    0 ,  127 ,    0 ,  127 ,  127 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    2 ,    0 ,    0
E: 000002.106028 32 04 fe 66 00 00 00 00 00 00 00 08 ff ff 00 00 ff 00 7f 00 7f 00 7f 7f 00 00 00 00 00 00 02 00 00
Left Stick Click
# ReportID: 4 / 0xffa00003:   -2 ,  102 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,   64 ,   -2 ,   -1 ,    2 ,    0 ,   -1 ,    0 , -114 ,    0 ,  121 ,    0 ,  127 ,  127 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    2 ,    0 ,    0
E: 000004.851067 32 04 fe 66 00 00 00 00 00 00 00 40 fe ff 02 00 ff 00 8e 00 79 00 7f 7f 00 00 00 00 00 00 02 00 00
Right Stick Click
# ReportID: 4 / 0xffa00003:   -2 ,  102 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 , -128 ,    0 ,    0 ,    0 ,    0 ,   -1 ,    0 ,  127 ,    0 ,  127 ,    0 ,  121 , -122 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0
E: 000004.682061 32 04 fe 66 00 00 00 00 00 00 00 80 00 00 00 00 ff 00 7f 00 7f 00 79 86 00 00 00 00 00 00 00 00 00
C
# ReportID: 4 / 0xffa00003:   -2 ,  102 ,    0 ,    0 ,    0 ,    0 ,    1 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,   -1 ,    0 ,  127 ,    0 ,  127 ,    0 ,  127 ,  127 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0
E: 000002.379578 32 04 fe 66 00 00 00 00 01 00 00 00 00 00 00 00 ff 00 7f 00 7f 00 7f 7f 00 00 00 00 00 00 00 00 00
Z
# ReportID: 4 / 0xffa00003:   -2 ,  102 , -128 ,    0 ,    0 ,    0 ,    2 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,   -1 ,    0 ,  127 ,    0 ,  127 ,    0 ,  127 ,  127 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    1 ,    0 ,    0
E: 000005.520075 32 04 fe 66 80 00 00 00 02 00 00 00 00 00 00 00 ff 00 7f 00 7f 00 7f 7f 00 00 00 00 00 00 01 00 00
Left Paddle 1
# ReportID: 4 / 0xffa00003:   -2 ,  102 ,    0 ,    0 ,    0 ,    0 ,    8 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    1 ,  127 ,    0 ,  127 ,    0 ,  127 ,  127 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,   -1 ,   -1 ,    0
E: 000034.390789 32 04 fe 66 00 00 00 00 08 00 00 00 00 00 00 00 00 01 7f 00 7f 00 7f 7f 00 00 00 00 00 00 ff ff 00
Left Paddle 2
# ReportID: 4 / 0xffa00003:   -2 ,  102 ,    0 ,    0 ,    0 ,    0 ,   32 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    1 ,  127 ,    0 ,  127 ,    0 ,  127 ,  127 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    2 ,    0 ,    0
E: 000002.776952 32 04 fe 66 00 00 00 00 20 00 00 00 00 00 00 00 00 01 7f 00 7f 00 7f 7f 00 00 00 00 00 00 02 00 00
Right Paddle 1
# ReportID: 4 / 0xffa00003:   -2 ,  102 ,    0 ,    0 ,    0 ,    0 ,    4 ,    0 ,    0 ,    0 ,    0 ,    0 ,   -1 ,   -1 ,   -1 ,    0 ,  127 ,    0 ,  127 ,    0 ,  127 ,  127 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0
E: 000003.023968 32 04 fe 66 00 00 00 00 04 00 00 00 00 00 ff ff ff 00 7f 00 7f 00 7f 7f 00 00 00 00 00 00 00 00 00
Right Paddle 2
# ReportID: 4 / 0xffa00003:   -2 ,  102 ,    0 ,    0 ,    0 ,    0 ,   16 ,    0 ,    0 ,    0 ,   -1 ,   -1 ,    0 ,    0 ,    0 ,    1 ,  127 ,    0 ,  127 ,    0 ,  127 ,  127 ,    0 ,    0 ,    0 ,    0 ,    0 ,    0 ,    1 ,    0 ,    0
E: 000004.290462 32 04 fe 66 00 00 00 00 10 00 00 00 ff ff 00 00 00 01 7f 00 7f 00 7f 7f 00 00 00 00 00 00 01 00 00
Motion
Trying Roll
# ReportID: 4 / 0xffa00003:   -2 ,  102 ,    0 ,  -79 ,   95 ,    0 ,    0 ,    0 ,    0 ,    0 ,   60 ,   -1 ,  -69 ,   -1 ,   11 ,    0 ,  127 ,  -79 ,  127 ,   -1 ,  127 ,  127 ,    0 ,    0 ,    0 ,    5 ,    0 ,    0 ,  -22 ,  -15 ,    0
E: 000008.921107 32 04 fe 66 00 b1 5f 00 00 00 00 00 3c ff bb ff 0b 00 7f b1 7f ff 7f 7f 00 00 00 05 00 00 ea f1 00
Trying Pitch
# ReportID: 4 / 0xffa00003:   -2 ,  102 ,    0 ,  -24 ,  -49 ,    6 ,    0 ,    0 ,    0 ,    0 ,  -37 ,   -1 ,   10 ,    1 ,  -29 ,    0 ,  127 ,  -24 ,  127 ,   -1 ,  127 ,  127 ,    0 ,    0 ,    0 ,  108 ,    0 ,    0 ,   99 ,   13 ,    0
E: 000003.607853 32 04 fe 66 00 e8 cf 06 00 00 00 00 db ff 0a 01 e3 00 7f e8 7f ff 7f 7f 00 00 00 6c 00 00 63 0d 00
Trying Yaw
# ReportID: 4 / 0xffa00003:   -2 ,  102 ,    0 ,   79 ,    0 ,   -9 ,    0 ,    0 ,    0 ,    0 ,   23 ,    1 ,  -37 ,   -2 ,  -80 ,    0 ,  127 ,   79 ,  127 ,    0 ,  127 ,  127 ,    0 ,    0 ,    0 ,  112 ,   -1 ,    0 ,   31 ,  122 ,    0
E: 000004.710162 32 04 fe 66 00 4f 00 f7 00 00 00 00 17 01 db fe b0 00 7f 4f 7f 00 7f 7f 00 00 00 70 ff 00 1f 7a 00 */

/// report for dinput via dongle
#[derive(PackedStruct, Debug, Copy, Clone, PartialEq)]
#[packed_struct(bit_numbering = "msb0", size_bytes = "31")]
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

    // byte 17
    #[packed_field(bytes = "17", endian = "lsb")]
    pub joystick_l_x: u8,

    // byte 19
    #[packed_field(bytes = "19", endian = "lsb")]
    pub joystick_l_y: u8,

    // byte 21-24
    #[packed_field(bytes = "21", endian = "lsb")]
    pub joystick_r_x: u8,
    #[packed_field(bytes = "22", endian = "lsb")]
    pub joystick_r_y: u8,
    #[packed_field(bytes = "23", endian = "lsb")]
    pub lt_analog: u8,
    #[packed_field(bytes = "24", endian = "lsb")]
    pub rt_analog: u8,
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
            //            tick: Integer::from_primitive(0),
            //            gyro_z: Integer::from_primitive(0),
            //            gyro_y: Integer::from_primitive(0),
            //            gyro_x: Integer::from_primitive(0),
            //            accel_z: Integer::from_primitive(0),
            //            accel_y: Integer::from_primitive(0),
            //            accel_x: Integer::from_primitive(0),
            //            charging: false,
            //            charge_percent: Integer::from_primitive(0),
        }
    }
}
