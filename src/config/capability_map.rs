pub mod evdev;
pub mod hidraw;

use std::{collections::HashMap, io::Read, path::Path};

use evdev::EvdevConfig;
use hidraw::HidrawConfig;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{
    path::{get_capability_maps_paths, get_multidir_sorted_files},
    LoadError,
};

/// Loads all capability mappings in all default locations and returns a hashmap
/// of the CapabilityMap ID and the [CapabilityMap].
pub fn load_capability_mappings() -> HashMap<String, CapabilityMapConfig> {
    let mut mappings = HashMap::new();
    let paths = get_capability_maps_paths();
    let files = get_multidir_sorted_files(paths.as_slice(), |entry| {
        entry.path().extension().unwrap() == "yaml"
    });

    // Look at each file in the directory and try to load them
    for file in files {
        // Try to load the capability map
        log::trace!("Found file: {}", file.display());
        let mapping = CapabilityMapConfig::from_yaml_file(file.display().to_string());
        if mapping.is_err() {
            log::warn!(
                "Failed to parse capability mapping: {}",
                mapping.unwrap_err()
            );
            continue;
        }
        let map = mapping.unwrap();
        mappings.insert(map.id(), map);
    }

    mappings
}

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

/// [CapabilityMapConfigV2] are used to fix or define the real capabilities of
/// an input device.
#[derive(Debug, Deserialize, Serialize, Clone, JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct CapabilityMapConfigV2 {
    pub version: u32,
    pub kind: String,
    pub name: String,
    pub id: String,
    pub mapping: Vec<CapabilityMapping>,
}

/// A [CapabilityMapping] defines how to map source input to an inputplumber
/// capability.
#[derive(Debug, Deserialize, Serialize, Clone, JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct CapabilityMapping {
    pub name: String,
    pub mapping_type: Option<MappingType>,
    pub source_events: Vec<SourceMapping>,
    pub target_event: CapabilityConfig,
}

/// [MappingType] defines how source events should be translated
#[derive(Debug, Deserialize, Serialize, Clone, JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct MappingType {
    pub evdev: Option<EvdevMappingType>,
}

/// How evdev source events should be translated
#[derive(Default, Debug, Deserialize, Serialize, Clone, JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum EvdevMappingType {
    /// Source events will be treated as an inclusive match to emit a single
    /// inputplumber event. Events emitted immediately upon match.
    Chord,
    /// Source events will be treated as an inclusive match to emit a single
    /// inputplumber event. Events emitted after button is released.
    DelayedChord,
    /// Multiple source events will emit the same inputplumber event. Useful for mapping
    /// axis where different directions are triggered by separate events or multiple
    /// events to the same inputplumber event.
    #[default]
    MultiSource,
}

/// A [SourceMapping] defines input events to be mapped
#[derive(Debug, Deserialize, Serialize, Clone, JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct SourceMapping {
    pub evdev: Option<EvdevConfig>,
    pub hidraw: Option<HidrawConfig>,
    pub capability: Option<CapabilityConfig>,
}

/// A [NativeCapabilityMapping] maps one or more native inputplumber events to
/// a different native inputplumber event.
#[derive(Debug, Deserialize, Serialize, Clone, JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct NativeCapabilityMapping {
    pub name: String,
    pub source_events: Vec<CapabilityConfig>,
    pub target_event: CapabilityConfig,
}

#[derive(Debug, Deserialize, Serialize, Clone, JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct CapabilityConfig {
    pub gamepad: Option<GamepadCapability>,
    pub keyboard: Option<String>,
    pub mouse: Option<MouseCapability>,
    pub dbus: Option<String>,
    pub touchpad: Option<TouchpadCapability>,
    pub touchscreen: Option<TouchCapability>,
    pub layer: Option<LayerCapability>,
}

#[derive(Debug, Deserialize, Serialize, Clone, JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct GamepadCapability {
    pub axis: Option<AxisCapability>,
    pub button: Option<String>,
    pub trigger: Option<TriggerCapability>,
    pub gyro: Option<GyroCapability>,
    pub accelerometer: Option<AccelerometerCapability>,
    pub dial: Option<DialCapability>,
}

#[derive(Debug, Deserialize, Serialize, Clone, JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct AxisCapability {
    pub name: String,
    pub direction: Option<String>,
    pub deadzone: Option<f64>,
}

#[derive(Debug, Deserialize, Serialize, Clone, JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct TriggerCapability {
    pub name: String,
    pub deadzone: Option<f64>,
}

#[derive(Debug, Deserialize, Serialize, Clone, JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct GyroCapability {
    pub name: String,
    pub direction: Option<String>,
    pub deadzone: Option<f64>,
    pub axis: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone, JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct AccelerometerCapability {
    pub name: String,
    pub direction: Option<String>,
    pub deadzone: Option<f64>,
    pub axis: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone, JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct DialCapability {
    pub name: String,
    pub direction: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone, JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct MouseCapability {
    pub button: Option<String>,
    pub motion: Option<MouseMotionCapability>,
}

#[derive(Debug, Deserialize, Serialize, Clone, JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct MouseMotionCapability {
    pub direction: Option<String>,
    pub speed_pps: Option<u64>,
}

#[derive(Debug, Deserialize, Serialize, Clone, JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct TouchpadCapability {
    pub name: String,
    pub touch: TouchCapability,
}

#[derive(Debug, Deserialize, Serialize, Clone, JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct TouchCapability {
    pub button: Option<String>,
    pub motion: Option<TouchMotionCapability>,
}

#[derive(Debug, Deserialize, Serialize, Clone, JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct TouchMotionCapability {
    pub region: Option<String>,
    pub speed_pps: Option<u64>,
}

#[derive(Debug, Deserialize, Serialize, Clone, JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct LayerCapability {
    pub name: String,
}

/// A layer mapping is used to translate input events whenever a given layer is
/// activated. This can be used for things like button combos, where pressing
/// and holding a button activates the layer, which contains different translations.
#[derive(Debug, Deserialize, Serialize, Clone, JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct LayerMapping {
    pub name: String,
    pub mappings: Vec<NativeCapabilityMapping>,
}
