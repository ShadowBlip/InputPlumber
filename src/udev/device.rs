use std::{
    collections::HashMap,
    error::Error,
    ffi::OsStr,
    fs::{self, read_link},
    path::Path,
};

pub trait AttributeGetter {
    /// Looks for the given attribute at the given path using sysfs.
    fn get_attribute_from_sysfs(&self, path: &str, attribute: &str) -> Option<String>;
    /// Gets an attribute from the first device in the device tree to match the attribute.
    fn get_attribute_from_tree(&self, attribute: &str) -> String;
    /// Return the bustype attribute from the device
    fn id_bustype(&self) -> u16;
    /// Returns the product ID of the device
    fn id_product(&self) -> u16;
    /// Returns the vendor ID of the device
    fn id_vendor(&self) -> u16;
    /// Returns the hardware version of the device
    fn id_version(&self) -> u16;
    fn interface_number(&self) -> i32;
    fn manufacturer(&self) -> String;
    fn name(&self) -> String;
    fn phys(&self) -> String;
    fn product(&self) -> String;
    fn serial_number(&self) -> String;
    fn uniq(&self) -> String;
    fn get_attributes(&self) -> HashMap<String, String>;
    /// Returns the value of the given property from the device
    fn get_property(&self, property: &str) -> Option<String>;
    /// Returns device properties for the device. E.g. {"ID_INPUT": "1", ...}
    fn get_properties(&self) -> HashMap<String, String>;
    /// Returns a list of all drivers used for this device. This list will be
    /// in ascending order, with the first item in the list being the first
    /// discovered driver in the device tree.
    fn drivers(&self) -> Vec<String>;
}

impl AttributeGetter for ::udev::Device {
    /// Return the bustype attribute from the device
    fn id_bustype(&self) -> u16 {
        let orig = self.get_attribute_from_tree("id/bustype");
        let stripped = orig.strip_prefix("0x").unwrap_or(orig.as_str());
        let attr = u16::from_str_radix(stripped, 16).unwrap_or(0);
        if attr != 0 {
            return attr;
        }
        let Some(attr) = self.get_attribute_from_sysfs("input", "id/bustype") else {
            return 0;
        };
        u16::from_str_radix(attr.as_str(), 16).unwrap_or(0)
    }

    /// Returns the product ID of the device
    fn id_product(&self) -> u16 {
        let Some(subsystem) = self.subsystem() else {
            return 0;
        };
        match subsystem.to_string_lossy().to_string().as_str() {
            "input" => {
                let orig = self.get_attribute_from_tree("id/product");
                let stripped = orig.strip_prefix("0x").unwrap_or(orig.as_str());
                u16::from_str_radix(stripped, 16).unwrap_or(0)
            }
            _ => {
                let orig = self.get_attribute_from_tree("idProduct");
                let stripped = orig.strip_prefix("0x").unwrap_or(orig.as_str());
                let attr = u16::from_str_radix(stripped, 16).unwrap_or(0);
                if attr != 0 {
                    return attr;
                }
                let Some(attr) = self.get_attribute_from_sysfs("input", "id/product") else {
                    return 0;
                };
                u16::from_str_radix(attr.as_str(), 16).unwrap_or(0)
            }
        }
    }

    /// Returns the vendor ID of the device
    fn id_vendor(&self) -> u16 {
        let Some(subsystem) = self.subsystem() else {
            return 0;
        };
        match subsystem.to_string_lossy().to_string().as_str() {
            "input" => {
                let orig = self.get_attribute_from_tree("id/vendor");
                let stripped = orig.strip_prefix("0x").unwrap_or(orig.as_str());
                u16::from_str_radix(stripped, 16).unwrap_or(0)
            }
            _ => {
                let orig = self.get_attribute_from_tree("idVendor");
                let stripped = orig.strip_prefix("0x").unwrap_or(orig.as_str());
                let attr = u16::from_str_radix(stripped, 16).unwrap_or(0);
                if attr != 0 {
                    return attr;
                }
                let Some(attr) = self.get_attribute_from_sysfs("input", "id/vendor") else {
                    return 0;
                };
                u16::from_str_radix(attr.as_str(), 16).unwrap_or(0)
            }
        }
    }

    /// Returns the hardware version of the device
    fn id_version(&self) -> u16 {
        let orig = self.get_attribute_from_tree("id/version");
        let stripped = orig.strip_prefix("0x").unwrap_or(orig.as_str());
        let attr = u16::from_str_radix(stripped, 16).unwrap_or(0);
        if attr != 0 {
            return attr;
        }
        let Some(attr) = self.get_attribute_from_sysfs("input", "id/version") else {
            return 0;
        };
        u16::from_str_radix(attr.as_str(), 16).unwrap_or(0)
    }

