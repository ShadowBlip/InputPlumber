use packed_struct::prelude::*;

// X/Y are flipped due to panel rotation
pub const TOUCHSCREEN_X_MAX: u16 = 800;
pub const TOUCHSCREEN_Y_MAX: u16 = 1280;

// # ReportID: 1 / Tip Switch: 1 | # | # | Contact Id:    0 | X:    540 ,    540 | Y:    505 ,    505 | Width:     48 | Height:     48
// #             | Tip Switch: 0 | # | # | Contact Id:   15 | X:   4095 ,   4095 | Y:   4095 ,   4095 | Width:     48 | Height:     48
// #             | Tip Switch: 0 | # | # | Contact Id:   15 | X:   4095 ,   4095 | Y:   4095 ,   4095 | Width:     48 | Height:     48
// #             | Tip Switch: 0 | # | # | Contact Id:   15 | X:   4095 ,   4095 | Y:   4095 ,   4095 | Width:     48 | Height:     48 | Scan Time:      0 | Contact Count:    1
// E: 000000.000000 60 01 01 00 1c 02 1c 02 f9 01 f9 01 30 00 30 00 00 0f ff 0f ff 0f ff 0f ff 0f 30 00 30 00 00 0f ff 0f ff 0f ff 0f ff 0f 30 00 30 00 00 0f ff 0f ff 0f ff 0f ff 0f 30 00 30 00 00 00 01
#[derive(PackedStruct, Debug, Copy, Clone, PartialEq)]
#[packed_struct(bit_numbering = "msb0", size_bytes = "60")]
pub struct PackedInputDataReport {
    #[packed_field(bytes = "0")]
    pub report_id: u8,
    #[packed_field(bytes = "1..=14")]
    pub touch1: TouchData,
    #[packed_field(bytes = "15..=28")]
    pub touch2: TouchData,
    #[packed_field(bytes = "29..=42")]
    pub touch3: TouchData,
    #[packed_field(bytes = "43..=56")]
    pub touch4: TouchData,
    #[packed_field(bytes = "57..=58", endian = "lsb")]
    pub scan_time: Integer<u16, packed_bits::Bits<16>>,
    #[packed_field(bytes = "59")]
    pub contact_count: u8,
}

impl Default for PackedInputDataReport {
    fn default() -> Self {
        Self {
            report_id: 1,
            touch1: TouchData::default(),
            touch2: TouchData::default(),
            touch3: TouchData::default(),
            touch4: TouchData::default(),
            scan_time: Integer::from_primitive(0),
            contact_count: 0,
        }
    }
}

// 00 00 29 02 29 02 12 01 12 01 30 00 30 00
#[derive(PackedStruct, Debug, Copy, Clone, PartialEq)]
#[packed_struct(bit_numbering = "msb0", size_bytes = "14")]
pub struct TouchData {
    #[packed_field(bytes = "0")]
    pub tip_switch: u8,
    #[packed_field(bytes = "1")]
    pub contact_id: u8,
    #[packed_field(bytes = "2")]
    pub x_lg: u8,
    #[packed_field(bits = "28..=31", endian = "lsb")]
    pub x_sm: Integer<u8, packed_bits::Bits<4>>,
    #[packed_field(bytes = "4")]
    pub x2_lg: u8,
    #[packed_field(bits = "44..=47", endian = "lsb")]
    pub x2_sm: Integer<u8, packed_bits::Bits<4>>,
    #[packed_field(bytes = "6")]
    pub y_lg: u8,
    #[packed_field(bits = "60..=63", endian = "lsb")]
    pub y_sm: Integer<u8, packed_bits::Bits<4>>,
    #[packed_field(bytes = "8")]
    pub y2_lg: u8,
    #[packed_field(bits = "76..=79", endian = "lsb")]
    pub y2_sm: Integer<u8, packed_bits::Bits<4>>,
    #[packed_field(bytes = "10")]
    pub width: u8,
    #[packed_field(bytes = "11")]
    pub _unused11: u8,
    #[packed_field(bytes = "12")]
    pub height: u8,
    #[packed_field(bytes = "13")]
    pub _unused13: u8,
}

impl TouchData {
    pub fn is_touching(&self) -> bool {
        self.tip_switch == 1
    }

    pub fn set_x(&mut self, value: u16) {
        let large: u8 = (value & 0x00ff) as u8;
        let small: u8 = ((value & 0xff00) >> 8) as u8;
        self.x_lg = large;
        self.x_sm = Integer::from_primitive(small);
    }

    pub fn get_x(&self) -> u16 {
        let small = self.x_sm.to_primitive() as u16;
        let large = self.x_lg as u16;
        small << 8 | large
    }

    pub fn set_x2(&mut self, value: u16) {
        let large: u8 = (value & 0x00ff) as u8;
        let small: u8 = ((value & 0xff00) >> 8) as u8;
        self.x2_lg = large;
        self.x2_sm = Integer::from_primitive(small);
    }

    #[allow(dead_code)]
    pub fn get_x2(&self) -> u16 {
        let small = self.x2_sm.to_primitive() as u16;
        let large = self.x2_lg as u16;
        small << 8 | large
    }

    pub fn set_y(&mut self, value: u16) {
        let large: u8 = (value & 0x00ff) as u8;
        let small: u8 = ((value & 0xff00) >> 8) as u8;
        self.y_lg = large;
        self.y_sm = Integer::from_primitive(small);
    }

    pub fn get_y(&self) -> u16 {
        let small = self.y_sm.to_primitive() as u16;
        let large = self.y_lg as u16;
        small << 8 | large
    }

    pub fn set_y2(&mut self, value: u16) {
        let large: u8 = (value & 0x00ff) as u8;
        let small: u8 = ((value & 0xff00) >> 8) as u8;
        self.y2_lg = large;
        self.y2_sm = Integer::from_primitive(small);
    }

    #[allow(dead_code)]
    pub fn get_y2(&self) -> u16 {
        let small = self.y2_sm.to_primitive() as u16;
        let large = self.y2_lg as u16;
        small << 8 | large
    }
}

// # ReportID: 1 / Tip Switch: 1 | # | # | Contact Id:    0 | X:    540 ,    540 | Y:    505 ,    505 | Width:     48 | Height:     48
// #             | Tip Switch: 0 | # | # | Contact Id:   15 | X:   4095 ,   4095 | Y:   4095 ,   4095 | Width:     48 | Height:     48
impl Default for TouchData {
    fn default() -> Self {
        let mut data = Self {
            tip_switch: 0,
            contact_id: 15,
            x_lg: 0,
            x_sm: Integer::from_primitive(0),
            x2_lg: 0,
            x2_sm: Integer::from_primitive(0),
            y_lg: 0,
            y_sm: Integer::from_primitive(0),
            y2_lg: 0,
            y2_sm: Integer::from_primitive(0),
            width: 48,
            _unused11: 0,
            height: 48,
            _unused13: 0,
        };
        data.set_x(4095);
        data.set_x2(4095);
        data.set_y(4095);
        data.set_y2(4095);

        data
    }
}
