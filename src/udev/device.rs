use std::{
    collections::HashMap,
    fs::{self, read_link},
    path::Path,
};

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
    /// Returns true if the given device is virtual
    pub fn is_virtual(&self) -> bool {
        self.path.starts_with("/devices/virtual")
    }

    /// Return the 'uniq' value from input siblings
    pub fn get_uniq(&self) -> Option<String> {
        let parent = self.get_parent()?;
        let input_path_string = format!("/sys{parent}/input");
        let input_path = Path::new(input_path_string.as_str());
        if !input_path.exists() {
            return None;
        }

        let paths = fs::read_dir(input_path).ok()?;

        for path in paths {
            let p = path.ok()?;
            let path = p.path();
            let uniq_path_string = format!("{}/uniq", path.display());
            let uniq_path = Path::new(uniq_path_string.as_str());
            if uniq_path.exists() {
                let uniq = fs::read_to_string(uniq_path)
                    .ok()
                    .map(|s| s.trim().to_string());
                if let Some(ref str) = uniq {
                    if str.is_empty() {
                        return None;
                    }
                }
                return uniq;
            }
        }

        None
    }

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
