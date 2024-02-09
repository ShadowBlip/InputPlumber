use std::error::Error;
use std::io;

use hidapi::{DeviceInfo, HidApi};
use serde::Deserialize;
use thiserror::Error;

use crate::procfs;

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
    pub vendor_id: Option<u32>,
    pub product_id: Option<u32>,
    pub interface_num: Option<u32>,
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
    pub source_devices: Vec<SourceDevice>,
    pub event_map_id: String,
    pub default_output_device: Option<String>,
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
        Ok(self.sources_exist_evdev()? && self.sources_exist_hidraw()?)
    }

    /// Returns true if all the hidraw source devices in the config exist on the system
    fn sources_exist_hidraw(&self) -> Result<bool, Box<dyn Error>> {
        let Some(hidraw_devices) = self.get_matching_hidraw()? else {
            return Ok(true);
        };
        let hidraw_configs = self.get_hidraw_configs();

        Ok(hidraw_configs.len() == hidraw_devices.len())
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

        // Get all hidraw devices to match on and check to see if they match
        // a hidraw definition in the config.
        let api = HidApi::new()?;
        let devices: Vec<DeviceInfo> = api.device_list().cloned().collect();
        for device in devices {
            for hidraw_config in hidraw_configs.clone() {
                let hidraw_config = hidraw_config.clone();
                let mut has_matches = false;

                if let Some(vendor_id) = hidraw_config.vendor_id {
                    if device.vendor_id() as u32 != vendor_id {
                        continue;
                    }
                    has_matches = true;
                }

                if let Some(product_id) = hidraw_config.product_id {
                    if device.product_id() as u32 != product_id {
                        continue;
                    }
                    has_matches = true;
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
                    if device.name != name {
                        continue;
                    }
                    has_matches = true;
                }

                if let Some(phys_path) = evdev_config.phys_path {
                    if device.phys_path != phys_path {
                        continue;
                    }
                    has_matches = true;
                }

                if let Some(handler) = evdev_config.handler {
                    if !device.handlers.contains(&handler) {
                        continue;
                    }
                    has_matches = true;
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
}
