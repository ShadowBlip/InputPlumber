use std::{
    collections::HashMap,
    error::Error,
    ffi::OsStr,
    fs::{self, read_link},
    path::Path,
};

#[derive(Debug, Clone, Default)]
pub struct UdevDevice {
    devnode: String,
    subsystem: String,
    syspath: String,
    sysname: String,
}

impl UdevDevice {
    /// Returns a UdevDevice object from the given base path and name.
    /// e.g. UdevDevice::from_devnode("/dev", "hidraw0");
    pub fn from_devnode(base_path: &str, name: &str) -> Self {
        let devnode = format!("{base_path}/{name}");
        let subsystem = {
            match base_path {
                "/dev" => {
                    if name.starts_with("hidraw") {
                        Some("hidraw")
                    } else if name.starts_with("iio:") {
                        Some("iio")
                    } else {
                        None
                    }
                }
                "/dev/input" => Some("input"),

                _ => None,
            }
        }
        .unwrap_or_default()
        .to_string();

        Self {
            devnode,
            subsystem,
            syspath: "".to_string(),
            sysname: name.to_string(),
        }
    }

    /// returns a udev::Device from the stored syspath.
    pub fn get_device(&self) -> Result<::udev::Device, Box<dyn Error + Send + Sync>> {
        match ::udev::Device::from_syspath(Path::new(self.syspath.as_str())) {
            Ok(device) => Ok(device),
            Err(e) => Err(e.into()),
        }
    }

    /// Returns true if this device is virtual
    pub fn is_virtual(&self) -> bool {
        let Ok(device) = self.get_device() else {
            return true;
        };

        // Some devices (e.g. Steam Deck,DualSense) have a syspath in /devices/virtual but also
        // have a real phys path. Only check the syspath if there is no phys attribute.
        if !device
            .attribute_value("phys")
            .unwrap_or(OsStr::new(""))
            .to_string_lossy()
            .to_string()
            .is_empty()
        {
            return false;
        }
        device
            .syspath()
            .to_string_lossy()
            .to_string()
            .contains("/devices/virtual")
    }

    pub fn devnode(&self) -> String {
        self.devnode.clone()
    }

    pub fn devpath(&self) -> String {
        let Ok(device) = self.get_device() else {
            return "".to_string();
        };
        device.devpath().to_string_lossy().to_string()
    }

    pub fn id_bustype(&self) -> u16 {
        let Ok(device) = self.get_device() else {
            return 0;
        };
        let orig = get_attribute_from_tree(&device, "id/bustype");
        let stripped = orig.strip_prefix("0x").unwrap_or(orig.as_str());
        let attr = u16::from_str_radix(stripped, 16).unwrap_or(0);
        if attr != 0 {
            return attr;
        }
        let Some(attr) = get_attribute_from_sysfs(&device, "input", "id/bustype") else {
            return 0;
        };
        u16::from_str_radix(attr.as_str(), 16).unwrap_or(0)
    }

    pub fn id_product(&self) -> u16 {
        let Ok(device) = self.get_device() else {
            return 0;
        };
        let orig = get_attribute_from_tree(&device, "idProduct");
        let stripped = orig.strip_prefix("0x").unwrap_or(orig.as_str());
        let attr = u16::from_str_radix(stripped, 16).unwrap_or(0);
        if attr != 0 {
            return attr;
        }
        let Some(attr) = get_attribute_from_sysfs(&device, "input", "id/product") else {
            return 0;
        };
        u16::from_str_radix(attr.as_str(), 16).unwrap_or(0)
    }

    pub fn id_vendor(&self) -> u16 {
        let Ok(device) = self.get_device() else {
            return 0;
        };
        let orig = get_attribute_from_tree(&device, "idVendor");
        let stripped = orig.strip_prefix("0x").unwrap_or(orig.as_str());
        let attr = u16::from_str_radix(stripped, 16).unwrap_or(0);
        if attr != 0 {
            return attr;
        }
        let Some(attr) = get_attribute_from_sysfs(&device, "input", "id/vendor") else {
            return 0;
        };
        u16::from_str_radix(attr.as_str(), 16).unwrap_or(0)
    }

    pub fn id_version(&self) -> u16 {
        let Ok(device) = self.get_device() else {
            return 0;
        };
        let orig = get_attribute_from_tree(&device, "id/version");
        let stripped = orig.strip_prefix("0x").unwrap_or(orig.as_str());
        let attr = u16::from_str_radix(stripped, 16).unwrap_or(0);
        if attr != 0 {
            return attr;
        }
        let Some(attr) = get_attribute_from_sysfs(&device, "input", "id/version") else {
            return 0;
        };
        u16::from_str_radix(attr.as_str(), 16).unwrap_or(0)
    }

