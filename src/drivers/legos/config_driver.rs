use crate::udev::device::{AttributeGetter, AttributeSetter, UdevDevice};
use std::error::Error;
use udev::Device;
use version_compare::{compare, Cmp};

use super::{CFG_IID, PIDS, VID};

pub struct ConfigDriver {
    _device: UdevDevice,
}

impl ConfigDriver {
    pub fn new(udevice: UdevDevice) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let vid = udevice.id_vendor();
        let pid = udevice.id_product();
        let iid = udevice.interface_number();

        if vid != VID || iid != CFG_IID || !PIDS.contains(&pid) {
            return Err(
                format!("'{}' is not a Legion Go S Config Device", udevice.devnode()).into(),
            );
        }

        let device = udevice.get_device()?;
        let Some(parent) = device.parent() else {
            return Err("Failed to get device parent".into());
        };

        let Some(driver) = parent.driver() else {
            return Err("Failed to identify device driver".into());
        };

        if driver != "lenovo-legos-hid" {
            return Err("Device is not using the lenovo-legos-hid driver.".into());
        }

        let mcu_version = device.get_attribute_from_tree("mcu_version");
        if compare(mcu_version.as_str(), "0.0.3.4") == Ok(Cmp::Lt) {
            log::warn!("MCU firmware v{mcu_version} does not meet the minimum supported feature level. Update to v0.0.3.4 or greater to ensure full compatibility.");
        }

        let tp_vendor = device.get_attribute_from_tree("touchpad/manufacturer");
        let tp_version = device.get_attribute_from_tree("touchpad/version");

        match tp_vendor.as_str() {
            "BetterLife" => {
                if compare(tp_version.as_str(), "15") == Ok(Cmp::Lt) {
                    log::warn!("Touchpad firmware v{tp_version} does not meet the minimum supported feature level. Update to v15 or greater to ensure full compatibility.");
                };
            }

            "SIPO" => {
                if compare(tp_version.as_str(), "20") == Ok(Cmp::Lt) {
                    log::warn!("Touchpad firmware v{tp_version} does not meet the minimum supported feature level. Update to v20 or greater to ensure full compatibility.");
                };
            }
            _ => {
                log::warn!("Unable to determine Touchpad vendor. Update your device to the latest firmware to ensure full compatibility.");
            }
        }

        set_attribute(device.clone(), "gamepad/auto_sleep_time", "0");
        set_attribute(device.clone(), "gamepad/dpad_mode", "8-way");
        set_attribute(device.clone(), "gamepad/mode", "xinput");
        set_attribute(device.clone(), "gamepad/poll_rate", "250");
        set_attribute(device.clone(), "os_mode", "linux");
        set_attribute(device.clone(), "touchpad/linux_mode", "absolute");

        Ok(Self { _device: udevice })
    }
}

fn set_attribute(mut device: Device, attribute: &str, value: &str) {
    match device.set_attribute_on_tree(attribute, value) {
        Ok(_) => log::debug!("set {attribute} to {value}"),
        Err(e) => log::warn!("Could not set {attribute} to {value}: {e:?}"),
    }
}
