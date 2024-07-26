#![allow(warnings)]
use packed_struct::prelude::*;

use crate::drivers::grand_unified_controller::update::ValueUpdate;

use super::{
    capability::InputCapability,
    event::Event,
    update::StateUpdate,
    value::{BoolValue, Value},
};

/// Major version of the Unified Controller Input Specification that this
/// implementation supports.
pub const UNIFIED_SPEC_VERSION_MAJOR: u8 = 1;
/// Minor version of the Unified Controller Input Specification that this
/// implementation supports.
pub const UNIFIED_SPEC_VERSION_MINOR: u8 = 0;

pub const INPUT_CAPABILITY_REPORT_SIZE: usize = 1020;
pub const INPUT_DATA_REPORT_SIZE: usize = 68;

/// ReportType contains an enumeration of all possible report types
#[derive(PrimitiveEnum_u8, Clone, Copy, PartialEq, Debug)]
pub enum ReportType {
    Unknown = 0x00,
    InputCapabilityReport = 0x01,
    InputDataReport = 0x02,
    OutputCapabilityReport = 0x03,
    OutputDataReport = 0x04,
}

impl From<u8> for ReportType {
    fn from(value: u8) -> Self {
        match value {
            0x01 => Self::InputCapabilityReport,
            0x02 => Self::InputDataReport,
            0x03 => Self::OutputCapabilityReport,
            0x04 => Self::OutputDataReport,
            _ => Self::Unknown,
        }
    }
}

/// Feature report types
#[derive(PrimitiveEnum_u8, Clone, Copy, PartialEq, Debug)]
pub enum FeatureReportType {
    /// Instruct the driver to return a feature report with the available capabilities
    GetInputCapabilities = 0x01,
    GetOutputCapabilites = 0x02,
    GetName = 0x03,
    GetVendorId = 0x04,
    GetProductId = 0x05,
    GetGlobalProductId = 0x06,
    GetSerial = 0x07,

    // Examples
    InputData = 0x09,
    SetMappings = 0x80,
    ClearMappings = 0x81,
    GetMappings = 0x82,
    GetAttrib = 0x83,
    GetAttribLabel = 0x84,
    DefaultMappings = 0x85,
    FactoryReset = 0x86,
    WriteRegister = 0x87,
    ClearRegister = 0x88,
    ReadRegister = 0x89,
    GetRegisterLabel = 0x8a,
    GetRegisterMax = 0x8b,
    GetRegisterDefault = 0x8c,
    SetMode = 0x8d,
    DefaultMouse = 0x8e,
    TriggerHapticPulse = 0x8f,
    RequestCommStatus = 0xb4,
    //GetSerial = 0xae,
    TriggerHapticCommand = 0xea,
    TriggerRumbleCommand = 0xeb,
}

// Describes how to decode a particular value
#[derive(PrimitiveEnum_u8, Clone, Copy, PartialEq, Debug, Default, Ord, PartialOrd, Eq)]
pub enum ValueType {
    #[default]
    None,
    /// Bool values take up 1 bit in the input report
    Bool,
    /// Uint8 values take up 1 byte in the input report
    UInt8,
    /// Uint16 values take up 2 bytes in the input report
    UInt16,
    /// UInt16Vector2 vales take up 4 bytes in the input report
    UInt16Vector2,
    /// Int16Vector3 vales take up 6 bytes in the input report
    Int16Vector3,
    /// Touch values takes up 6 bytes in the input report
    Touch,
}

impl ValueType {
    /// Returns the size in bits the value type takes up in the input data report
    pub fn size_bits(&self) -> usize {
        match self {
            ValueType::None => 0,
            ValueType::Bool => 1,
            ValueType::UInt8 => 8,
            ValueType::UInt16 => 16,
            ValueType::UInt16Vector2 => 32,
            ValueType::Int16Vector3 => 40,
            ValueType::Touch => 48,
        }
    }
}

