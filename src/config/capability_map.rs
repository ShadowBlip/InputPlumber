pub mod evdev;
pub mod hidraw;

use std::{io::Read, path::Path};

use evdev::EvdevConfig;
use hidraw::HidrawConfig;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::LoadError;

/// [CapabilityMapConfig] enumerates all versions of a capability map. Capability
/// maps are used to fix or define the real capabilities of an input device.
#[derive(Debug, Clone)]
pub enum CapabilityMapConfig {
    V1(CapabilityMapConfigV1),
    V2(CapabilityMapConfigV2),
}

impl CapabilityMapConfig {
    /// Load a [CapabilityMapConfig] from the given YAML string
    pub fn from_yaml(content: String) -> Result<Self, LoadError> {
        let header: CapabilityMapConfigHeader = serde_yaml::from_str(content.as_str())?;
        let config = match header.version {
            1 => Self::V1(serde_yaml::from_str(content.as_str())?),
            2 => Self::V2(serde_yaml::from_str(content.as_str())?),
            _ => Self::V1(serde_yaml::from_str(content.as_str())?),
        };

        Ok(config)
    }

    /// Load a [CapabilityMapConfig] from the given YAML file
    pub fn from_yaml_file<P>(path: P) -> Result<Self, LoadError>
    where
        P: AsRef<Path>,
    {
        let mut file = std::fs::File::open(path)?;
        let mut content = String::default();
        file.read_to_string(&mut content)?;
        Self::from_yaml(content)
    }

    /// Returns the capability map identifier
    pub fn id(&self) -> String {
        match self {
            CapabilityMapConfig::V1(conf) => conf.id.clone(),
            CapabilityMapConfig::V2(conf) => conf.id.clone(),
        }
    }
}

/// [CapabilityMapConfigHeader] is used to check the version/kind to determine which
/// capability map schema to use.
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct CapabilityMapConfigHeader {
    pub version: u32,
    pub kind: String,
}

/// The [CapabilityMapConfigV1] contains a list of mappings that is used to map
/// one or more native inputplumber events to another.
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct CapabilityMapConfigV1 {
    pub version: u32,
    pub kind: String,
    pub name: String,
    pub id: String,
    pub mapping: Vec<NativeCapabilityMapping>,
}

impl CapabilityMapConfigV1 {
    /// Load a [CapabilityMapConfigV1] from the given YAML string
    pub fn _from_yaml(content: String) -> Result<CapabilityMapConfigV1, LoadError> {
        let device: CapabilityMapConfigV1 = serde_yaml::from_str(content.as_str())?;
        Ok(device)
    }

    /// Load a [CapabilityMapConfigV1] from the given YAML file
    pub fn from_yaml_file(path: String) -> Result<CapabilityMapConfigV1, LoadError> {
        let file = std::fs::File::open(path)?;
        let device: CapabilityMapConfigV1 = serde_yaml::from_reader(file)?;
        Ok(device)
    }
}

/// [CapabilityMapConfigV2] are used to fix or define the real capabilities of
/// an input device.
#[derive(Debug, Deserialize, Serialize, Clone, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct CapabilityMapConfigV2 {
    pub version: u32,
    pub kind: String,
    pub name: String,
    pub id: String,
    pub mapping: Vec<CapabilityMapping>,
}

impl CapabilityMapConfigV2 {
    /// Load a [CapabilityMap] from the given YAML string
    pub fn _from_yaml(content: String) -> Result<CapabilityMapConfigV2, LoadError> {
        let device: CapabilityMapConfigV2 = serde_yaml::from_str(content.as_str())?;
        Ok(device)
    }

    /// Load a [CapabilityMap] from the given YAML file
    pub fn from_yaml_file(path: String) -> Result<CapabilityMapConfigV2, LoadError> {
        let file = std::fs::File::open(path)?;
        let device: CapabilityMapConfigV2 = serde_yaml::from_reader(file)?;
        Ok(device)
    }
}

/// A [CapabilityMapping] defines how to map source input to an inputplumber
/// capability.
#[derive(Debug, Deserialize, Serialize, Clone, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct CapabilityMapping {
    pub name: String,
    pub source_events: Vec<SourceMapping>,
    pub target_event: CapabilityConfig,
}

/// A [SourceMapping] defines input events to be mapped
#[derive(Debug, Deserialize, Serialize, Clone, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct SourceMapping {
    pub evdev: Option<EvdevConfig>,
    pub hidraw: Option<HidrawConfig>,
    pub capability: Option<CapabilityConfig>,
}

/// A [NativeCapabilityMapping] maps one or more native inputplumber events to
/// a different native inputplumber event.
#[derive(Debug, Deserialize, Serialize, Clone, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct NativeCapabilityMapping {
    pub name: String,
    pub source_events: Vec<CapabilityConfig>,
    pub target_event: CapabilityConfig,
}

#[derive(Debug, Deserialize, Serialize, Clone, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct CapabilityConfig {
    pub gamepad: Option<GamepadCapability>,
    pub keyboard: Option<String>,
    pub mouse: Option<MouseCapability>,
    pub dbus: Option<String>,
    pub touchpad: Option<TouchpadCapability>,
    pub touchscreen: Option<TouchCapability>,
}

#[derive(Debug, Deserialize, Serialize, Clone, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct GamepadCapability {
    pub axis: Option<AxisCapability>,
    pub button: Option<String>,
    pub trigger: Option<TriggerCapability>,
    pub gyro: Option<GyroCapability>,
}

#[derive(Debug, Deserialize, Serialize, Clone, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct AxisCapability {
    pub name: String,
    pub direction: Option<String>,
    pub deadzone: Option<f64>,
}

#[derive(Debug, Deserialize, Serialize, Clone, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct TriggerCapability {
    pub name: String,
    pub deadzone: Option<f64>,
}

#[derive(Debug, Deserialize, Serialize, Clone, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct GyroCapability {
    pub name: String,
    pub direction: Option<String>,
    pub deadzone: Option<f64>,
    pub axis: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct MouseCapability {
    pub button: Option<String>,
    pub motion: Option<MouseMotionCapability>,
}

#[derive(Debug, Deserialize, Serialize, Clone, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct MouseMotionCapability {
    pub direction: Option<String>,
    pub speed_pps: Option<u64>,
}

#[derive(Debug, Deserialize, Serialize, Clone, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct TouchpadCapability {
    pub name: String,
    pub touch: TouchCapability,
}

#[derive(Debug, Deserialize, Serialize, Clone, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct TouchCapability {
    pub button: Option<String>,
    pub motion: Option<TouchMotionCapability>,
}

#[derive(Debug, Deserialize, Serialize, Clone, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct TouchMotionCapability {
    pub region: Option<String>,
    pub speed_pps: Option<u64>,
}
