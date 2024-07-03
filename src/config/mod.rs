use std::io;

use ::procfs::CpuInfo;
use glob_match::glob_match;
use hidapi::DeviceInfo;
use serde::Deserialize;
use thiserror::Error;

use crate::{
    dmi::data::DMIData,
    iio,
    input::{
        event::{native::NativeEvent, value::InputValue},
        manager::SourceDeviceInfo,
    },
    procfs,
};

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
pub struct DeviceProfile {
    pub version: u32, //useful?
    pub kind: String, //useful?
    pub name: String, //useful?
    pub target_devices: Option<Vec<String>>,
    pub description: Option<String>,
    pub mapping: Vec<ProfileMapping>,
}

impl DeviceProfile {
    /// Load a [CapabilityProfile] from the given YAML string
    pub fn _from_yaml(content: String) -> Result<DeviceProfile, LoadError> {
        let device: DeviceProfile = serde_yaml::from_str(content.as_str())?;
        Ok(device)
    }

    /// Load a [CapabilityProfile] from the given YAML file
    pub fn from_yaml_file(path: String) -> Result<DeviceProfile, LoadError> {
        let file = std::fs::File::open(path)?;
        let device: DeviceProfile = serde_yaml::from_reader(file)?;
        Ok(device)
    }
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct ProfileMapping {
    pub name: String,
    pub source_event: CapabilityConfig,
    pub target_events: Vec<CapabilityConfig>,
}

impl ProfileMapping {
    /// Returns true if the given event matches this profile mapping's source
    /// event. This method assumes that the event capability already matches, so
    /// this should only be called when trying to match specific properties of
    /// a capability. I.e. Checking if an axis event matches "direction: left"
    pub fn source_matches_properties(&self, event: &NativeEvent) -> bool {
        // Gamepad event
        if let Some(gamepad) = self.source_event.gamepad.as_ref() {
            // Gamepad Axis
            if let Some(axis) = gamepad.axis.as_ref() {
                // Axis was defined for source event!
                if let Some(direction) = axis.direction.as_ref() {
                    // A direction was defined!
                    let value = event.get_value();
                    return match value {
                        InputValue::Vector2 { x, y } => match direction.as_str() {
                            // Left should be a negative value
                            "left" => x.filter(|&x| x <= 0.0).is_some(),
                            // Right should be a positive value
                            "right" => x.filter(|&x| x >= 0.0).is_some(),
                            // Up should be a negative value
                            "up" => y.filter(|&y| y <= 0.0).is_some(),
                            // Down should be a positive value
                            "down" => y.filter(|&y| y >= 0.0).is_some(),
                            _ => false,
                        },
                        // Other values should never be used if this was an axis
                        _ => false,
                    };
                } else {
                    // If no direction was defined for an axis, then this should match
                    return true;
                }
            }
            // Gamepad trigger
            else if let Some(_trigger) = gamepad.trigger.as_ref() {
                // Trigger was defined for source event!
                return true;
            }
            // Gamepad gyro
            else if let Some(_gyro) = gamepad.gyro.as_ref() {
                // Gyro was defined for source event!
                // TODO: this
            }
        }

        // Mouse event
        if let Some(mouse) = self.source_event.mouse.as_ref() {
            // Mouse motion
            if let Some(motion) = mouse.motion.as_ref() {
                // Mouse motion was defined for source event!
                if let Some(direction) = motion.direction.as_ref() {
                    // A direction was defined!
                    let value = event.get_value();
                    return match value {
                        InputValue::Vector2 { x, y } => match direction.as_str() {
                            // Left should be a negative value
                            "left" => x.filter(|&x| x <= 0.0).is_some(),
                            // Right should be a positive value
                            "right" => x.filter(|&x| x >= 0.0).is_some(),
                            // Up should be a negative value
                            "up" => y.filter(|&y| y <= 0.0).is_some(),
                            // Down should be a positive value
                            "down" => y.filter(|&y| y >= 0.0).is_some(),
                            _ => false,
                        },
                        // Other values should never be used if this was an axis
                        _ => false,
                    };
                } else {
                    // If no direction was defined for mouse motion, then this should match
                    return true;
                }
            }
        }

        // If no other input types were defined in the config, then it counts as
        // a match.
        true
    }
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

impl CapabilityMap {
    /// Load a [CapabilityMap] from the given YAML string
    pub fn _from_yaml(content: String) -> Result<CapabilityMap, LoadError> {
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

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct CapabilityMapping {
    pub name: String,
    pub source_events: Vec<CapabilityConfig>,
    pub target_event: CapabilityConfig,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct CapabilityConfig {
    pub gamepad: Option<GamepadCapability>,
    pub keyboard: Option<String>,
    pub mouse: Option<MouseCapability>,
    pub dbus: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct GamepadCapability {
    pub axis: Option<AxisCapability>,
    pub button: Option<String>,
    pub trigger: Option<TriggerCapability>,
    pub gyro: Option<GyroCapability>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct AxisCapability {
    pub name: String,
    pub direction: Option<String>,
    pub deadzone: Option<f64>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct TriggerCapability {
    pub name: String,
    pub deadzone: Option<f64>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct GyroCapability {
    pub name: String,
    pub direction: Option<String>,
    pub deadzone: Option<f64>,
    pub axis: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct MouseCapability {
    pub button: Option<String>,
    pub motion: Option<MouseMotionCapability>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct MouseMotionCapability {
    pub direction: Option<String>,
    pub speed_pps: Option<u64>,
}

/// Defines a platform match for loading a [CompositeDevice]
#[derive(Debug, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct Match {
    pub dmi_data: Option<DMIMatch>,
}

/// Match DMI data for loading a [CompositeDevice]
#[derive(Debug, Deserialize, Clone, PartialEq)]
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

#[derive(Debug, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct SourceDevice {
    pub group: String,
    pub evdev: Option<Evdev>,
    pub hidraw: Option<Hidraw>,
    pub iio: Option<IIO>,
    pub unique: Option<bool>,
    pub blocked: Option<bool>,
    pub ignore: Option<bool>,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct Evdev {
    pub name: Option<String>,
    pub phys_path: Option<String>,
    pub handler: Option<String>,
    pub vendor_id: Option<String>,
    pub product_id: Option<String>,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct Hidraw {
    pub vendor_id: Option<u16>,
    pub product_id: Option<u16>,
    pub interface_num: Option<i32>,
    pub handler: Option<String>,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
#[allow(clippy::upper_case_acronyms)]
pub struct IIO {
    pub id: Option<String>,
    pub name: Option<String>,
    pub mount_matrix: Option<MountMatrix>,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
#[allow(clippy::upper_case_acronyms)]
pub struct MountMatrix {
    pub x: [f64; 3],
    pub y: [f64; 3],
    pub z: [f64; 3],
}

/// Defines a combined device
#[derive(Debug, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct CompositeDeviceConfig {
    pub version: u32,
    pub kind: String,
    pub name: String,
    pub matches: Vec<Match>,
    pub single_source: Option<bool>,
    pub capability_map_id: Option<String>,
    pub source_devices: Vec<SourceDevice>,
    pub target_devices: Option<Vec<String>>,
}

impl CompositeDeviceConfig {
    /// Load a [CompositeDevice] from the given YAML string
    pub fn _from_yaml(content: String) -> Result<CompositeDeviceConfig, LoadError> {
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
    fn _get_hidraw_configs(&self) -> Vec<Hidraw> {
        self.source_devices
            .iter()
            .filter_map(|device| device.hidraw.clone())
            .collect()
    }

    /// Returns an array of all defined evdev source devices
    fn _get_evdev_configs(&self) -> Vec<Evdev> {
        self.source_devices
            .iter()
            .filter_map(|device| device.evdev.clone())
            .collect()
    }

    /// Returns a [SourceDevice] if it matches the given [SourceDeviceInfo].
    pub fn get_matching_device(&self, device: &SourceDeviceInfo) -> Option<SourceDevice> {
        match device {
            SourceDeviceInfo::EvdevDeviceInfo(evdev) => {
                for config in self.source_devices.iter() {
                    if let Some(evdev_config) = config.evdev.as_ref() {
                        if self.has_matching_evdev(evdev, evdev_config) {
                            return Some(config.clone());
                        }
                    }
                }
            }
            SourceDeviceInfo::HIDRawDeviceInfo(hidraw) => {
                for config in self.source_devices.iter() {
                    if let Some(hidraw_config) = config.hidraw.as_ref() {
                        if self.has_matching_hidraw(hidraw, hidraw_config) {
                            return Some(config.clone());
                        }
                    }
                }
            }
            SourceDeviceInfo::IIODeviceInfo(iio) => {
                for config in self.source_devices.iter() {
                    if let Some(iio_config) = config.iio.as_ref() {
                        if self.has_matching_iio(iio, iio_config) {
                            return Some(config.clone());
                        }
                    }
                }
            }
        }
        None
    }

    /// Returns true if a given hidraw device is within a list of hidraw configs.
    pub fn has_matching_hidraw(&self, device: &DeviceInfo, hidraw_config: &Hidraw) -> bool {
        log::debug!("Checking hidraw config: {:?}", hidraw_config);
        let hidraw_config = hidraw_config.clone();

        if let Some(vendor_id) = hidraw_config.vendor_id {
            if device.vendor_id() != vendor_id {
                return false;
            }
        }

        if let Some(product_id) = hidraw_config.product_id {
            if device.product_id() != product_id {
                return false;
            }
        }

        if let Some(interface_num) = hidraw_config.interface_num {
            if device.interface_number() != interface_num {
                return false;
            }
        }

        true
    }

    /// Returns true if a given iio device is within a list of iio configs.
    pub fn has_matching_iio(&self, device: &iio::device::Device, iio_config: &IIO) -> bool {
        log::debug!("Checking iio config: {:?}", iio_config);
        let iio_config = iio_config.clone();

        if let Some(id) = iio_config.id {
            let Some(device_id) = device.id.as_ref() else {
                return false;
            };
            if !glob_match(id.as_str(), device_id.as_str()) {
                return false;
            }
        }

        if let Some(name) = iio_config.name {
            let Some(device_name) = device.name.as_ref() else {
                return false;
            };
            if !glob_match(name.as_str(), device_name.as_str()) {
                return false;
            }
        }

        true
    }

    /// Returns true if a given evdev device is within a list of evdev configs.
    pub fn has_matching_evdev(
        &self,
        device: &procfs::device::Device,
        evdev_config: &Evdev,
    ) -> bool {
        //TODO: Check if the evdev has no proterties defined, that would always match.

        if device.is_virtual() {
            log::debug!("{} is virtual, skipping.", device.name);
            return false;
        }

        let evdev_config = evdev_config.clone();

        if let Some(name) = evdev_config.name {
            if !glob_match(name.as_str(), device.name.as_str()) {
                return false;
            }
        }

        if let Some(phys_path) = evdev_config.phys_path {
            if !glob_match(phys_path.as_str(), device.phys_path.as_str()) {
                return false;
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
                return false;
            }
        }

        if let Some(vendor_id) = evdev_config.vendor_id {
            if !glob_match(vendor_id.as_str(), device.id.vendor.as_str()) {
                return false;
            }
        }

        if let Some(product_id) = evdev_config.product_id {
            if !glob_match(product_id.as_str(), device.id.product.as_str()) {
                return false;
            }
        }
        true
    }

    /// Returns true if the configuration has a valid set of matches. This will
    /// return true if ANY match config matches. If this list is empty, it will return true.
    pub fn has_valid_matches(&self, data: &DMIData, cpu_info: &CpuInfo) -> bool {
        self.get_valid_matches(data, cpu_info).is_some()
    }

    /// Returns matches that matched system data.
    pub fn get_valid_matches(&self, data: &DMIData, cpu_info: &CpuInfo) -> Option<Vec<Match>> {
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
                if let Some(cpu_vendor) = dmi_config.cpu_vendor {
                    if !glob_match(
                        cpu_vendor.as_str(),
                        cpu_info.vendor_id(0).unwrap_or_default(),
                    ) {
                        continue;
                    }
                    has_matches = true;
                }

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
