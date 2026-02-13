pub mod capability_map;
#[cfg(test)]
pub mod config_test;
pub mod path;

use std::io::{self, Read};

use ::procfs::CpuInfo;
use capability_map::CapabilityConfig;
use glob_match::glob_match;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    dmi::data::DMIData,
    input::{
        event::{native::NativeEvent, value::InputValue},
        info::DeviceInfo,
    },
    udev::device::UdevDevice,
};

/// Represents all possible errors loading a [CompositeDevice]
#[derive(Debug, Error)]
pub enum LoadError {
    #[error("Could not read: {0}")]
    IoError(#[from] io::Error),
    #[error("Unable to deserialize: {0}")]
    DeserializeError(#[from] serde_yaml::Error),
    #[error("Config too large, reached maximum size of {0} bytes")]
    MaximumSizeReached(usize),
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct DeviceProfile {
    pub version: u32, //useful?
    pub kind: String, //useful?
    pub name: String, //useful?
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_devices: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub mapping: Vec<ProfileMapping>,
}

impl DeviceProfile {
    /// Load a [CapabilityProfile] from the given YAML string
    pub fn from_yaml(content: String) -> Result<DeviceProfile, LoadError> {
        let device: DeviceProfile = serde_yaml::from_str(content.as_str())?;
        Ok(device)
    }

    /// Load a [CapabilityProfile] from the given YAML file
    pub fn from_yaml_file(path: String) -> Result<DeviceProfile, LoadError> {
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
}

#[derive(Debug, Deserialize, Serialize, Clone)]
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

        // Touchpad event
        // TODO: implement touchpad specific matching

        // Touchscreen event
        if let Some(touch) = self.source_event.touchscreen.as_ref() {
            // Touch motion
            if let Some(motion) = touch.motion.as_ref() {
                // Touch motion was defined for source event!
                if let Some(_direction) = motion.region.as_ref() {
                    // TODO: Implement ability to map certain parts of the touch
                    // screen.
                    return true;
                }
            }
        }

        // If no other input types were defined in the config, then it counts as
        // a match.
        true
    }
}

/// Defines available options for loading a [CompositeDeviceConfig]
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct CompositeDeviceConfigOptions {
    /// If true, InputPlumber will automatically try to manage the input device.
    /// If this is false, InputPlumber will not try to manage the device unless
    /// an external service enables management of all devices.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_manage: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub persist: Option<bool>,
}

/// Defines a platform match for loading a [CompositeDeviceConfig]
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct Match {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dmi_data: Option<DMIMatch>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub udev: Option<Udev>,
}

/// Match DMI data for loading a [CompositeDevice]
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct DMIMatch {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bios_release: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bios_vendor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bios_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub board_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product_sku: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sys_vendor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu_vendor: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct SourceDevice {
    /// Custom group identifier for the source device.
    pub group: String,
    /// Devices that match the given evdev properties will be captured by InputPlumber
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evdev: Option<Evdev>,
    /// Devices that match the given hidraw properties will be captured by InputPlumber
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hidraw: Option<Hidraw>,
    /// Devices that match the given iio properties will be captured by InputPlumber
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iio: Option<IIO>,
    /// Devices that match the given led properties will be capture by InputPlumber
    #[serde(skip_serializing_if = "Option::is_none")]
    pub led: Option<Led>,
    /// Devices that match the given udev properties will be captured by InputPlumber
    #[serde(skip_serializing_if = "Option::is_none")]
    pub udev: Option<Udev>,
    /// Devices that match the given tty propertied will be captured by InputPlumber
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tty: Option<Tty>,
    /// Device configuration options are used to alter how the source device is managed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<SourceDeviceConfig>,
    /// If false, any devices matching this description will be added to the
    /// existing composite device. Defaults to true.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unique: Option<bool>,
    /// If true, device will be grabbed but no events from this device will
    /// reach target devices. Defaults to false.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocked: Option<bool>,
    /// If true, this source device will be ignored and not managed by
    /// InputPlumber. Defaults to false.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ignore: Option<bool>,
    /// If true, events will be read from this device, but the source device
    /// will not be hidden or grabbed. Defaults to false.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub passthrough: Option<bool>,
    /// Defines which events are included or excluded from input processing by
    /// the source device.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub events: Option<EventsConfig>,
    /// The ID of a device event mapping in the 'capability_maps' directory
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capability_map_id: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct SourceDeviceConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub touchscreen: Option<TouchscreenConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub imu: Option<ImuConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub led: Option<LedConfig>,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct TouchscreenConfig {
    /// Orientation of the touchscreen. Can be one of: ["normal", "left", "right", "upsidedown"]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub orientation: Option<String>,
    /// Width of the touchscreen. If set, any virtual touchscreens will use this width
    /// instead of querying the source device for its size.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    /// Height of the touchscreen. If set, any virtual touchscreens will use this height
    /// instead of querying the source device for its size.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    /// If true, the source device will use the width/height defined in this configuration
    /// instead of the size advertised by the device itself.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub override_source_size: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct ImuConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mount_matrix: Option<MountMatrix>,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct LedConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fixed_color: Option<FixedRgbColor>,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct Evdev {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phys_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub handler: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vendor_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product_id: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct Hidraw {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vendor_id: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product_id: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interface_num: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub handler: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct Udev {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attributes: Option<Vec<UdevAttribute>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dev_node: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dev_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub driver: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<Vec<UdevAttribute>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subsystem: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sys_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sys_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vendor_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interface_number: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct UdevAttribute {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
#[allow(clippy::upper_case_acronyms)]
pub struct IIO {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[deprecated(
        since = "0.43.0",
        note = "please use `<SourceDevice>.config.imu.mount_matrix` instead"
    )]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mount_matrix: Option<MountMatrix>,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
#[allow(clippy::upper_case_acronyms)]
pub struct Led {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
#[allow(clippy::upper_case_acronyms)]
pub struct Tty {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct FixedRgbColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
#[allow(clippy::upper_case_acronyms)]
pub struct MountMatrix {
    pub x: [f64; 3],
    pub y: [f64; 3],
    pub z: [f64; 3],
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct EventsConfig {
    /// Events to exclude from being processed by a source device
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exclude: Option<Vec<String>>,
    /// Events to include and be processed by a source device
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include: Option<Vec<String>>,
}

/// Defines a combined device
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct CompositeDeviceConfig {
    pub version: u32,
    pub kind: String,
    pub name: String,
    pub matches: Vec<Match>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub single_source: Option<bool>, // DEPRECATED; use 'maximum_sources' instead
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maximum_sources: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capability_map_id: Option<String>,
    pub source_devices: Vec<SourceDevice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_devices: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<CompositeDeviceConfigOptions>,
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

    /// Returns a [SourceDevice] if it matches the given [DeviceInfo]. Will return
    /// the first [SourceDevice] match found if multiple matches exist.
    pub fn get_matching_device(&self, device: &DeviceInfo) -> Option<SourceDevice> {
        for config in self.source_devices.iter() {
            let matched_config = match device {
                DeviceInfo::Udev(udevice) => self.get_matching_udev_device(config, udevice),
            };
            if matched_config.is_some() {
                return matched_config;
            }
        }

        None
    }

    /// Returns a copy of the given [SourceDevice] config if it matches the given
    /// [UdevDevice].
    fn get_matching_udev_device(
        &self,
        config: &SourceDevice,
        udevice: &UdevDevice,
    ) -> Option<SourceDevice> {
        let subsystem = udevice.subsystem();

        // TODO: This is a dirty hack to get around slow iio udev querying on
        // some devices. Replace with proper mitigation.
        let should_check_udev = { subsystem.as_str() != "iio" };

        // Check udev matches first
        if should_check_udev {
            if let Some(udev_config) = config.udev.as_ref() {
                if self.has_matching_udev(udevice, udev_config) {
                    return Some(config.clone());
                }
            }
        }

        // Use subsystem-specific device matching
        match subsystem.as_str() {
            "input" => {
                let evdev_config = config.evdev.as_ref()?;
                if self.has_matching_evdev(udevice, evdev_config) {
                    return Some(config.clone());
                }
            }
            "hidraw" => {
                let hidraw_config = config.hidraw.as_ref()?;
                if self.has_matching_hidraw(udevice, hidraw_config) {
                    return Some(config.clone());
                }
            }
            "iio" => {
                let iio_config = config.iio.as_ref()?;
                if self.has_matching_iio(udevice, iio_config) {
                    return Some(config.clone());
                }
            }
            "leds" => {
                let led_config = config.led.as_ref()?;
                if self.has_matching_led(udevice, led_config) {
                    return Some(config.clone());
                }
            }
            "tty" => {
                let tty_config = config.tty.as_ref()?;
                if self.has_matching_tty(udevice, tty_config) {
                    return Some(config.clone());
                }
            }
            _ => (),
        }

        None
    }

    /// Returns true if a given device matches the given udev config
    pub fn has_matching_udev(&self, device: &UdevDevice, udev_config: &Udev) -> bool {
        log::debug!("Checking udev config '{:?}'", udev_config);

        if let Some(attributes) = udev_config.attributes.as_ref() {
            let device_attributes = device.get_attributes();

            for attribute in attributes {
                let Some(device_attr_value) = device_attributes.get(&attribute.name) else {
                    // If the device does not have this attribute, return false
                    return false;
                };

                // If no value was specified in the config, then only match on
                // the presence of the attribute and not the value.
                let Some(attr_value) = attribute.value.as_ref() else {
                    continue;
                };

                // Glob match on the attribute value
                log::debug!("Checking attribute: {attr_value} against {device_attr_value}");
                if !glob_match(attr_value.as_str(), device_attr_value.as_str()) {
                    return false;
                }
            }
        }

        if let Some(dev_node) = udev_config.dev_node.as_ref() {
            let device_dev_node = device.devnode();
            log::debug!("Checking dev_node: {dev_node} against {device_dev_node}");
            if !glob_match(dev_node.as_str(), device_dev_node.as_str()) {
                return false;
            }
        }

        if let Some(dev_path) = udev_config.dev_path.as_ref() {
            let device_dev_path = device.devpath();
            log::debug!("Checking dev_path: {dev_path} against {device_dev_path}");
            if !glob_match(dev_path.as_str(), device_dev_path.as_str()) {
                return false;
            }
        }

        if let Some(driver) = udev_config.driver.as_ref() {
            let all_drivers = device.drivers();
            let mut has_matches = false;

            for device_driver in all_drivers {
                log::debug!("Checking driver: {driver} against {device_driver}");
                if glob_match(driver.as_str(), device_driver.as_str()) {
                    has_matches = true;
                    break;
                }
            }

            if !has_matches {
                return false;
            }
        }

        if let Some(properties) = udev_config.properties.as_ref() {
            let device_properties = device.get_properties();

            for property in properties {
                let Some(device_prop_value) = device_properties.get(&property.name) else {
                    // If the device does not have this property, return false
                    return false;
                };

                // If no value was specified in the config, then only match on
                // the presence of the property and not the value.
                let Some(prop_value) = property.value.as_ref() else {
                    continue;
                };

                // Glob match on the property value
                log::debug!("Checking property: {prop_value} against {device_prop_value}");
                if !glob_match(prop_value.as_str(), device_prop_value.as_str()) {
                    return false;
                }
            }
        }

        if let Some(subsystem) = udev_config.subsystem.as_ref() {
            let device_subsystem = device.subsystem();
            log::debug!("Checking subsystem: {subsystem} against {device_subsystem}");
            if !glob_match(subsystem.as_str(), device_subsystem.as_str()) {
                return false;
            }
        }

        if let Some(sys_name) = udev_config.sys_name.as_ref() {
            let device_sys_name = device.sysname();
            log::debug!("Checking sys_name: {sys_name} against {device_sys_name}");
            if !glob_match(sys_name.as_str(), device_sys_name.as_str()) {
                return false;
            }
        }

        if let Some(sys_path) = udev_config.sys_path.as_ref() {
            let device_sys_path = device.syspath();
            log::debug!("Checking sys_path: {sys_path} against {device_sys_path}");
            if !glob_match(sys_path.as_str(), device_sys_path.as_str()) {
                return false;
            }
        }

        if let Some(vendor_id) = udev_config.vendor_id.as_ref() {
            let device_vendor_id = device.id_vendor();
            let Ok(vendor_id) = vendor_id.parse::<u16>() else {
                return false;
            };
            log::debug!("Checking vendor id: {vendor_id} against {device_vendor_id}");

            if vendor_id != device_vendor_id {
                return false;
            }
        }

        if let Some(product_id) = udev_config.product_id.as_ref() {
            let device_product_id = device.id_product();
            let Ok(product_id) = product_id.parse::<u16>() else {
                return false;
            };
            log::debug!("Checking product id: {product_id} against {device_product_id}");

            if product_id != device_product_id {
                return false;
            }
        }

        if let Some(if_num) = udev_config.interface_number.as_ref() {
            let device_if_num = device.interface_number();
            let Ok(if_num) = if_num.parse::<i32>() else {
                return false;
            };
            log::debug!("Checking interface number: {if_num} against {device_if_num}");

            if if_num != device_if_num {
                return false;
            }
        }

        true
    }

    /// Returns true if a given hidraw device is within a list of hidraw configs.
    pub fn has_matching_hidraw(&self, device: &UdevDevice, hidraw_config: &Hidraw) -> bool {
        log::trace!("Checking hidraw config '{:?}'", hidraw_config,);

        // TODO: Switch either evdev of hidraw configs to use the same type. Legacy version had i16
        // for hidraw and string for evdev.
        if let Some(vendor_id) = hidraw_config.vendor_id {
            let vid = device.id_vendor();
            log::trace!("Checking vendor id: {vendor_id} against {vid}");
            if vid != vendor_id {
                return false;
            }
        }

        if let Some(product_id) = hidraw_config.product_id {
            let pid = device.id_product();
            log::trace!("Checking product_id: {product_id} against {pid}");
            if pid != product_id {
                return false;
            }
        }

        if let Some(interface_num) = hidraw_config.interface_num {
            let ifnum = device.interface_number();
            log::trace!("Checking interface number: {interface_num} against {ifnum}");
            if ifnum != interface_num {
                return false;
            }
        }

        if let Some(name) = hidraw_config.name.as_ref() {
            let dname = device.name();
            log::trace!("Checking name: {name} against {dname}");
            if !glob_match(name.as_str(), dname.as_str()) {
                return false;
            }
        }

        true
    }

    /// Returns true if a given iio device is within a list of iio configs.
    pub fn has_matching_iio(&self, device: &UdevDevice, iio_config: &IIO) -> bool {
        log::trace!("Checking iio config: {:?} against {:?}", iio_config, device);

        if let Some(id) = iio_config.id.as_ref() {
            let dsyspath = device.syspath();
            log::trace!("Checking id: {id} against {dsyspath}");
            if !glob_match(id.as_str(), dsyspath.as_str()) {
                return false;
            }
        }

        if let Some(name) = iio_config.name.as_ref() {
            let dname = device.name();
            log::trace!("Checking name: {name} against {dname}");
            if !glob_match(name.as_str(), dname.as_str()) {
                return false;
            }
        }

        true
    }

    /// Returns true if a given led device is within a list of led configs.
    pub fn has_matching_led(&self, device: &UdevDevice, led_config: &Led) -> bool {
        log::trace!("Checking led config: {:?} against {:?}", led_config, device);

        if let Some(id) = led_config.id.as_ref() {
            let dsyspath = device.syspath();
            log::trace!("Checking id: {id} against {dsyspath}");
            if !glob_match(id.as_str(), dsyspath.as_str()) {
                return false;
            }
        }

        if let Some(name) = led_config.name.as_ref() {
            let dname = device.name();
            log::trace!("Checking name: {name} against {dname}");
            if !glob_match(name.as_str(), dname.as_str()) {
                return false;
            }
        }

        true
    }

    /// Returns true if a given tty device is within a list of led configs.
    pub fn has_matching_tty(&self, device: &UdevDevice, tty_config: &Tty) -> bool {
        log::trace!("Checking tty config: {:?} against {:?}", tty_config, device);

        if let Some(id) = tty_config.id.as_ref() {
            let dsyspath = device.syspath();
            log::trace!("Checking id: {id} against {dsyspath}");
            if !glob_match(id.as_str(), dsyspath.as_str()) {
                return false;
            }
        }

        if let Some(name) = tty_config.name.as_ref() {
            let dname = device.name();
            log::trace!("Checking name: {name} against {dname}");
            if !glob_match(name.as_str(), dname.as_str()) {
                return false;
            }
        }

        true
    }

    /// Returns true if a given evdev device is within a list of evdev configs.
    pub fn has_matching_evdev(&self, device: &UdevDevice, evdev_config: &Evdev) -> bool {
        //TODO: Check if the evdev has no proterties defined, that would always match.
        log::trace!(
            "Checking evdev config: {:?} against {:?}",
            evdev_config,
            device
        );

        if let Some(name) = evdev_config.name.as_ref() {
            let dname = device.name();
            log::trace!("Checking name: {name} against {dname}");
            if !glob_match(name.as_str(), dname.as_str()) {
                return false;
            }
        }

        if let Some(phys_path) = evdev_config.phys_path.as_ref() {
            let dphys_path = device.phys();
            log::trace!("Checking phys_path: {phys_path} against {dphys_path}");
            if !glob_match(phys_path.as_str(), dphys_path.as_str()) {
                return false;
            }
        }

        if let Some(handler) = evdev_config.handler.as_ref() {
            let handle = device.sysname();
            log::trace!("Checking handler: {handler} against {handle}");
            if !glob_match(handler.as_str(), handle.as_str()) {
                return false;
            }
        }

        if let Some(vendor_id) = evdev_config.vendor_id.as_ref() {
            let id_vendor = format!("{:04x}", device.id_vendor());
            log::trace!("Checking vendor ID: {vendor_id} against {id_vendor}");
            if !glob_match(vendor_id.as_str(), id_vendor.as_str()) {
                return false;
            }
        }

        if let Some(product_id) = evdev_config.product_id.as_ref() {
            let id_product = format!("{:04x}", device.id_product());
            log::trace!("Checking product ID: {product_id} against {id_product}");
            if !glob_match(product_id.as_str(), id_product.as_str()) {
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

            if let Some(udev) = match_config.udev {
                let Some(sys_path) = udev.sys_path else {
                    log::warn!("Match using udev MUST define 'sys_path'");
                    return None;
                };

                let device = UdevDevice::from_syspath(sys_path.as_str());

                if let Some(subsystem_pattern) = udev.subsystem {
                    let subsystem = device.subsystem();
                    if !glob_match(&subsystem_pattern, &subsystem) {
                        continue;
                    }
                    has_matches = true;
                }

                if let Some(driver_pattern) = udev.driver {
                    let drivers = device.drivers();
                    let mut has_matching_driver = false;
                    for driver in drivers {
                        if !glob_match(&driver_pattern, &driver) {
                            continue;
                        }
                        has_matching_driver = true;
                        break;
                    }
                    has_matches = has_matching_driver;
                }

                if let Some(sys_name_pattern) = udev.sys_name {
                    let sys_name = device.sysname();
                    if !glob_match(&sys_name_pattern, &sys_name) {
                        continue;
                    }
                    has_matches = true;
                }

                if let Some(dev_node_pattern) = udev.dev_node {
                    let dev_node = device.devnode();
                    if !glob_match(&dev_node_pattern, &dev_node) {
                        continue;
                    }
                    has_matches = true;
                }

                if let Some(dev_path_pattern) = udev.dev_path {
                    let dev_path = device.devpath();
                    if !glob_match(&dev_path_pattern, &dev_path) {
                        continue;
                    }
                    has_matches = true;
                }

                if let Some(vendor_id_pattern) = udev.vendor_id {
                    let vendor_id = device.id_vendor().to_string();
                    if !glob_match(&vendor_id_pattern, &vendor_id) {
                        continue;
                    }
                    has_matches = true;
                }

                if let Some(product_id_pattern) = udev.product_id {
                    let product_id = device.id_product().to_string();
                    if !glob_match(&product_id_pattern, &product_id) {
                        continue;
                    }
                    has_matches = true;
                }

                if let Some(iface_num_pattern) = udev.interface_number {
                    let iface_num = device.interface_number().to_string();
                    if !glob_match(&iface_num_pattern, &iface_num) {
                        continue;
                    }
                    has_matches = true;
                }

                if let Some(attribute_patterns) = udev.attributes {
                    let mut all_attributes_match = true;

                    // All attribute patterns in the config must match
                    for attr_pattern in attribute_patterns {
                        let attr_name = attr_pattern.name;
                        let Some(attr_value) = device.get_attribute_from_tree(&attr_name) else {
                            // If the attribute was not found, this is not a match
                            all_attributes_match = false;
                            break;
                        };

                        let Some(value_pattern) = attr_pattern.value else {
                            // If no value was given, then assume '**' pattern
                            continue;
                        };

                        if !glob_match(&value_pattern, &attr_value) {
                            all_attributes_match = false;
                            break;
                        }
                    }

                    has_matches = all_attributes_match;
                }

                if let Some(property_patterns) = udev.properties {
                    let properties = device.get_properties();
                    let mut all_properties_match = true;

                    // All property patterns in the config must match
                    for prop_pattern in property_patterns {
                        let prop_name = prop_pattern.name;
                        let Some(prop_value) = properties.get(&prop_name) else {
                            // If the attribute was not found, this is not a match
                            all_properties_match = false;
                            break;
                        };

                        let Some(value_pattern) = prop_pattern.value else {
                            // If no value was given, then assume '**' pattern
                            continue;
                        };

                        if !glob_match(&value_pattern, prop_value) {
                            all_properties_match = false;
                            break;
                        }
                    }

                    has_matches = all_properties_match;
                }
            }

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