/// [InputCapabilityInfo] describes a single input capability that a device supports
/// and how to decode the value in the input data report. A consuming driver can
/// use the offset to look for the value at a specific bit offset and can use the
/// value type to determine how to unpack the value.
#[derive(PackedStruct, Debug, Copy, Clone, PartialEq)]
#[packed_struct(bit_numbering = "msb0", size_bytes = "4")]
pub struct InputCapabilityInfo {
    /// The capability
    #[packed_field(bytes = "0..=1", endian = "lsb", ty = "enum")]
    pub capability: InputCapability,
    /// The type of value this capability emits
    #[packed_field(bytes = "2", ty = "enum")]
    pub value_type: ValueType,
    /// The bit offset in the input report to read the value for this capability.
    #[packed_field(bytes = "3")]
    pub offset: u8,
}

impl Default for InputCapabilityInfo {
    fn default() -> Self {
        Self {
            capability: Default::default(),
            value_type: Default::default(),
            offset: Default::default(),
        }
    }
}

/// The [InputCapabilityReport] describes the input capabilities of the device
/// and how to decode the [InputDataReport].
#[derive(PackedStruct, Debug, Copy, Clone, PartialEq)]
#[packed_struct(bit_numbering = "msb0", size_bytes = "1020")]
pub struct InputCapabilityReport {
    /// Major version indicates whether or not compatibility-breaking changes
    /// have occurred.
    #[packed_field(bytes = "0")]
    pub major_ver: u8,
    /// The minor version indicates what capabilities are available
    #[packed_field(bytes = "1")]
    pub minor_ver: u8,
    /// The report type
    #[packed_field(bytes = "2", ty = "enum")]
    pub report_type: ReportType,

    /// The number of input capabilities the device supports
    #[packed_field(bytes = "3")]
    pub capabilities_count: u8,
    /// The capabilities the device supports
    #[packed_field(bytes = "4..=1019")]
    pub capabilities: [InputCapabilityInfo; 254],
}

impl InputCapabilityReport {
    /// Add the given capability to the capability report
    pub fn add_capability(&mut self, capability: InputCapabilityInfo) {
        // Don't add empty capabilities
        if capability.value_type == ValueType::None {
            return;
        }
        if capability.capability == InputCapability::NONE {
            return;
        }
        if self.capabilities_count > 253 {
            // TODO: Add errors
            // Max capabilities reached
            return;
        }

        // Increment the capability count and add the capability
        self.capabilities_count += 1; // TODO: Handle wrapping errors if > 255
        let idx = (self.capabilities_count - 1) as usize;
        self.capabilities[idx] = capability;

        // Update the bit offset of all capabilities based on the value type
        let mut offset = 0;
        for cap in self.capabilities.iter_mut() {
            if cap.value_type == ValueType::None {
                break;
            }

            // Non-bool values should be byte-aligned
            let offset_remainder = offset % 8;
            if cap.value_type != ValueType::Bool && offset_remainder != 0 {
                offset += offset_remainder;
            }

            // Update the offset
            cap.offset = offset;
            let value_size = cap.value_type.size_bits() as u8;
            offset += value_size;
        }
    }

    /// Get the capability information for the given capability
    pub fn get_capability(&self, capability: InputCapability) -> Option<InputCapabilityInfo> {
        for cap in self.capabilities {
            if cap.capability == InputCapability::NONE {
                return None;
            }
            if cap.capability == capability {
                return Some(cap);
            }
        }
        None
    }

    /// Decode the given [InputDataReport] according to the capability report
    pub fn decode_data_report(&self, data: &InputDataReport) -> Vec<Value> {
        let mut values = Vec::with_capacity(self.capabilities_count as usize);
        for capability_info in self.capabilities {
            // Get the value type and its size
            let value_type = capability_info.value_type;
            let value_size_bits = value_type.size_bits();
            let value_size_bytes = value_size_bits + 7 & !7;

            // Get the bit/byte offset
            let offset_bits = capability_info.offset as usize;
            let offset_bytes = offset_bits + 7 & !7;

            // Get the byte start and end
            let byte_start = offset_bytes;
            let byte_end = byte_start + value_size_bytes;

            let value = match value_type {
                ValueType::None => Value::None,
                ValueType::Bool => {
                    // TODO: bit shift the value for bit fields
                    let slice = &data.data[byte_start..byte_end];
                    let buffer = slice.try_into().unwrap(); // TODO: don't unwrap
                    let value = BoolValue::unpack(buffer).unwrap();
                    Value::Bool(value)
                }
                ValueType::UInt8 => todo!(),
                ValueType::UInt16 => todo!(),
                ValueType::UInt16Vector2 => todo!(),
                ValueType::Int16Vector3 => todo!(),
                ValueType::Touch => todo!(),
            };
            values.push(value);
        }

        values
    }
}