    pub fn interface_number(&self) -> i32 {
        let Ok(device) = self.get_device() else {
            return -1;
        };
        let orig = get_attribute_from_tree(&device, "bInterfaceNumber");
        let stripped = orig.strip_prefix("0x").unwrap_or(orig.as_str());
        i32::from_str_radix(stripped, 16).unwrap_or(0)
    }

    pub fn manufacturer(&self) -> String {
        let Ok(device) = self.get_device() else {
            return "".to_string();
        };
        get_attribute_from_tree(&device, "manufacturer")
    }

    pub fn name(&self) -> String {
        let Ok(device) = self.get_device() else {
            return "".to_string();
        };
        let attr = get_attribute_from_tree(&device, "name");
        if !attr.is_empty() {
            return attr;
        }
        let Some(attr) = get_attribute_from_sysfs(&device, "input", "name") else {
            return "".to_string();
        };
        attr
    }

    pub fn phys(&self) -> String {
        let Ok(device) = self.get_device() else {
            return "".to_string();
        };
        let attr = get_attribute_from_tree(&device, "phys");
        if !attr.is_empty() {
            return attr;
        }
        let Some(attr) = get_attribute_from_sysfs(&device, "input", "phys") else {
            return "".to_string();
        };
        attr
    }

    pub fn product(&self) -> String {
        let Ok(device) = self.get_device() else {
            return "".to_string();
        };
        get_attribute_from_tree(&device, "product")
    }

    pub fn serial_number(&self) -> String {
        let Ok(device) = self.get_device() else {
            return "".to_string();
        };
        get_attribute_from_tree(&device, "serial")
    }

    pub fn subsystem(&self) -> String {
        self.subsystem.clone()
    }

    pub fn sysname(&self) -> String {
        self.sysname.clone()
    }

    pub fn syspath(&self) -> String {
        self.syspath.clone()
    }

    pub fn uniq(&self) -> String {
        let Ok(device) = self.get_device() else {
            return "".to_string();
        };
        let attr = get_attribute_from_tree(&device, "uniq");
        if !attr.is_empty() {
            return attr;
        }
        let Some(attr) = get_attribute_from_sysfs(&device, "input", "uniq") else {
            return "".to_string();
        };
        attr
    }

    pub fn get_id(&self) -> String {
        match self.subsystem().as_str() {
            "input" => {
                format!("evdev://{}", self.sysname)
            }
            "hidraw" => {
                format!("hidraw://{}", self.sysname)
            }
            "iio" => {
                format!("iio://{}", self.sysname)
            }
            _ => "".to_string(),
        }
    }
}

/// Looks for the given attribute at the given path using sysfs.
pub fn get_attribute_from_sysfs(
    device: &::udev::Device,
    path: &str,
    attribute: &str,
) -> Option<String> {
    let Some(parent) = device.parent() else {
        return None;
    };

    let input_path_string = format!("{}/{path}", parent.syspath().to_str().unwrap());
    let input_path = Path::new(input_path_string.as_str());
    if !input_path.exists() {
        return None;
    }

    let paths = fs::read_dir(input_path).ok()?;

    for path in paths {
        let p = path.ok()?;
        let path = p.path();
        let attr_path_string = format!("{}/{attribute}", path.display());
        let attr_path = Path::new(attr_path_string.as_str());
        if attr_path.exists() {
            let attr = fs::read_to_string(attr_path)
                .ok()
                .map(|s| s.trim().to_string());
            if let Some(ref str) = attr {
                if str.is_empty() {
                    return None;
                }
            }
            return attr;
        }
    }

    None
}

/// Gets an attribute from the first device in the device tree to match the attribute.
pub fn get_attribute_from_tree(device: &::udev::Device, attribute: &str) -> String {
    // Check if the current device has this attribute
    //log::debug!("Looking for {attribute}.");
    let attr = match device.attribute_value(attribute) {
        Some(attr) => attr,
        None => {
            if let Some(parent) = device.parent() {
                //log::debug!("Could not find {attribute}. Checking parrent...");
                return get_attribute_from_tree(&parent, attribute);
            } else {
                //log::debug!("No more parents to check. Returning nil");
                return "".to_string();
            };
        }
    };
    attr.to_string_lossy().to_string()
}

