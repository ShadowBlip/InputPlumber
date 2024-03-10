use std::error::Error;
use std::io;

use glob_match::glob_match;
use hidapi::{DeviceInfo, HidApi};
use serde::Deserialize;
use thiserror::Error;

use crate::{dmi::data::DMIData, procfs};

/// Represents all possible errors loading a [CompositeDevice]
#[derive(Debug, Error)]
pub enum LoadError {
    #[error("Could not read: {0}")]
    IoError(#[from] io::Error),
    #[error("Unable to deserialize: {0}")]
    DeserializeError(#[from] serde_yaml::Error),
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct CapabilityMap {
    pub version: u32,
    pub kind: String,
    pub name: String,
    pub id: String,
    pub mapping: Vec<CapabilityMapping>,
    //pub filtered_events: Option<Vec<Capability>>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct CapabilityMapping {
    pub name: String,
    pub source_events: Vec<Capability>,
    pub target_event: Capability,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct Capability {
    pub gamepad: Option<GamepadCapability>,
    pub keyboard: Option<String>,
    pub mouse: Option<MouseCapability>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct GamepadCapability {
    pub axis: Option<String>,
    pub button: Option<String>,
    pub trigger: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct MouseCapability {
    pub button: Option<String>,
    pub motion: Option<String>,
}

impl CapabilityMap {
    /// Load a [CapabilityMap] from the given YAML string
    pub fn from_yaml(content: String) -> Result<CapabilityMap, LoadError> {
        let device: CapabilityMap = serde_yaml::from_str(content.as_str())?;
        Ok(device)
    }

    /// Load a [CapabilityMap] from the given YAML file
    pub fn from_yaml_file(path: String) -> Result<CapabilityMap, LoadError> {
        let file = std::fs::File::open(path)?;
        let device: CapabilityMap = serde_yaml::from_reader(file)?;
        Ok(device)
    }
}

/// Defines a platform match for loading a [CompositeDevice]
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct Match {
    pub dmi_data: Option<DMIMatch>,
}

/// Match DMI data for loading a [CompositeDevice]
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct DMIMatch {
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

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct SourceDevice {
    pub group: String,
    pub evdev: Option<Evdev>,
    pub hidraw: Option<Hidraw>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct Evdev {
    pub name: Option<String>,
    pub phys_path: Option<String>,
    pub handler: Option<String>,
    pub vendor_id: Option<String>,
    pub product_id: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct Hidraw {
    pub vendor_id: Option<u16>,
    pub product_id: Option<u16>,
    pub interface_num: Option<i32>,
    pub handler: Option<String>,
}

/// Defines a combined device
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct CompositeDeviceConfig {
    pub version: u32,
    pub kind: String,
    pub name: String,
    pub matches: Vec<Match>,
    pub capability_map_id: Option<String>,
    pub source_devices: Vec<SourceDevice>,
    pub target_devices: Option<Vec<String>>,
}

impl CompositeDeviceConfig {
    /// Load a [CompositeDevice] from the given YAML string
    pub fn from_yaml(content: String) -> Result<CompositeDeviceConfig, LoadError> {
        let device: CompositeDeviceConfig = serde_yaml::from_str(content.as_str())?;
        Ok(device)
    }

    /// Load a [CompositeDevice] from the given YAML file
    pub fn from_yaml_file(path: String) -> Result<CompositeDeviceConfig, LoadError> {
        let file = std::fs::File::open(path)?;
        let device: CompositeDeviceConfig = serde_yaml::from_reader(file)?;
        Ok(device)
    }

    /// Returns an array of all defined hidraw source devices
    fn get_hidraw_configs(&self) -> Vec<Hidraw> {
        self.source_devices
            .iter()
            .filter_map(|device| device.hidraw.clone())
            .collect()
    }

    /// Returns an array of all defined evdev source devices
    fn get_evdev_configs(&self) -> Vec<Evdev> {
        self.source_devices
            .iter()
            .filter_map(|device| device.evdev.clone())
            .collect()
    }

    /// Returns true if a given hidraw device is within a list of hidraw configs.
    pub fn has_matching_hidraw(&self, device: &DeviceInfo, hidraw_configs: &Vec<Hidraw>) -> bool {
        log::debug!("Checking hidraw config: {:?}", hidraw_configs);
        for hidraw_config in hidraw_configs.clone() {
            let hidraw_config = hidraw_config.clone();

            if let Some(vendor_id) = hidraw_config.vendor_id {
                if device.vendor_id() != vendor_id {
                    continue;
                }
            }

            if let Some(product_id) = hidraw_config.product_id {
                if device.product_id() != product_id {
                    continue;
                }
            }

            if let Some(interface_num) = hidraw_config.interface_num {
                if device.interface_number() != interface_num {
                    continue;
                }
            }

            return true;
        }

        false
    }

    /// Returns true if a given evdev device is within a list of evdev configs.
    pub fn has_matching_evdev(
        &self,
        device: &procfs::device::Device,
        evdev_configs: &Vec<Evdev>,
    ) -> bool {
        // TODO: Maybe in the future we will support virtual devices if we figure something
        // out. Ignore virtual devices.
        if is_virtual(device) {
            log::debug!("{} is virtual, skipping.", device.name);
            return false;
        }

        for evdev_config in evdev_configs.clone() {
            let evdev_config = evdev_config.clone();

            if let Some(name) = evdev_config.name {
                if !glob_match(name.as_str(), device.name.as_str()) {
                    continue;
                }
            }

            if let Some(phys_path) = evdev_config.phys_path {
                if !glob_match(phys_path.as_str(), device.phys_path.as_str()) {
                    continue;
                }
            }

            if let Some(handler) = evdev_config.handler {
                let mut has_matches = false;
                for handle in device.handlers.clone() {
                    if !glob_match(handler.as_str(), handle.as_str()) {
                        continue;
                    }
                    has_matches = true;
                }
                if !has_matches {
                    continue;
                }
            }

            if let Some(vendor_id) = evdev_config.vendor_id {
                if !glob_match(vendor_id.as_str(), device.id.vendor.as_str()) {
                    continue;
                }
            }

            if let Some(product_id) = evdev_config.product_id {
                if !glob_match(product_id.as_str(), device.id.product.as_str()) {
                    continue;
                }
            }

            return true;
        }

        false
    }

    /// Returns true if the configuration has a valid set of matches. This will
    /// return true if ANY match config matches. If this list is empty, it will return true.
    pub fn has_valid_matches(&self, data: DMIData) -> bool {
        self.get_valid_matches(data).is_some()
    }

    /// Returns matches that matched system data.
    pub fn get_valid_matches(&self, data: DMIData) -> Option<Vec<Match>> {
        let mut matches: Vec<Match> = Vec::new();

        // If there are no match definitions, consider it a match
        if self.matches.is_empty() {
            return Some(matches);
        }

        // Check all match configs for ANY matches.
        for match_config in self.matches.clone() {
            let conf = match_config.clone();
            let mut has_matches = false;

            if let Some(dmi_config) = match_config.dmi_data {
                if let Some(bios_release) = dmi_config.bios_release {
                    if !glob_match(bios_release.as_str(), data.bios_release.as_str()) {
                        continue;
                    }
                    has_matches = true;
                }

                if let Some(bios_vendor) = dmi_config.bios_vendor {
                    if !glob_match(bios_vendor.as_str(), data.bios_vendor.as_str()) {
                        continue;
                    }
                    has_matches = true;
                }

                if let Some(bios_version) = dmi_config.bios_version {
                    if !glob_match(bios_version.as_str(), data.bios_version.as_str()) {
                        continue;
                    }
                    has_matches = true;
                }

                if let Some(board_name) = dmi_config.board_name {
                    if !glob_match(board_name.as_str(), data.board_name.as_str()) {
                        continue;
                    }
                    has_matches = true;
                }

                if let Some(product_name) = dmi_config.product_name {
                    if !glob_match(product_name.as_str(), data.product_name.as_str()) {
                        continue;
                    }
                    has_matches = true;
                }

                if let Some(product_version) = dmi_config.product_version {
                    if !glob_match(product_version.as_str(), data.product_version.as_str()) {
                        continue;
                    }
                    has_matches = true;
                }

                if let Some(product_sku) = dmi_config.product_sku {
                    if !glob_match(product_sku.as_str(), data.product_sku.as_str()) {
                        continue;
                    }
                    has_matches = true;
                }

                if let Some(sys_vendor) = dmi_config.sys_vendor {
                    if !glob_match(sys_vendor.as_str(), data.sys_vendor.as_str()) {
                        continue;
                    }
                    has_matches = true;
                }
            }

            if !has_matches {
                continue;
            }

            matches.push(conf);
        }

        if matches.is_empty() {
            return None;
        }

        Some(matches)
    }
}

/// Determines if a procfs device is virtual or real.
fn is_virtual(device: &procfs::device::Device) -> bool {
    if device.phys_path != "" {
        return false;
    }

    if device.sysfs_path.contains("/devices/virtual") {
        return true;
    }
    return false;
}
