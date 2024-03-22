use packed_struct::{prelude::*, types::SizedInteger, PackedStruct};

use super::driver::*;

pub trait PackedInputDataReport: PackedStruct + Copy + Clone + PartialEq
{
    fn new() -> Self;

    fn set_frame_number(&mut self, num: u8);
} 

#[derive(PackedStruct, Debug, Copy, Clone, PartialEq)]
#[packed_struct(bit_numbering = "msb0", size_bytes = "64")]
pub struct USBPackedInputDataReport {
    #[packed_field(bytes = "0")]
    pub input_report: u8,

    #[packed_field(bytes = "1")]
    pub joystick_l_x: u8,
    #[packed_field(bytes = "2")]
    pub joystick_l_y: u8,
    #[packed_field(bytes = "3")]
    pub joystick_r_x: u8,
    #[packed_field(bytes = "4")]
    pub joystick_r_y: u8,

}

impl PackedInputDataReport for USBPackedInputDataReport {
    /// Return a new empty input data report
    fn new() -> Self {
        Self {
            input_report: INPUT_REPORT_USB,
            joystick_l_x : 127,
            joystick_l_y : 127,
            joystick_r_x : 127,
            joystick_r_y : 127,
        }
    }

    fn set_frame_number(&mut self, num: u8) {

    }
}

impl Default for USBPackedInputDataReport {
    fn default() -> Self {
        Self::new()
    }
}