/// Sets an attribute from the first device in the device tree to match the attribute with the
/// given value.
pub fn set_attribute_on_tree(
    device: &mut ::udev::Device,
    attribute: &str,
    value: &str,
) -> Result<(), Box<dyn Error>> {
    let result = match device.attribute_value(attribute) {
        Some(_) => Ok(device.set_attribute_value(OsStr::new(attribute), OsStr::new(value))?),
        None => {
            if let Some(mut parent) = device.parent() {
                return set_attribute_on_tree(&mut parent, attribute, value);
            } else {
                return Err("Failed to find {attribute} on device tree.".into());
            };
        }
    };
    result
}

impl From<::udev::Device> for UdevDevice {
    fn from(device: ::udev::Device) -> Self {
        let devnode = device
            .devnode()
            .unwrap_or(Path::new(""))
            .to_string_lossy()
            .to_string();
        let subsystem = device
            .subsystem()
            .unwrap_or(OsStr::new(""))
            .to_string_lossy()
            .to_string();
        let sysname = device.sysname().to_string_lossy().to_string();
        let syspath = device.syspath().to_string_lossy().to_string();

        Self {
            devnode,
            subsystem,
            sysname,
            syspath,
        }
    }
}

/// Container for system devices
/// This contains parsed data from a single device entry from 'udevadm info'
#[derive(Debug, Clone, Default)]
pub struct Device {
    /// P: Device path in /sys
    pub path: String,
    /// M: Device name in /sys (i.e. the last component of "P:")
    pub name: String,
    /// R: Device number in /sys (i.e. the numeric suffix of the last component of "P:")
    pub number: u32,
    /// U: Kernel subsystem
    pub subsystem: String,
    /// T: Device type within subsystem
    pub device_type: String,
    /// D: Kernel device node major/minor
    pub node: String,
    /// I: Network interface index
    pub network_index: String,
    /// N: Kernel device node name
    pub node_name: String,
    /// L: Device node symlink priority
    pub symlink_priority: u32,
    /// S: Device node symlink
    pub symlink: Vec<String>,
    /// Q: Block device sequence number (DISKSEQ)
    pub sequence_num: u32,
    /// V: Attached driver
    pub driver: String,
    /// E: Device property
    pub properties: HashMap<String, String>,
}

impl Device {
    /// Returns the parent sysfs device path
    pub fn get_parent(&self) -> Option<String> {
        let path = format!("/sys{}/device", self.path.clone());
        let base_path = format!("/sys{}", self.path.clone());
        let device_path = read_link(path.clone()).ok()?.to_string_lossy().to_string();
        let relative_path = format!("{base_path}/{device_path}");
        let full_path = fs::canonicalize(relative_path).ok()?;
        let full_path = full_path.to_string_lossy().to_string();
        Some(full_path.replacen("/sys", "", 1))
    }

    /// Returns the name of the parent (e.g. input26)
    pub fn get_parent_device_name(&self) -> Option<String> {
        let path = format!("/sys{}/device", self.path.clone());
        let device_path = read_link(path).ok()?;
        let name = device_path.file_name()?;
        Some(name.to_string_lossy().to_string())
    }

    /// Returns a udev rule that will match the given device
    pub fn get_match_rule(&self) -> Option<String> {
        let subsystem = self.subsystem.clone();

        // Create a match rule based on subsystem
        let match_rule = match subsystem.as_str() {
            "hidraw" => {
                let name = self.name.clone();
                Some(format!(r#"SUBSYSTEMS=="{subsystem}", KERNEL=="{name}""#))
            }
            "input" => {
                let rule_fn = || {
                    let Some(device_name) = self.get_parent_device_name() else {
                        return None;
                    };
                    let Some(vid) = self.get_vendor_id() else {
                        return None;
                    };
                    let Some(pid) = self.get_product_id() else {
                        return None;
                    };

                    Some(format!(
                        r#"SUBSYSTEMS=="{subsystem}", KERNELS=="{device_name}", ATTRS{{id/vendor}}=="{vid}", ATTRS{{id/product}}=="{pid}""#
                    ))
                };
                rule_fn()
            }
            _ => None,
        };

        match_rule
    }

    /// Returns the vendor id for the given device. Will only work with event
    /// devices.
    pub fn get_vendor_id(&self) -> Option<String> {
        let path = format!("/sys{}/device/id/vendor", self.path.clone());
        let Some(id) = fs::read_to_string(path).ok() else {
            return None;
        };
        Some(id.replace('\n', ""))
    }

    /// Returns the product id for the given device. Will only work with event
    /// devices.
    pub fn get_product_id(&self) -> Option<String> {
        let path = format!("/sys{}/device/id/product", self.path.clone());
        let Some(id) = fs::read_to_string(path).ok() else {
            return None;
        };
        Some(id.replace('\n', ""))
    }
}
