//! Sources:
//! - https://github.com/dekuNukem/Nintendo_Switch_Reverse_Engineering/blob/master/bluetooth_hid_notes.md
//! - https://github.com/torvalds/linux/blob/master/drivers/hid/hid-nintendo.c
//! - https://switchbrew.org/w/index.php?title=Joy-Con
use packed_struct::prelude::*;

#[derive(PrimitiveEnum_u8, Clone, Copy, PartialEq, Debug)]
pub enum ReportType {
    CommandOutputReport = 0x01,
    McuUpdateOutputReport = 0x03,
    BasicOutputReport = 0x10,
    McuOutputReport = 0x11,
    AttachmentOutputReport = 0x12,
    CommandInputReport = 0x21,
    McuUpdateInputReport = 0x23,
    BasicInputReport = 0x30,
    McuInputReport = 0x31,
    AttachmentInputReport = 0x32,
    _Unused1 = 0x33,
    GenericInputReport = 0x3F,
    OtaEnableFwuReport = 0x70,
    OtaSetupReadReport = 0x71,
    OtaReadReport = 0x72,
    OtaWriteReport = 0x73,
    OtaEraseReport = 0x74,
    OtaLaunchReport = 0x75,
    ExtGripOutputReport = 0x80,
    ExtGripInputReport = 0x81,
    _Unused2 = 0x82,
}

impl TryFrom<u8> for ReportType {
    type Error = &'static str;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x01 => Ok(Self::CommandOutputReport),
            0x03 => Ok(Self::McuUpdateOutputReport),
            0x10 => Ok(Self::BasicOutputReport),
            0x11 => Ok(Self::McuOutputReport),
            0x12 => Ok(Self::AttachmentOutputReport),
            0x21 => Ok(Self::CommandInputReport),
            0x23 => Ok(Self::McuUpdateInputReport),
            0x30 => Ok(Self::BasicInputReport),
            0x31 => Ok(Self::McuInputReport),
            0x32 => Ok(Self::AttachmentInputReport),
            0x33 => Ok(Self::_Unused1),
            0x3F => Ok(Self::GenericInputReport),
            0x70 => Ok(Self::OtaEnableFwuReport),
            0x71 => Ok(Self::OtaSetupReadReport),
            0x72 => Ok(Self::OtaReadReport),
            0x73 => Ok(Self::OtaWriteReport),
            0x74 => Ok(Self::OtaEraseReport),
            0x75 => Ok(Self::OtaLaunchReport),
            0x80 => Ok(Self::ExtGripOutputReport),
            0x81 => Ok(Self::ExtGripInputReport),
            0x82 => Ok(Self::_Unused2),
            _ => Err("Invalid report type"),
        }
    }
}

#[derive(PrimitiveEnum_u8, Clone, Copy, PartialEq, Debug)]
pub enum BatteryLevel {
    Empty = 0,
    Critical = 1,
    Low = 2,
    Medium = 3,
    Full = 4,
}

#[derive(PackedStruct, Debug, Copy, Clone, PartialEq)]
#[packed_struct(bit_numbering = "msb0", size_bytes = "1")]
pub struct BatteryConnection {
    /// Battery level. 8=full, 6=medium, 4=low, 2=critical, 0=empty. LSB=Charging.
    #[packed_field(bits = "0..=2", ty = "enum")]
    pub battery_level: BatteryLevel,
    #[packed_field(bits = "3")]
    pub charging: bool,
    /// Connection info. (con_info >> 1) & 3 - 3=JC, 0=Pro/ChrGrip. con_info & 1 - 1=Switch/USB powered.
    #[packed_field(bits = "4..=7")]
    pub conn_info: u8,
}

#[derive(PackedStruct, Debug, Copy, Clone, PartialEq)]
#[packed_struct(bit_numbering = "msb0", size_bytes = "3")]
pub struct ButtonStatus {
    // byte 0 (Right)
    #[packed_field(bits = "7")]
    pub y: bool,
    #[packed_field(bits = "6")]
    pub x: bool,
    #[packed_field(bits = "5")]
    pub b: bool,
    #[packed_field(bits = "4")]
    pub a: bool,
    #[packed_field(bits = "3")]
    pub sr_right: bool,
    #[packed_field(bits = "2")]
    pub sl_right: bool,
    #[packed_field(bits = "1")]
    pub r: bool,
    #[packed_field(bits = "0")]
    pub zr: bool,