    fn interface_number(&self) -> i32 {
        let orig = self.get_attribute_from_tree("bInterfaceNumber");
        let stripped = orig.strip_prefix("0x").unwrap_or(orig.as_str());
        i32::from_str_radix(stripped, 16).unwrap_or(0)
    }

    fn manufacturer(&self) -> String {
        self.get_attribute_from_tree("manufacturer")
    }

    fn name(&self) -> String {
        let attr = self.get_attribute_from_tree("name");
        if !attr.is_empty() {
            return attr;
        }
        let Some(attr) = self.get_attribute_from_sysfs("input", "name") else {
            return "".to_string();
        };
        attr
    }

    fn phys(&self) -> String {
        let attr = self.get_attribute_from_tree("phys");
        if !attr.is_empty() {
            return attr;
        }
        let Some(attr) = self.get_attribute_from_sysfs("input", "phys") else {
            return "".to_string();
        };
        attr
    }

    fn product(&self) -> String {
        self.get_attribute_from_tree("product")
    }

    fn serial_number(&self) -> String {
        self.get_attribute_from_tree("serial")
    }

    fn uniq(&self) -> String {
        let attr = self.get_attribute_from_tree("uniq");
        if !attr.is_empty() {
            return attr;
        }
        let Some(attr) = self.get_attribute_from_sysfs("input", "uniq") else {
            return "".to_string();
        };
        attr
    }

    /// Returns a list of all drivers used for this device. This list will be
    /// in ascending order, with the first item in the list being the first
    /// discovered driver in the device tree.
    fn drivers(&self) -> Vec<String> {
        let mut drivers = vec![];
        if let Some(driver) = self.driver() {
            let value = driver.to_string_lossy().to_string();
            if !value.is_empty() {
                drivers.push(value);
            }
        }

        // Walk up the device tree and query for each driver
        let mut parent = self.parent();
        while parent.is_some() {
            let current_parent = parent.unwrap();
            if let Some(driver) = current_parent.driver() {
                let value = driver.to_string_lossy().to_string();
                if !value.is_empty() {
                    drivers.push(value);
                }
            }

            parent = current_parent.parent();
        }

        drivers
    }

