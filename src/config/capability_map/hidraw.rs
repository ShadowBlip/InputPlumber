use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A [HidrawConfig] defines how to decode a particular event in an HID input
/// report.
#[derive(Debug, Deserialize, Serialize, Clone, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct HidrawConfig {
    pub report_id: u32,
    pub input_type: String,
    pub byte_start: u64,
    pub bit_offset: u8,
}