impl Default for InputCapabilityReport {
    fn default() -> Self {
        Self {
            major_ver: UNIFIED_SPEC_VERSION_MAJOR,
            minor_ver: UNIFIED_SPEC_VERSION_MINOR,
            report_type: ReportType::InputCapabilityReport,
            capabilities_count: Default::default(),
            capabilities: [InputCapabilityInfo::default(); 254],
        }
    }
}

/// The [InputDataReport] contains input data based on the capabilities of the
/// device according to the [InputCapabilityReport].
#[derive(PackedStruct, Debug, Copy, Clone, PartialEq)]
#[packed_struct(bit_numbering = "msb0", size_bytes = "68")]
pub struct InputDataReport {
    // byte 0-3
    #[packed_field(bytes = "0")]
    pub major_ver: u8, // Major version? Always 0x01
    #[packed_field(bytes = "1")]
    pub minor_ver: u8, // Minor version? Always 0x00
    #[packed_field(bytes = "2", ty = "enum")]
    pub report_type: ReportType, // Report type? Always 0x09

    /// The state version will increment whenever a state change has occurred.
    /// If the version has not changed since the last report, there is no need
    /// to further process the report.
    #[packed_field(bytes = "4..=5", endian = "lsb")]
    pub state_version: Integer<u16, packed_bits::Bits<16>>,

    /// Input data can be decoded by reading capabilities in the [InputCapabilityReport].
    #[packed_field(bytes = "6..=67")]
    pub data: [u8; 62],
}

impl Default for InputDataReport {
    fn default() -> Self {
        Self {
            major_ver: UNIFIED_SPEC_VERSION_MAJOR,
            minor_ver: UNIFIED_SPEC_VERSION_MINOR,
            report_type: ReportType::InputDataReport,
            state_version: Default::default(),
            data: [0; 62],
        }
    }
}

impl InputDataReport {
    /// Update the [InputDataReport] with the given value based on the [InputCapabilityReport]
    pub fn update(&mut self, capability_report: &InputCapabilityReport, update: StateUpdate) {
        let Some(capability_info) = capability_report.get_capability(update.capability) else {
            // TODO: Add errors
            // Not found
            return;
        };

        // Validate that the event value matches the value type
        if capability_info.value_type != update.value.value_type() {
            // Invalid value type for capability
            return;
        }

        // Get the value type and its size
        let value_type = capability_info.value_type;
        let value_size_bits = value_type.size_bits();
        let value_size_bytes = if value_size_bits == 1 {
            1
        } else {
            value_size_bits / 8
        };

        // Get the bit/byte offset
        let offset_bits = capability_info.offset as usize;
        let offset_remainder = offset_bits % 8;
        let offset_bytes = (offset_bits - offset_remainder) / 8;

        // Get the byte start and end
        let byte_start = offset_bytes;
        let byte_end = byte_start + value_size_bytes;

        // Ensure the data can fit in the report
        if byte_start + value_size_bytes > self.data.len() {
            return;
        }

        match update.value {
            ValueUpdate::None => (),
            ValueUpdate::Bool(value) => {
                // Shift the value to the appropriate bit offset
                if value.value {
                    // Toggle on the bit at the offset remainder
                    self.data[byte_start] = 1 << offset_remainder | self.data[byte_start];
                } else {
                    // Toggle off the bit at the offset remainder
                    self.data[byte_start] = 1 << offset_remainder ^ self.data[byte_start];
                }
            }
            ValueUpdate::UInt8(value) => {
                self.data[byte_start] = value.value;
            }
            ValueUpdate::UInt16(value) => {
                let data = value.value.to_le_bytes();
                self.data[byte_start] = data[0];
                self.data[byte_start + 1] = data[1];
            }
            ValueUpdate::UInt16Vector2(value) => {
                let x_data = value.x.map(|x| x.to_le_bytes());
                if let Some(data) = x_data {
                    self.data[byte_start] = data[0];
                    self.data[byte_start + 1] = data[1];
                }

                let y_data = value.y.map(|y| y.to_le_bytes());
                if let Some(data) = y_data {
                    self.data[byte_start + 2] = data[0];
                    self.data[byte_start + 3] = data[1];
                }
            }
            ValueUpdate::Int16Vector3(value) => {
                let x_data = value.x.map(|x| x.to_le_bytes());
                if let Some(data) = x_data {
                    self.data[byte_start] = data[0];
                    self.data[byte_start + 1] = data[1];
                }

                let y_data = value.y.map(|y| y.to_le_bytes());
                if let Some(data) = y_data {
                    self.data[byte_start + 2] = data[0];
                    self.data[byte_start + 3] = data[1];
                }

                let z_data = value.z.map(|z| z.to_le_bytes());
                if let Some(data) = z_data {
                    self.data[byte_start + 4] = data[0];
                    self.data[byte_start + 5] = data[1];
                }
            }
            ValueUpdate::Touch(value) => {
                let data = value.pack().unwrap(); // TODO: don't unwrap
                self.data[byte_start] = data[0];
                self.data[byte_start + 1] = data[1];
                self.data[byte_start + 2] = data[2];
                self.data[byte_start + 3] = data[3];
                self.data[byte_start + 4] = data[4];
                self.data[byte_start + 5] = data[5];
            }
        };

        log::trace!("Updated data: {:?}", self.data);
    }
}

