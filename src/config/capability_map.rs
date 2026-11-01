pub mod evdev;
pub mod hidraw;

use std::{
    collections::{HashMap, HashSet},
    io::Read,
    path::Path,
};

use evdev::EvdevConfig;
use hidraw::HidrawConfig;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{
    path::{get_capability_maps_paths, get_multidir_sorted_files},
    LoadError,
};
use crate::input::{capability::Capability, event::native::NativeEvent};

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
        let map = match mapping {
            Ok(map) => map,
            Err(e) => {
                log::warn!("Failed to parse capability mapping: {e}",);
                continue;
            }
        };
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
        let file = std::fs::File::open(path)?;

        // Read up to a defined maximum size to prevent denial of service
        const MAX_SIZE: usize = 512 * 1024;
        let mut reader = file.take(MAX_SIZE as u64);
        let mut content = String::default();
        let bytes_read = reader.read_to_string(&mut content)?;
        if bytes_read == MAX_SIZE {
            return Err(LoadError::MaximumSizeReached(MAX_SIZE));
        }
        Self::from_yaml(content)
    }

    /// Returns the capability map identifier
    pub fn id(&self) -> String {
        match self {
            CapabilityMapConfig::V1(conf) => conf.id.clone(),
            CapabilityMapConfig::V2(conf) => conf.id.clone(),
        }
    }

    /// Returns the V1 single-source capability-to-capability mappings.
    pub fn capability_translation(&self) -> HashMap<Capability, Capability> {
        let CapabilityMapConfig::V1(map) = self else {
            log::error!(
                "V2 capability map '{}' cannot be applied to non-evdev sources",
                self.id()
            );
            return HashMap::new();
        };

        let mut source_counts: HashMap<Capability, usize> = HashMap::new();
        for mapping in map.mapping.iter() {
            if mapping.source_events.len() != 1 {
                log::warn!(
                    "Capability map '{}' mapping '{}' has multiple source events, which \
                     cannot be applied to non-evdev sources; dropping",
                    map.id,
                    mapping.name
                );
                continue;
            }
            let source: Capability = mapping.source_events[0].clone().into();
            if matches!(source, Capability::NotImplemented) {
                continue;
            }
            *source_counts.entry(source).or_default() += 1;
        }
        let duplicates: HashSet<Capability> = source_counts
            .into_iter()
            .filter(|(_, count)| *count > 1)
            .map(|(capability, _)| capability)
            .collect();
        for capability in &duplicates {
            log::warn!(
                "Capability map '{}' has multiple mappings for {capability:?}; dropping all",
                map.id
            );
        }

        map.mapping
            .iter()
            .filter(|mapping| mapping.source_events.len() == 1)
            .filter_map(|mapping| {
                let source: Capability = mapping.source_events[0].clone().into();
                let target: Capability = mapping.target_event.clone().into();
                if matches!(source, Capability::NotImplemented)
                    || matches!(target, Capability::NotImplemented)
                {
                    return None;
                }
                if duplicates.contains(&source) {
                    return None;
                }
                Some((source, target))
            })
            .collect()
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mapping_type: Option<MappingType>,
    pub source_events: Vec<SourceMapping>,
    pub target_event: CapabilityConfig,
}

/// [MappingType] defines how source events should be translated
#[derive(Debug, Deserialize, Serialize, Clone, JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct MappingType {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evdev: Option<EvdevMappingType>,
}

/// How evdev source events should be translated
#[derive(Default, Debug, Deserialize, Serialize, Clone, JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum EvdevMappingType {
    /// Source events will be treated as an inclusive match to emit a single
    /// inputplumber event. Events emitted immediately upon match.
    Chord,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evdev: Option<EvdevConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hidraw: Option<HidrawConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
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

#[derive(Debug, Deserialize, Serialize, Clone, JsonSchema, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub struct CapabilityConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gamepad: Option<GamepadCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keyboard: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mouse: Option<MouseCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dbus: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub touchpad: Option<TouchpadCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub touchscreen: Option<TouchCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gyroscope: Option<SourceCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accelerometer: Option<SourceCapability>,
}

