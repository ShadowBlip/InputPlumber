use std::{error::Error, ffi::OsStr};

use udev::Device;

use crate::udev::device::{AttributeSetter, UdevDevice};

// Hardware ID's
const ALLY_PID: u16 = 0x1abe;
const ALLYX_PID: u16 = 0x1b4c;
pub const PIDS: [u16; 2] = [ALLY_PID, ALLYX_PID];
pub const VID: u16 = 0x0b05;

pub struct Driver {
    _device: UdevDevice,
}

impl Driver {
    pub fn new(udevice: UdevDevice) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let vid = udevice.id_vendor();
        let pid = udevice.id_product();
        if VID != vid || !PIDS.contains(&pid) {
            return Err(format!("'{}' is not an ROG Ally controller", udevice.devnode()).into());
        }

        // Set the controller buttons to the correct values at startup
        let device = udevice.get_device()?;
        let Some(mut parent) = device.parent() else {
            return Err("Failed to get device parent".into());
        };
        let Some(driver) = parent.driver() else {
            return Err("Failed to identify device driver".into());
        };
        if driver.to_str().unwrap() == "asus" {
            set_attribute(device.clone(), "btn_m1/remap", "kb_f15")?;
            set_attribute(device.clone(), "btn_m2/remap", "kb_f14")?;
            set_attribute(device, "gamepad_mode", "1")?;
            //TODO: Figure out why this fails and manually running the same thing
            //doesn't.
            //set_attribute(device, "apply", "1")?;
            let result = parent.set_attribute_value(OsStr::new("apply"), OsStr::new("1"));
            match result {
                Ok(_) => log::debug!("set apply to 1"),
                Err(e) => return Err(format!("Could set apply to 1: {e:?}").into()),
            };
        } else {
            return Err("Device is not an asus device.".into());
        }

        Ok(Self { _device: udevice })
    }
}

pub fn set_attribute(
    mut device: Device,
    attribute: &str,
    value: &str,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let result = device.set_attribute_on_tree(attribute, value);
    match result {
        Ok(r) => {
            log::debug!("set {attribute} to {value}");
            Ok(r)
        }
        Err(e) => Err(format!("Could not set {attribute} to {value}: {e:?}").into()),
    }
}