/*
#[derive(PackedStruct, Debug, Copy, Clone, PartialEq)]
#[packed_struct(bit_numbering = "msb0", size_bytes = "64")]
pub struct PackedInputDataReport {
    // byte 0-3
    #[packed_field(bytes = "0")]
    pub major_ver: u8, // Major version? Always 0x01
    #[packed_field(bytes = "1")]
    pub minor_ver: u8, // Minor version? Always 0x00
    #[packed_field(bytes = "2")]
    pub report_type: u8, // Report type? Always 0x09
    #[packed_field(bytes = "3")]
    pub report_size: u8, // Actual data length of report in bytes.  Always 64 for input reports.

    // byte 4-7
    #[packed_field(bytes = "4..=7", endian = "lsb")]
    pub frame: Integer<u32, packed_bits::Bits<32>>, // Input frame counter?

    // byte 8
    #[packed_field(bits = "64")]
    pub a: bool, // Button cluster
    #[packed_field(bits = "65")]
    pub x: bool,
    #[packed_field(bits = "66")]
    pub b: bool,
    #[packed_field(bits = "67")]
    pub y: bool,
    #[packed_field(bits = "68")]
    pub l1: bool, // Shoulder buttons
    #[packed_field(bits = "69")]
    pub r1: bool,
    #[packed_field(bits = "70")]
    pub l2: bool,
    #[packed_field(bits = "71")]
    pub r2: bool, // Binary sensor for analog triggers

    // byte 9
    #[packed_field(bits = "72")]
    pub l5: bool, // L5 & R5 on the back of the deck
    #[packed_field(bits = "73")]
    pub menu: bool, // Hamburger (☰) button located above right stick
    #[packed_field(bits = "74")]
    pub steam: bool, // STEAM button below left trackpad
    #[packed_field(bits = "75")]
    pub options: bool, // Overlapping square ⧉  button located above left stick
    #[packed_field(bits = "76")]
    pub down: bool,
    #[packed_field(bits = "77")]
    pub left: bool,
    #[packed_field(bits = "78")]
    pub right: bool,
    #[packed_field(bits = "79")]
    pub up: bool, // Directional Pad buttons

    // byte 10
    #[packed_field(bits = "80")]
    pub _unk4: bool,
    #[packed_field(bits = "81")]
    pub l3: bool, // Z-axis button on the left stick
    #[packed_field(bits = "82")]
    pub _unk3: bool,
    #[packed_field(bits = "83")]
    pub r_pad_touch: bool, // Binary "touch" sensor for trackpads
    #[packed_field(bits = "84")]
    pub l_pad_touch: bool,
    #[packed_field(bits = "85")]
    pub r_pad_press: bool, // Binary "press" sensor for trackpads
    #[packed_field(bits = "86")]
    pub l_pad_press: bool,
    #[packed_field(bits = "87")]
    pub r5: bool,

    // byte 11
    #[packed_field(bits = "88")]
    pub _unk11: bool,
    #[packed_field(bits = "89")]
    pub _unk10: bool,
    #[packed_field(bits = "90")]
    pub _unk9: bool,
    #[packed_field(bits = "91")]
    pub _unk8: bool,
    #[packed_field(bits = "92")]
    pub _unk7: bool,
    #[packed_field(bits = "93")]
    pub r3: bool, // Z-axis button on the right stick
    #[packed_field(bits = "94")]
    pub _unk6: bool,
    #[packed_field(bits = "95")]
    pub _unk5: bool,

    // byte 12
    #[packed_field(bits = "96")]
    pub _unk19: bool,
    #[packed_field(bits = "97")]
    pub _unk18: bool,
    #[packed_field(bits = "98")]
    pub _unk17: bool,
    #[packed_field(bits = "99")]
    pub _unk16: bool,
    #[packed_field(bits = "100")]
    pub _unk15: bool,
    #[packed_field(bits = "101")]
    pub _unk14: bool,
    #[packed_field(bits = "102")]
    pub _unk13: bool,
    #[packed_field(bits = "103")]
    pub _unk12: bool,

    // byte 13
    #[packed_field(bits = "104")]
    pub r_stick_touch: bool, // Binary touch sensors on the stick controls
    #[packed_field(bits = "105")]
    pub l_stick_touch: bool,
    #[packed_field(bits = "106")]
    pub _unk23: bool,
    #[packed_field(bits = "107")]
    pub _unk22: bool,
    #[packed_field(bits = "108")]
    pub _unk21: bool,
    #[packed_field(bits = "109")]
    pub r4: bool,
    #[packed_field(bits = "110")]
    pub l4: bool, // L4 & R4 on the back of the deck
    #[packed_field(bits = "111")]
    pub _unk20: bool,

    // byte 14
    #[packed_field(bits = "112")]
    pub _unk30: bool,
    #[packed_field(bits = "113")]
    pub _unk29: bool,
    #[packed_field(bits = "114")]
    pub _unk28: bool,
    #[packed_field(bits = "115")]
    pub _unk27: bool,
    #[packed_field(bits = "116")]
    pub _unk26: bool,
    #[packed_field(bits = "117")]
    pub quick_access: bool, // Quick Access (...) button below right trackpad
    #[packed_field(bits = "118")]
    pub _unk25: bool,
    #[packed_field(bits = "119")]
    pub _unk24: bool,

    // byte 15
    #[packed_field(bytes = "15")]
    pub _unk31: u8, // Not sure, maybe padding or just unused

    // byte 16-23
    #[packed_field(bytes = "16..=17", endian = "lsb")]
    pub l_pad_x: Integer<i16, packed_bits::Bits<16>>, // Trackpad touch coordinates
    #[packed_field(bytes = "18..=19", endian = "lsb")]
    pub l_pad_y: Integer<i16, packed_bits::Bits<16>>,
    #[packed_field(bytes = "20..=21", endian = "lsb")]
    pub r_pad_x: Integer<i16, packed_bits::Bits<16>>,
    #[packed_field(bytes = "22..=23", endian = "lsb")]
    pub r_pad_y: Integer<i16, packed_bits::Bits<16>>,

    // byte 24-29
    #[packed_field(bytes = "24..=25", endian = "lsb")]
    pub accel_x: Integer<i16, packed_bits::Bits<16>>, // Accelerometers
    #[packed_field(bytes = "26..=27", endian = "lsb")]
    pub accel_y: Integer<i16, packed_bits::Bits<16>>,
    #[packed_field(bytes = "28..=29", endian = "lsb")]
    pub accel_z: Integer<i16, packed_bits::Bits<16>>,

    // byte 30-35
    #[packed_field(bytes = "30..=31", endian = "lsb")]
    pub pitch: Integer<i16, packed_bits::Bits<16>>, // Gyro
    #[packed_field(bytes = "32..=33", endian = "lsb")]
    pub yaw: Integer<i16, packed_bits::Bits<16>>,
    #[packed_field(bytes = "34..=35", endian = "lsb")]
    pub roll: Integer<i16, packed_bits::Bits<16>>,

    // byte 36-43
    #[packed_field(bytes = "36..=37", endian = "lsb")]
    pub _magn_0: Integer<i16, packed_bits::Bits<16>>, // Magnetometer
    #[packed_field(bytes = "38..=39", endian = "lsb")]
    pub _magn_1: Integer<i16, packed_bits::Bits<16>>,
    #[packed_field(bytes = "40..=41", endian = "lsb")]
    pub _magn_2: Integer<i16, packed_bits::Bits<16>>,
    #[packed_field(bytes = "42..=43", endian = "lsb")]
    pub _magn_3: Integer<i16, packed_bits::Bits<16>>,

    // byte 44-47
    #[packed_field(bytes = "44..=45", endian = "lsb")]
    pub l_trigg: Integer<u16, packed_bits::Bits<16>>, // Pressure sensors for L2 & R2 triggers
    #[packed_field(bytes = "46..=47", endian = "lsb")]
    pub r_trigg: Integer<u16, packed_bits::Bits<16>>,

    // byte 48-55
    #[packed_field(bytes = "48..=49", endian = "lsb")]
    pub l_stick_x: Integer<i16, packed_bits::Bits<16>>, // Analogue thumbsticks
    #[packed_field(bytes = "50..=51", endian = "lsb")]
    pub l_stick_y: Integer<i16, packed_bits::Bits<16>>,
    #[packed_field(bytes = "52..=53", endian = "lsb")]
    pub r_stick_x: Integer<i16, packed_bits::Bits<16>>,
    #[packed_field(bytes = "54..=55", endian = "lsb")]
    pub r_stick_y: Integer<i16, packed_bits::Bits<16>>,

    // byte 56-59
    #[packed_field(bytes = "56..=57", endian = "lsb")]
    pub l_pad_force: Integer<u16, packed_bits::Bits<16>>,
    #[packed_field(bytes = "58..=59", endian = "lsb")]
    pub r_pad_force: Integer<u16, packed_bits::Bits<16>>,

    // byte 60-63
    #[packed_field(bytes = "60..=61", endian = "lsb")]
    pub l_stick_force: Integer<u16, packed_bits::Bits<16>>, // Thumbstick capacitive sensors
    #[packed_field(bytes = "62..=63", endian = "lsb")]
    pub r_stick_force: Integer<u16, packed_bits::Bits<16>>,
    // 64 Bytes total
}

impl PackedInputDataReport {
    /// Return a new empty input data report
    pub fn new() -> Self {
        PackedInputDataReport {
            major_ver: 0x01,
            minor_ver: 0x00,
            report_type: 0x09,
            report_size: 64,
            frame: Integer::from_primitive(0),
            a: false,
            x: false,
            b: false,
            y: false,
            l1: false,
            r1: false,
            l2: false,
            r2: false,
            l5: false,
            menu: false,
            steam: false,
            options: false,
            down: false,
            left: false,
            right: false,
            up: false,
            _unk4: false,
            l3: false,
            _unk3: false,
            r_pad_touch: false,
            l_pad_touch: false,
            r_pad_press: false,
            l_pad_press: false,
            r5: false,
            _unk11: false,
            _unk10: false,
            _unk9: false,
            _unk8: false,
            _unk7: false,
            r3: false,
            _unk6: false,
            _unk5: false,
            _unk19: false,
            _unk18: false,
            _unk17: false,
            _unk16: false,
            _unk15: false,
            _unk14: false,
            _unk13: false,
            _unk12: false,
            r_stick_touch: false,
            l_stick_touch: false,
            _unk23: false,
            _unk22: false,
            _unk21: false,
            r4: false,
            l4: false,
            _unk20: false,
            _unk30: false,
            _unk29: false,
            _unk28: false,
            _unk27: false,
            _unk26: false,
            quick_access: false,
            _unk25: false,
            _unk24: false,
            _unk31: 0,
            l_pad_x: Integer::from_primitive(0),
            l_pad_y: Integer::from_primitive(0),
            r_pad_x: Integer::from_primitive(0),
            r_pad_y: Integer::from_primitive(0),
            accel_x: Integer::from_primitive(0),
            accel_y: Integer::from_primitive(0),
            accel_z: Integer::from_primitive(0),
            pitch: Integer::from_primitive(0),
            yaw: Integer::from_primitive(0),
            roll: Integer::from_primitive(0),
            _magn_0: Integer::from_primitive(0),
            _magn_1: Integer::from_primitive(0),
            _magn_2: Integer::from_primitive(0),
            _magn_3: Integer::from_primitive(0),
            l_trigg: Integer::from_primitive(0),
            r_trigg: Integer::from_primitive(0),
            l_stick_x: Integer::from_primitive(0),
            l_stick_y: Integer::from_primitive(0),
            r_stick_x: Integer::from_primitive(0),
            r_stick_y: Integer::from_primitive(0),
            l_pad_force: Integer::from_primitive(0),
            r_pad_force: Integer::from_primitive(0),
            l_stick_force: Integer::from_primitive(0),
            r_stick_force: Integer::from_primitive(0),
        }
    }
}

impl Default for PackedInputDataReport {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(PrimitiveEnum_u8, Clone, Copy, PartialEq, Debug)]
pub enum Pad {
    Left = 0,
    Right = 1,
    Both = 2,
}

/*
 * Send a haptic pulse to the trackpads
 * Duration and interval are measured in microseconds, count is the number
 * of pulses to send for duration time with interval microseconds between them
 * and gain is measured in decibels, ranging from -24 to +6
 */
