use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A [HidrawConfig] defines how to decode a particular event in an HID input
/// report.
#[derive(Debug, Deserialize, Serialize, Clone, JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct HidrawConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub report_id: Option<u8>,
    pub input_type: String,
    pub byte_start: u64,
    /// Bit position within the byte (LSB=0).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bit_offset: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<u8>,
}
