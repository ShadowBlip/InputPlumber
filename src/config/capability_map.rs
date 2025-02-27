use std::{
    io::{self, Read},
    path::Path,
};

use ::procfs::CpuInfo;
use glob_match::glob_match;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    dmi::data::DMIData,
    input::event::{native::NativeEvent, value::InputValue},
    udev::device::UdevDevice,
};

use super::{CapabilityConfig, CapabilityMap, LoadError};

/// [CapabilityMapConfig] enumerates all versions of a [CapabilityMap]
#[derive(Debug, Clone)]
pub enum CapabilityMapConfig {
    V1(CapabilityMap),
    V2(CapabilityMapV2),
}

impl CapabilityMapConfig {
    pub fn from_yaml(content: String) -> Result<Self, LoadError> {
        let header: CapabilityMapHeader = serde_yaml::from_str(content.as_str())?;
        let config = match header.version {
            1 => Self::V1(serde_yaml::from_str(content.as_str())?),
            2 => Self::V2(serde_yaml::from_str(content.as_str())?),
            _ => Self::V1(serde_yaml::from_str(content.as_str())?),
        };

        Ok(config)
    }

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

/// [CapabilityMapHeader] is used to check the version/kind to determine which
/// capability map schema to use.
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct CapabilityMapHeader {
    pub version: u32,
    pub kind: String,
}

/// [CapabilityMapV2] defines a mapping of events to capabilities
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct CapabilityMapV2 {
    pub version: u32,
    pub kind: String,
    pub name: String,
    pub id: String,
    pub mapping: Vec<CapabilityMappingV2>,
}

impl CapabilityMapV2 {
    /// Load a [CapabilityMap] from the given YAML string
    pub fn _from_yaml(content: String) -> Result<CapabilityMapV2, LoadError> {
        let device: CapabilityMapV2 = serde_yaml::from_str(content.as_str())?;
        Ok(device)
    }

    /// Load a [CapabilityMap] from the given YAML file
    pub fn from_yaml_file(path: String) -> Result<CapabilityMapV2, LoadError> {
        let file = std::fs::File::open(path)?;
        let device: CapabilityMapV2 = serde_yaml::from_reader(file)?;
        Ok(device)
    }
}

/// A [CapabilityMappingV2] defines how to map source input to an inputplumber
/// capability.
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct CapabilityMappingV2 {
    pub name: String,
    pub source_events: Vec<SourceMapping>,
    pub target_event: CapabilityConfig,
}

/// A [SourceMapping] defines input events to be mapped
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct SourceMapping {
    pub evdev: Option<SourceEvdevMapping>,
    pub hidraw: Option<SourceHidrawMapping>,
    pub udev: Option<String>,
    pub capability: Option<CapabilityConfig>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct SourceEvdevMapping {
    pub event_type: String,
    pub event_code: String,
    pub event_value: Option<u32>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct SourceHidrawMapping {
    pub report_id: u32,
    pub input_type: String,
    pub byte_start: u64,
    pub bit_offset: u8,
}
