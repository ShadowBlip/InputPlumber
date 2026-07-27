use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A [HidrawConfig] defines how to decode a particular event in an HID input
/// report.
#[derive(Debug, Deserialize, Serialize, Clone, JsonSchema, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub struct HidrawConfig {
    /// Optional report ID of the input report. This is typically the first byte
    /// of the input report.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub report_id: Option<u8>,
    /// Data type of the input. This is used to decode the value of the input
    /// report.
    pub value_type: ValueType,
    /// The byte where the data begins
    pub byte_start: usize,
    /// Optional maximum value used for normalizing the value. InputPlumber
    /// typically normalizes input values from 0.0 - 1.0 or from -1.0 - 1.0.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_value: Option<i64>,
    /// Optional minimum value used for normalizing the value. InputPlumber
    /// typically normalizes input values from 0.0 - 1.0 or from -1.0 - 1.0.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_value: Option<i64>,
    /// Optional bit offset to start reading from. Only used with
    /// `value_type: bool` — other value types are decoded byte-granular.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bit_offset: Option<u8>,
    /// Optional endianness of the value being decoded. Defaults to LSB.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endian: Option<Endianness>,
}

/// Endianness is the order in which a multi-byte number is represented.
#[derive(Default, Debug, Deserialize, Serialize, Clone, JsonSchema, PartialEq)]
pub enum Endianness {
    /// Least significant byte ordering
    #[default]
    #[serde(rename = "lsb")]
    Lsb,
    /// Most significant byte ordering
    #[serde(rename = "msb")]
    Msb,
}

#[derive(Debug, Deserialize, Serialize, Clone, JsonSchema, PartialEq, Default)]
pub enum ValueType {
    /// Bool values take up 1 bit in the input report
    #[serde(rename = "bool")]
    #[default]
    Bool,

    /// Uint8 values take up 1 byte in the input report
    #[serde(rename = "uint8")]
    UInt8,
    /// Uint16 values take up 2 bytes in the input report
    #[serde(rename = "uint16")]
    UInt16,
    /// Uint32 values take up 4 bytes in the input report
    #[serde(rename = "uint32")]
    UInt32,
    /// Int8 values take up 1 byte in the input report
    #[serde(rename = "int8")]
    Int8,
    /// Int16 values take up 2 bytes in the input report
    #[serde(rename = "int16")]
    Int16,
    /// Int32 values take up 4 bytes in the input report
    #[serde(rename = "int32")]
    Int32,

    /// UInt8Vector2 values take up 2 bytes in the input report
    #[serde(rename = "vector2_uint8")]
    UInt8Vector2,
    /// UInt16Vector2 values take up 4 bytes in the input report
    #[serde(rename = "vector2_uint16")]
    UInt16Vector2,
    /// UInt32Vector2 values take up 8 bytes in the input report
    #[serde(rename = "vector2_uint32")]
    UInt32Vector2,
    /// Int8Vector2 values take up 2 bytes in the input report
    #[serde(rename = "vector2_int8")]
    Int8Vector2,
    /// Int16Vector2 values take up 4 bytes in the input report
    #[serde(rename = "vector2_int16")]
    Int16Vector2,
    /// Int32Vector2 values take up 8 bytes in the input report
    #[serde(rename = "vector2_int32")]
    Int32Vector2,

    /// UInt8Vector3 values take up 3 bytes in the input report
    #[serde(rename = "vector3_uint8")]
    UInt8Vector3,
    /// UInt16Vector3 values take up 6 bytes in the input report
    #[serde(rename = "vector3_uint16")]
    UInt16Vector3,
    /// UInt32Vector3 values take up 12 bytes in the input report
    #[serde(rename = "vector3_uint32")]
    UInt32Vector3,
    /// Int8Vector3 values take up 3 bytes in the input report
    #[serde(rename = "vector3_int8")]
    Int8Vector3,
    /// Int16Vector3 values take up 6 bytes in the input report
    #[serde(rename = "vector3_int16")]
    Int16Vector3,
    /// Int32Vector3 values take up 12 bytes in the input report
    #[serde(rename = "vector3_int32")]
    Int32Vector3,
}

impl ValueType {
    /// Whether this value type is signed.
    pub fn is_signed(&self) -> bool {
        matches!(
            self,
            ValueType::Int8
                | ValueType::Int16
                | ValueType::Int32
                | ValueType::Int8Vector2
                | ValueType::Int16Vector2
                | ValueType::Int32Vector2
                | ValueType::Int8Vector3
                | ValueType::Int16Vector3
                | ValueType::Int32Vector3
        )
    }

    /// Number of values in this type (1 for scalars, 2 or 3 for vectors).
    pub fn dimensions(&self) -> usize {
        match self {
            ValueType::Bool
            | ValueType::UInt8
            | ValueType::UInt16
            | ValueType::UInt32
            | ValueType::Int8
            | ValueType::Int16
            | ValueType::Int32 => 1,
            ValueType::UInt8Vector2
            | ValueType::UInt16Vector2
            | ValueType::UInt32Vector2
            | ValueType::Int8Vector2
            | ValueType::Int16Vector2
            | ValueType::Int32Vector2 => 2,
            ValueType::UInt8Vector3
            | ValueType::UInt16Vector3
            | ValueType::UInt32Vector3
            | ValueType::Int8Vector3
            | ValueType::Int16Vector3
            | ValueType::Int32Vector3 => 3,
        }
    }

    /// Size in bytes of a single value of this type.
    pub fn component_size(&self) -> usize {
        match self {
            ValueType::UInt8
            | ValueType::Int8
            | ValueType::UInt8Vector2
            | ValueType::Int8Vector2
            | ValueType::UInt8Vector3
            | ValueType::Int8Vector3 => 1,
            ValueType::UInt16
            | ValueType::Int16
            | ValueType::UInt16Vector2
            | ValueType::Int16Vector2
            | ValueType::UInt16Vector3
            | ValueType::Int16Vector3 => 2,
            ValueType::UInt32
            | ValueType::Int32
            | ValueType::UInt32Vector2
            | ValueType::Int32Vector2
            | ValueType::UInt32Vector3
            | ValueType::Int32Vector3 => 4,
            ValueType::Bool => 1,
        }
    }
}
