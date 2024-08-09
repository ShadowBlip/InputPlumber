use std::{error::Error, ffi::OsStr, time::Duration};

use udev::Device;

use crate::udev::device::{AttributeGetter, AttributeSetter, UdevDevice};

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
        // TODO: When resuming from sleep, if mcu_powersave is set, the device init takes longer.
        // inotify will trigger this driver before the attribute tree is fully built and the driver
        // will error and exit, leaving some controls unusable. We should find a way to only
        // trigger this driver or continue processing once we know the driver has fully init.
        std::thread::sleep(Duration::from_millis(300));

        // Set the controller buttons to the correct values at startup
        let device = udevice.get_device()?;
        let Some(mut parent) = device.parent() else {
            return Err("Failed to get device parent".into());
        };
        let Some(driver) = parent.driver() else {
            return Err("Failed to identify device driver".into());
        };
        // Apply settings for the controller
        if driver.to_str().unwrap() == "asus_rog_ally" {
            let if_num = device.get_attribute_from_tree("bInterfaceNumber");
            let if_num = if_num.as_str();
            match if_num {
                "02" => {
                    // Ally and Ally X, map back buttons and ensure it is in gamepad mode.
                    log::debug!("Setting buttons and gamepad mode.");
                    set_attribute(device.clone(), "btn_m1/remap", "KB_F15")?;
                    set_attribute(device.clone(), "btn_m2/remap", "KB_F14")?;
                    set_attribute(device, "gamepad_mode", "1")?;
                    //TODO: Figure out why this fails and manually running the same thing
                    //doesn't.
                    //set_attribute(device, "apply", "1")?;
                    let result =
                        parent.set_attribute_value(OsStr::new("apply_all"), OsStr::new("1"));
                    match result {
                        Ok(_) => log::debug!("set apply_all to 1"),
                        Err(e) => return Err(format!("Could set apply_all to 1: {e:?}").into()),
                    };
                }
                "05" => {
                    // Ally X only, switch from driver emiting a BTN_MODE with CC and a
                    // BTN_MODE/BTN_SOUTH chord with AC to the same events as original
                    // Ally so we can capture them as the Guide and QuickAccess Capabilities.
                    log::debug!("Setting qam mode.");
                    set_attribute(device, "qam_mode", "0")?;
                }
                _ => return Err(format!("Invalid interface number {if_num} provided.").into()),
            };
        } else {
            return Err("Device is not using the asus_rog_ally driver.".into());
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