    // byte 1 (Shared)
    #[packed_field(bits = "15")]
    pub minus: bool,
    #[packed_field(bits = "14")]
    pub plus: bool,
    #[packed_field(bits = "13")]
    pub r_stick: bool,
    #[packed_field(bits = "12")]
    pub l_stick: bool,
    #[packed_field(bits = "11")]
    pub home: bool,
    #[packed_field(bits = "10")]
    pub capture: bool,
    #[packed_field(bits = "9")]
    pub _unused: bool,
    #[packed_field(bits = "8")]
    pub charging_grip: bool,

    // byte 2 (Left)
    #[packed_field(bits = "23")]
    pub down: bool,
    #[packed_field(bits = "22")]
    pub up: bool,
    #[packed_field(bits = "21")]
    pub right: bool,
    #[packed_field(bits = "20")]
    pub left: bool,
    #[packed_field(bits = "19")]
    pub sr_left: bool,
    #[packed_field(bits = "18")]
    pub sl_left: bool,
    #[packed_field(bits = "17")]
    pub l: bool,
    #[packed_field(bits = "16")]
    pub zl: bool,
}

#[derive(PackedStruct, Debug, Copy, Clone, PartialEq)]
#[packed_struct(bit_numbering = "msb0", size_bytes = "3")]
pub struct StickData {
    /// Analog stick X-axis
    #[packed_field(bytes = "0", endian = "lsb")]
    pub x_lo: u8,
    #[packed_field(bits = "8", endian = "lsb")]
    pub x_hi: u8,
    /// Analog stick Y-axis
    #[packed_field(bits = "9..=11", endian = "lsb")]
    pub y_lo: Integer<u8, packed_bits::Bits<3>>,
    #[packed_field(bits = "12..=23", endian = "lsb")]
    pub y_hi: Integer<i16, packed_bits::Bits<12>>,
}

impl StickData {
    pub fn get_x(&self) -> i16 {
        let x_lo = self.x_lo as i16;
        let x_hi = (self.x_hi as i16).rotate_left(8);
        x_lo | x_hi
    }
}

/// The 6-Axis data is repeated 3 times. On Joy-con with a 15ms packet push,
/// this is translated to 5ms difference sampling. E.g. 1st sample 0ms, 2nd 5ms,
/// 3rd 10ms. Using all 3 samples let you have a 5ms precision instead of 15ms.
#[derive(PackedStruct, Debug, Copy, Clone, PartialEq)]
#[packed_struct(bit_numbering = "msb0", size_bytes = "12")]
pub struct ImuData {
    #[packed_field(bytes = "0..=1", endian = "lsb")]
    pub accel_x: Integer<i16, packed_bits::Bits<16>>,
    #[packed_field(bytes = "2..=3", endian = "lsb")]
    pub accel_y: Integer<i16, packed_bits::Bits<16>>,
    #[packed_field(bytes = "4..=5", endian = "lsb")]
    pub accel_z: Integer<i16, packed_bits::Bits<16>>,
    #[packed_field(bytes = "6..=7", endian = "lsb")]
    pub gyro_x: Integer<i16, packed_bits::Bits<16>>,
    #[packed_field(bytes = "8..=9", endian = "lsb")]
    pub gyro_y: Integer<i16, packed_bits::Bits<16>>,
    #[packed_field(bytes = "10..=11", endian = "lsb")]
    pub gyro_z: Integer<i16, packed_bits::Bits<16>>,
}

#[derive(PackedStruct, Debug, Copy, Clone, PartialEq)]
#[packed_struct(bit_numbering = "msb0", size_bytes = "64")]
pub struct PackedInputDataReport {
    // byte 0-2
    /// Input report ID
    #[packed_field(bytes = "0", ty = "enum")]
    pub id: ReportType,
    /// Timer. Increments very fast. Can be used to estimate excess Bluetooth latency.
    #[packed_field(bytes = "1")]
    pub timer: u8,
    /// Battery and connection information
    #[packed_field(bytes = "2")]
    pub info: BatteryConnection,

    // byte 3-5
    /// Button status
    #[packed_field(bytes = "3..=5")]
    pub buttons: ButtonStatus,

    // byte 6-11
    /// Left analog stick
    #[packed_field(bytes = "6..=8")]
    pub left_stick: StickData,
    /// Right analog stick
    #[packed_field(bytes = "9..=11")]
    pub right_stick: StickData,

    // byte 12
    /// Vibrator input report. Decides if next vibration pattern should be sent.
    #[packed_field(bytes = "12")]
    pub vibrator_report: u8,
}
