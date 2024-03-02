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
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct Hidraw {
    pub vendor_id: Option<u16>,
    pub product_id: Option<u16>,
    pub interface_num: Option<i32>,
    pub handler: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct EventMapping {
    pub name: String,
    pub from: String,
    pub source_events: Vec<Event>,
    pub emits: Event,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct Event {
    pub code: String,
    #[serde(rename = "type")]
    pub event_type: String,
}

/// Defines a combined device
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct CompositeDeviceConfig {
    pub version: u32,
    pub kind: String,
    pub name: String,
    pub matches: Vec<Match>,
    pub event_map_id: String,
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

    /// Checks to see if the [CompositeDeviceConfig] matches what is available
    /// on the system.
    pub fn sources_exist(&self) -> Result<bool, Box<dyn Error>> {
        let evdev_exists = self.sources_exist_evdev()?;
        log::debug!("Evdev Devices Exist: {}", evdev_exists);
        let hidraw_exists = self.sources_exist_hidraw()?;
        log::debug!("HIDRaw Devices Exist: {}", hidraw_exists);
        Ok(evdev_exists && hidraw_exists)
    }

    /// Returns true if all the hidraw source devices in the config exist on the system
    fn sources_exist_hidraw(&self) -> Result<bool, Box<dyn Error>> {
        let Some(hidraw_devices) = self.get_matching_hidraw()? else {
            return Ok(true);
        };

        Ok(hidraw_devices.len() >= 1)
    }

    /// Returns true if all the evdev source devices in the config exist on the system
    fn sources_exist_evdev(&self) -> Result<bool, Box<dyn Error>> {
        let Some(evdev_devices) = self.get_matching_evdev()? else {
            return Ok(true);
        };
        let evdev_configs = self.get_evdev_configs();

        Ok(evdev_configs.len() == evdev_devices.len())
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

    /// Returns a list of hidraw device information for all devices that
    /// match the configuration.
    pub fn get_matching_hidraw(&self) -> Result<Option<Vec<DeviceInfo>>, Box<dyn Error>> {
        // Only consider hidraw devices
        let hidraw_configs = self.get_hidraw_configs();

        // If there are no hidraw definitions, consider it a match
        if hidraw_configs.is_empty() {
            return Ok(None);
        }
        let mut matches: Vec<DeviceInfo> = Vec::new();

        // Keep track of potentially duplicate hidraw devices with the same
        // vendor + product
        let mut seen_devices: Vec<String> = Vec::new();

        // Get all hidraw devices to match on and check to see if they match
        // a hidraw definition in the config.
        let api = HidApi::new()?;
        let devices: Vec<DeviceInfo> = api.device_list().cloned().collect();
        for device in devices {
            for hidraw_config in hidraw_configs.clone() {
                let hidraw_config = hidraw_config.clone();
                let mut has_matches = false;

                if let Some(vendor_id) = hidraw_config.vendor_id {
                    if device.vendor_id() != vendor_id {
                        continue;
                    }
                    has_matches = true;
                }

                if let Some(product_id) = hidraw_config.product_id {
                    if device.product_id() != product_id {
                        continue;
                    }
                    has_matches = true;
                }

                if let Some(interface_num) = hidraw_config.interface_num {
                    if device.interface_number() != interface_num {
                        continue;
                    }
                    has_matches = true;
                }

                if !has_matches {
                    continue;
                }

                // Construct a device ID from the vendor and product to see
                // if this device has already been matched.
                let device_id = format!(
                    "{:04x}:{:04x}:{}",
                    device.vendor_id(),
                    device.product_id(),
                    device.interface_number()
                );
                if seen_devices.contains(&device_id) {
                    log::debug!("Device already seen: {}", device_id);
                    continue;
                }

                // If it's gotten this far, then the config has matched all
                // non-empty fields!
                matches.push(device.clone());
                seen_devices.push(device_id);
            }
        }

        Ok(Some(matches))
    }

    /// Returns a list of evdev device information for all devices that match
    /// the configuration
    pub fn get_matching_evdev(
        &self,
    ) -> Result<Option<Vec<procfs::device::Device>>, Box<dyn Error>> {
        // Only consider evdev devices
        let evdev_configs = self.get_evdev_configs();

        // If there are no evdev definitions, consider it a match
        if evdev_configs.is_empty() {
            return Ok(None);
        }
        let mut matches: Vec<procfs::device::Device> = Vec::new();

        // Get all evdev devices to match on and check to see if they match
        // an evdev definition in the config.
        let devices = procfs::device::get_all()?;
        for device in devices {
            for evdev_config in evdev_configs.clone() {
                let evdev_config = evdev_config.clone();
                let mut has_matches = false;

                if let Some(name) = evdev_config.name {
                    if !glob_match(name.as_str(), device.name.as_str()) {
                        continue;
                    }
                    has_matches = true;
                }

                if let Some(phys_path) = evdev_config.phys_path {
                    if !glob_match(phys_path.as_str(), device.phys_path.as_str()) {
                        continue;
                    }
                    has_matches = true;
                }

                if let Some(handler) = evdev_config.handler {
                    for handle in device.handlers.clone() {
                        if !glob_match(handler.as_str(), handle.as_str()) {
                            continue;
                        }
                        has_matches = true;
                    }
                }

                if !has_matches {
                    continue;
                }

                // If it's gotten this far, then the config has matched all
                // non-empty fields!
                matches.push(device.clone());
            }
        }

        Ok(Some(matches))
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
