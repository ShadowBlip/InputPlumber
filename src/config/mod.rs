use std::io;

use serde::Deserialize;
use thiserror::Error;

/// Represents all possible errors loading a [CompositeDevice]
#[derive(Debug, Error)]
pub enum LoadError {
    #[error("Could not read: {0}")]
    IoError(#[from] io::Error),
    #[error("Unable to deserialize: {0}")]
    DeserializeError(#[from] serde_yaml::Error),
}

/// Defines a platform match for loading a [CompositeDevice]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Match {
    pub bios_release: Option<String>,
    pub bios_vendor: Option<String>,
    pub bios_version: Option<String>,
    pub board_name: Option<String>,
    pub product_name: Option<String>,
    pub product_version: Option<String>,
    pub product_sku: Option<String>,
    pub sys_vendor: Option<String>,
    pub cpu_vendor: Option<String>,
}

/// Defines a combined device
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct CompositeDevice {
    pub version: u32,
    pub kind: String,
    pub name: String,
    pub matches: Vec<Match>,
    pub source_devices: Vec<SourceDevice>,
    pub event_map_id: String,
}

impl CompositeDevice {
    /// Load a [CompositeDevice] from the given YAML string
    pub fn from_yaml(content: String) -> Result<CompositeDevice, LoadError> {
        let device: CompositeDevice = serde_yaml::from_str(content.as_str())?;
        Ok(device)
    }

    /// Load a [CompositeDevice] from the given YAML file
    pub fn from_yaml_file(path: String) -> Result<CompositeDevice, LoadError> {
        let file = std::fs::File::open(path)?;
        let device: CompositeDevice = serde_yaml::from_reader(file)?;
        Ok(device)
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SourceDevice {
    pub name: String,
    pub phys_path: String,
    pub id: String,
    pub primary: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct EventMapping {
    pub name: String,
    pub from: String,
    pub source_events: Vec<Event>,
    pub emits: Event,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Event {
    pub code: String,
    #[serde(rename = "type")]
    pub event_type: String,
}