#[derive(PackedStruct, Debug, Copy, Clone, PartialEq)]
#[packed_struct(bit_numbering = "msb0")]
pub struct PackedHapticPulseReport {
    #[packed_field(bytes = "0")]
    pub report_id: u8,
    #[packed_field(bytes = "1")]
    pub report_size: u8,
    #[packed_field(bytes = "2", ty = "enum")]
    pub side: Pad,
    #[packed_field(bytes = "3..=4", endian = "lsb")]
    pub amplitude: Integer<u16, packed_bits::Bits<16>>,
    #[packed_field(bytes = "5..=6", endian = "lsb")]
    pub period: Integer<u16, packed_bits::Bits<16>>,
    #[packed_field(bytes = "7..=8", endian = "lsb")]
    pub count: Integer<u16, packed_bits::Bits<16>>,
}

impl PackedHapticPulseReport {
    pub fn new() -> Self {
        Self {
            report_id: ReportType::TriggerHapticPulse as u8,
            report_size: 9,
            side: Pad::Both,
            amplitude: Integer::from_primitive(0),
            period: Integer::from_primitive(0),
            count: Integer::from_primitive(0),
        }
    }
}

impl Default for PackedHapticPulseReport {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(PackedStruct, Debug, Copy, Clone, PartialEq)]
#[packed_struct(bit_numbering = "msb0")]
pub struct PackedRumbleReport {
    #[packed_field(bytes = "0")]
    pub report_id: u8,
    #[packed_field(bytes = "1")]
    pub report_size: u8,
    #[packed_field(bytes = "3..=4", endian = "lsb")]
    pub intensity: Integer<u16, packed_bits::Bits<16>>,
    #[packed_field(bytes = "5..=6", endian = "lsb")]
    pub left_speed: Integer<u16, packed_bits::Bits<16>>,
    #[packed_field(bytes = "7..=8", endian = "lsb")]
    pub right_speed: Integer<u16, packed_bits::Bits<16>>,
    /// Max gain: 135
    #[packed_field(bytes = "9")]
    pub left_gain: u8,
    /// Max gain: 135
    #[packed_field(bytes = "10")]
    pub right_gain: u8,
}

impl PackedRumbleReport {
    pub fn new() -> Self {
        Self {
            report_id: ReportType::TriggerRumbleCommand as u8,
            report_size: 9,
            intensity: Integer::from_primitive(1),
            left_speed: Integer::from_primitive(0),
            right_speed: Integer::from_primitive(0),
            left_gain: 130,
            right_gain: 130,
        }
    }
}

impl Default for PackedRumbleReport {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(PackedStruct, Debug, Copy, Clone, PartialEq)]
#[packed_struct(bit_numbering = "msb0", size_bytes = "64")]
pub struct PackedMappingsReport {
    #[packed_field(bytes = "0")]
    pub report_id: u8,
}
*/