#[derive(Debug, Deserialize, Serialize, Clone, JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct GamepadCapability {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub axis: Option<AxisCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub button: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trigger: Option<TriggerCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gyro: Option<GyroCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accelerometer: Option<AccelerometerCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dial: Option<DialCapability>,
}

#[derive(Debug, Deserialize, Serialize, Clone, JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct AxisCapability {
    pub name: String,
    /// Direction of the axis to translate. Can be one of ["vertical", "horizontal", "left",
    /// "right", "up", "down"].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direction: Option<String>,
    /// A value from 0.0-1.0 of how far an axis should be pressed in a particular direction
    /// in order for it to be considered "pressed"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deadzone: Option<f64>,
    /// Use quadratic scaling for more precise control with small stick movements
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quadratic_scaling: Option<bool>,
    /// Invert all values of this axis with the matching direction
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invert: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize, Clone, JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct TriggerCapability {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deadzone: Option<f64>,
}

#[derive(Debug, Deserialize, Serialize, Clone, JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct GyroCapability {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direction: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deadzone: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub axis: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone, JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct AccelerometerCapability {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direction: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deadzone: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub axis: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone, JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct DialCapability {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direction: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone, JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct MouseCapability {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub button: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub motion: Option<MouseMotionCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wheel: Option<MouseWheelCapability>,
}

#[derive(Debug, Deserialize, Serialize, Clone, JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct MouseMotionCapability {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direction: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speed_pps: Option<u64>,
}

#[derive(Debug, Deserialize, Serialize, Clone, JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct MouseWheelCapability {
    pub direction: Option<String>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub button: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub motion: Option<TouchMotionCapability>,
}

#[derive(Debug, Deserialize, Serialize, Clone, JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct TouchMotionCapability {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speed_pps: Option<u64>,
}
#[derive(Debug, Deserialize, Serialize, Clone, JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct SourceCapability {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direction: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deadzone: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub axis: Option<String>,
}

/// Builds the capability translation map from the composite-level and
/// optional source-level maps. Entries present in both are logged and
/// dropped.
pub fn get_capability_translations(
    composite_map: Option<&CapabilityMapConfig>,
    source_map: Option<&CapabilityMapConfig>,
) -> HashMap<Capability, Capability> {
    let mut map = composite_map
        .map(|map| map.capability_translation())
        .unwrap_or_default();

    let Some(source_map) = source_map else {
        return map;
    };
    let source_map = source_map.capability_translation();

    let duplicates: HashSet<Capability> = source_map
        .keys()
        .filter(|capability| map.contains_key(*capability))
        .cloned()
        .collect();
    for capability in &duplicates {
        log::warn!("Duplicate capability map entry {capability:?}; dropping both");
    }

    map.retain(|capability, _| !duplicates.contains(capability));
    for (capability, target) in source_map {
        if !duplicates.contains(&capability) {
            map.insert(capability, target);
        }
    }

    map
}

/// Translates the capabilities of the given events using the given map.
/// Event values are passed through as-is.
pub fn translate_capabilities(
    events: Vec<NativeEvent>,
    map: &HashMap<Capability, Capability>,
) -> Vec<NativeEvent> {
    if map.is_empty() {
        return events;
    }
    events
        .into_iter()
        .map(|event| {
            let source = event.as_capability();
            match map.get(&source) {
                Some(target) => {
                    log::trace!("capability map: {source:?} -> {target:?}");
                    NativeEvent::new_translated(source, target.clone(), event.get_value())
                }
                None => event,
            }
        })
        .collect()
}

/// Maps the given capabilities through the given map, leaving
/// untranslated capabilities unchanged.
pub fn exposed_capabilities(
    capabilities: Vec<Capability>,
    map: &HashMap<Capability, Capability>,
) -> Vec<Capability> {
    if map.is_empty() {
        return capabilities;
    }
    capabilities
        .into_iter()
        .map(|capability| map.get(&capability).cloned().unwrap_or(capability))
        .collect()
}
