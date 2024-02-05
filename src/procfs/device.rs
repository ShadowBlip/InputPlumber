use std::{fs, io};

/// Container for a sysfs ID
/// E.g. I: Bus=0003 Vendor=045e Product=028e Version=0120
#[derive(Debug, Clone)]
pub struct ID {
    pub bus_type: String,
    pub vendor: String,
    pub product: String,
    pub version: String,
}

/// Container representing a device bitmap
/// E.g. B: KEY=7cdb000000000000 0 0 0 0
#[derive(Debug, Clone)]
pub struct Bitmap {
    pub kind: String,
    pub values: String,
}

/// Container for sysfs input devices
/// This contains parsed data from a single device entry in /proc/bus/input/devices.
#[derive(Debug, Clone)]
pub struct Device {
    pub phys_path: String,
    pub name: String,
    pub id: ID,
    pub sysfs_path: String,
    pub unique_id: String,
    pub handlers: Vec<String>,
    pub bitmaps: Vec<Bitmap>,
}

impl Device {
    fn new() -> Device {
        Device {
            phys_path: "".to_string(),
            name: "".to_string(),
            id: ID {
                bus_type: "".to_string(),
                vendor: "".to_string(),
                product: "".to_string(),
                version: "".to_string(),
            },
            sysfs_path: "".to_string(),
            unique_id: "".to_string(),
            handlers: Vec::new(),
            bitmaps: Vec::new(),
        }
    }
}

/// Returns a list of sysfs input devices that are currently detected. This
/// function parses the file at /proc/bus/input/devices
pub fn get_all() -> io::Result<Vec<Device>> {
    let mut devices: Vec<Device> = Vec::new();

    // Get the contents of the input devices file
    let path = "/proc/bus/input/devices";
    let result = fs::read_to_string(path);
    let content = result?;
    let lines = content.split('\n');

    // Parse the output
    let mut device: Option<Device> = None;
    for line in lines {
        if line.starts_with("I: ") {
            let mut new_device = Device::new();
            let line = line.replace("I: ", "");

            let parts = line.split_whitespace();
            for part in parts {
                let mut pair = part.split('=');
                if pair.clone().count() != 2 {
                    continue;
                }
                let key = pair.next().unwrap();
                let value = pair.last().unwrap();
                if key == "Bus" {
                    new_device.id.bus_type = String::from(value);
                } else if key == "Vendor" {
                    new_device.id.vendor = String::from(value);
                } else if key == "Product" {
                    new_device.id.product = String::from(value);
                } else if key == "Version" {
                    new_device.id.version = String::from(value);
                }
            }
            device = Some(new_device);
        } else if line.starts_with("N: ") {
            if let Some(device) = &mut device {
                let line = line.replace("N: ", "");
                let parts = line.splitn(2, '=');
                if parts.clone().count() != 2 {
                    continue;
                }
                device.name = parts.last().unwrap().replace('\"', "");
            }
        } else if line.starts_with("P: ") {
            match device {
                None => continue,
                Some(ref mut d) => {
                    let line = line.replace("P: ", "");
                    let parts = line.splitn(2, '=');
                    if parts.clone().count() != 2 {
                        continue;
                    }
                    d.phys_path = parts.last().unwrap().to_string();
                }
            }
        } else if line.starts_with("S: ") {
            match device {
                None => continue,
                Some(ref mut d) => {
                    let line = line.replace("S: ", "");
                    let parts = line.splitn(2, '=');
                    if parts.clone().count() != 2 {
                        continue;
                    }
                    d.sysfs_path = parts.last().unwrap().to_string();
                }
            }
        } else if line.starts_with("U: ") {
            match device {
                None => continue,
                Some(ref mut d) => {
                    let line = line.replace("U: ", "");
                    let parts = line.splitn(2, '=');
                    if parts.clone().count() != 2 {
                        continue;
                    }
                    d.unique_id = parts.last().unwrap().to_string();
                }
            }
        } else if line.starts_with("H: ") {
            match device {
                None => continue,
                Some(ref mut d) => {
                    let line = line.replace("H: ", "");
                    let parts = line.splitn(2, '=');
                    if parts.clone().count() != 2 {
                        continue;
                    }
                    let list = parts.last().unwrap().split_whitespace();
                    for handler in list {
                        d.handlers.push(handler.to_string());
                    }
                }
            }
        } else if line.starts_with("B: ") {
            // TODO
        } else if line.is_empty() {
            match device {
                None => continue,
                Some(ref mut d) => {
                    let new_device = d.clone();
                    devices.push(new_device);
                    device = None;
                }
            }
        }
    }

    Ok(devices)
}