    /// Looks for the given attribute at the given path using sysfs.
    fn get_attribute_from_sysfs(&self, path: &str, attribute: &str) -> Option<String> {
        let parent = self.parent()?;

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

    /// Recursively gets attributes for this device and all parent devices.
    fn get_attributes(&self) -> HashMap<String, String> {
        let mut attributes = HashMap::new();
        for attr in self.attributes() {
            let key = attr.name().to_string_lossy().to_string();
            if attributes.contains_key(&key) {
                continue;
            }
            let value = attr.value().to_string_lossy().to_string();
            attributes.insert(key, value);
        }

        // Walk up the device tree and query each device
        let mut parent = self.parent();
        while parent.is_some() {
            let current_parent = parent.unwrap();
            for attr in current_parent.attributes() {
                let key = attr.name().to_string_lossy().to_string();
                if attributes.contains_key(&key) {
                    continue;
                }
                let value = attr.value().to_string_lossy().to_string();
                attributes.insert(key, value);
            }

            parent = current_parent.parent();
        }

        attributes
    }

    /// Gets an attribute from the first device in the device tree to match the attribute.
    fn get_attribute_from_tree(&self, attribute: &str) -> String {
        // Check if the current device has this attribute
        //log::debug!("Looking for {attribute}.");
        let attr = match self.attribute_value(attribute) {
            Some(attr) => attr,
            None => {
                if let Some(parent) = self.parent() {
                    //log::debug!("Could not find {attribute}. Checking parrent...");
                    return parent.get_attribute_from_tree(attribute);
                } else {
                    //log::debug!("No more parents to check. Returning nil");
                    return "".to_string();
                };
            }
        };
        attr.to_string_lossy().to_string()
    }

    /// Returns the value of the given property from the device
    fn get_property(&self, property: &str) -> Option<String> {
        self.property_value(property)
            .map(|v| v.to_string_lossy().to_string())
    }

    /// Returns device properties for the device. E.g. {"ID_INPUT": "1", ...}
    fn get_properties(&self) -> HashMap<String, String> {
        let mut properties = HashMap::new();
        for property in self.properties() {
            let key = property.name().to_string_lossy().to_string();
            let value = property.value().to_string_lossy().to_string();
            properties.insert(key, value);
        }

        properties
    }
}

pub trait AttributeSetter {
    /// Sets an attribute from the first device in the device tree to match the attribute with the
    /// given value.
    fn set_attribute_on_tree(&mut self, attribute: &str, value: &str)
        -> Result<(), Box<dyn Error>>;
}

impl AttributeSetter for ::udev::Device {
    /// Sets an attribute from the first device in the device tree to match the attribute with the
    /// given value.
    fn set_attribute_on_tree(
        &mut self,
        attribute: &str,
        value: &str,
    ) -> Result<(), Box<dyn Error>> {
        match self.attribute_value(attribute) {
            Some(_) => {
                log::trace!("Set '{attribute}' on {:?}", self.syspath(),);
                Ok(self.set_attribute_value(OsStr::new(attribute), OsStr::new(value))?)
            }
            None => {
                if let Some(mut parent) = self.parent() {
                    log::trace!(
                        "Couldn't find '{attribute}' on {:?}, checking parent {:?}",
                        self.syspath(),
                        parent.syspath()
                    );
                    parent.set_attribute_on_tree(attribute, value)
                } else {
                    Err(format!("Failed to find '{attribute}' on device tree.").into())
                }
            }
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct UdevDevice {
    devnode: String,
    subsystem: String,
    syspath: String,
    sysname: String,
    name: Option<String>,
    vendor_id: Option<u16>,
    product_id: Option<u16>,
    bus_type: Option<u16>,
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
            name: None,
            vendor_id: None,
            product_id: None,
            bus_type: None,
        }
    }

    /// Returns a udev::Device from the stored syspath.
    pub fn get_device(&self) -> Result<::udev::Device, Box<dyn Error + Send + Sync>> {
        match ::udev::Device::from_syspath(Path::new(self.syspath.as_str())) {
            Ok(device) => Ok(device),
            Err(e) => Err(e.into()),
        }
    }

    /// Returns true if this device is virtual
    pub fn is_virtual(&self) -> bool {
        self.syspath().contains("/devices/virtual") || self.syspath().contains("vhci_hcd")
    }

    /// Returns the devnode of the device. The devnode is the full path to the
    /// device in the "/dev" filesystem. E.g. "/dev/input/event0", "/dev/hidraw0"
    pub fn devnode(&self) -> String {
        self.devnode.clone()
    }

    pub fn devpath(&self) -> String {
        let Ok(device) = self.get_device() else {
            return "".to_string();
        };
        device.devpath().to_string_lossy().to_string()
    }

    /// Recursively returns all drivers associated with the device.
    pub fn drivers(&self) -> Vec<String> {
        let Ok(device) = self.get_device() else {
            return vec![];
        };
        device.drivers()
    }

    /// Return the bustype attribute from the device
    pub fn id_bustype(&self) -> u16 {
        if let Some(bus_type) = self.bus_type {
            return bus_type;
        }
        let Ok(device) = self.get_device() else {
            return 0;
        };
        device.id_bustype()
    }

    /// Returns the product ID of the device
    pub fn id_product(&self) -> u16 {
        if let Some(value) = self.product_id {
            return value;
        }
        let Ok(device) = self.get_device() else {
            return 0;
        };
        device.id_product()
    }

    /// Returns the vendor ID of the device
    pub fn id_vendor(&self) -> u16 {
        if let Some(value) = self.vendor_id {
            return value;
        }
        let Ok(device) = self.get_device() else {
            return 0;
        };
        device.id_vendor()
    }

    /// Returns the hardware version of the device
    pub fn id_version(&self) -> u16 {
        let Ok(device) = self.get_device() else {
            return 0;
        };
        device.id_version()
    }

    /// Returns the USB interface number
    pub fn interface_number(&self) -> i32 {
        let Ok(device) = self.get_device() else {
            return -1;
        };
        device.interface_number()
    }

    /// Returns the USB manufacturer string
    pub fn manufacturer(&self) -> String {
        let Ok(device) = self.get_device() else {
            return "".to_string();
        };
        device.manufacturer()
    }

    /// Returns the name property of the device
    pub fn name(&self) -> String {
        if let Some(ref value) = self.name {
            return value.clone();
        }
        let Ok(device) = self.get_device() else {
            return "".to_string();
        };
        device.name()
    }

    /// Returns the phys property of the device
    pub fn phys(&self) -> String {
        let Ok(device) = self.get_device() else {
            return "".to_string();
        };
        device.phys()
    }

    /// Returns the product string of the device
    pub fn product(&self) -> String {
        let Ok(device) = self.get_device() else {
            return "".to_string();
        };
        device.product()
    }

    /// Returns the serial number of the device
    pub fn serial_number(&self) -> String {
        let Ok(device) = self.get_device() else {
            return "".to_string();
        };
        device.serial_number()
    }

    /// Returns the subsystem that the device belongs to. E.g. "input", "hidraw"
    pub fn subsystem(&self) -> String {
        self.subsystem.clone()
    }

    /// Returns the system name of the device. E.g. "event0", "hidraw0"
    pub fn sysname(&self) -> String {
        self.sysname.clone()
    }

    /// Returns the syspath of the device.
    ///
    /// The path is an absolute path and includes the sys mount point. For example, the syspath for
    /// `tty0` could be `/sys/devices/virtual/tty/tty0`, which includes the sys mount point,
    /// `/sys`.
    pub fn syspath(&self) -> String {
        self.syspath.clone()
    }

    /// Returns the uniq property of the device
    pub fn uniq(&self) -> String {
        let Ok(device) = self.get_device() else {
            return "".to_string();
        };
        device.uniq()
    }

    /// Return a unique identifier for the device based on the subsystem and
    /// sysname. E.g. "evdev://event3", "hidraw://hidraw0"
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

    /// Recursively gets attributes for this device and all parent devices.
    pub fn get_attributes(&self) -> HashMap<String, String> {
        let Ok(device) = self.get_device() else {
            return HashMap::new();
        };
        device.get_attributes()
    }

    /// Gets an attribute from the first device in the device tree to match the attribute.
    pub fn get_attribute_from_tree(&self, attribute: &str) -> Option<String> {
        let Ok(device) = self.get_device() else {
            return None;
        };
        let value = device.get_attribute_from_tree(attribute);
        if value.is_empty() {
            None
        } else {
            Some(value)
        }
    }

    /// Returns the value of the given property from the device
    pub fn get_property(&self, property: &str) -> Option<String> {
        let Ok(device) = self.get_device() else {
            return None;
        };
        device.get_property(property)
    }

    /// Returns device properties for the device. E.g. {"ID_INPUT": "1", ...}
    pub fn get_properties(&self) -> HashMap<String, String> {
        let Ok(device) = self.get_device() else {
            return HashMap::new();
        };
        device.get_properties()
    }
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
            name: Some(device.name()),
            vendor_id: Some(device.id_vendor()),
            product_id: Some(device.id_product()),
            bus_type: Some(device.id_bustype()),
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
    pub number: u64,
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
    /// Returns a path to the sysfs device link, such as
    /// "/sys/devices/pci0000:00/0000:00:08.1/0000:09:00.3/usb1/1-3/1-3:1.2/0003:0B05:1ABE.0003/hidraw/hidraw2/device"
    fn sysnode_device_link(&self) -> String {
        let s = self.path.as_str();
        match s.ends_with("/device") {
            true => String::from(s),
            false => format!("{}/device", s)
        }
    }

    /// Returns the parent sysfs device path
    pub fn get_parent(&self) -> Option<String> {
        let path = self.sysnode_device_link();
        let s = path.as_str();
        let base_path = if s.ends_with("/device") { &s[..s.len()-7] } else { &s };
        let device_path = read_link(path.clone()).ok()?.to_string_lossy().to_string();
        let relative_path = format!("{base_path}/{device_path}");
        let full_path = fs::canonicalize(relative_path).ok()?;
        let full_path = full_path.to_string_lossy().to_string();
        Some(full_path.replacen("/sys", "", 1))
    }

    /// Returns the name of the parent (e.g. input26)
    pub fn get_parent_device_name(&self) -> Option<String> {
        let path = self.sysnode_device_link();
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
                    let device_name = self.get_parent_device_name()?;
                    let vid = self.get_vendor_id()?;
                    let pid = self.get_product_id()?;

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
        let path = format!("{}/id/vendor", self.sysnode_device_link());
        let id = fs::read_to_string(path).ok()?;
        Some(id.replace('\n', ""))
    }

    /// Returns the product id for the given device. Will only work with event
    /// devices.
    pub fn get_product_id(&self) -> Option<String> {
        let path = format!("{}/id/product", self.sysnode_device_link());
        let id = fs::read_to_string(path).ok()?;
        Some(id.replace('\n', ""))
    }
}
