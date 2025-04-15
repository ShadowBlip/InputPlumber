use std::error::Error;

use udev::Device;

use crate::udev::device::{AttributeGetter, AttributeSetter, UdevDevice};

// Hardware ID's
const ZONE_PID: u16 = 0x1590;
pub const PIDS: [u16; 1] = [ZONE_PID];
pub const VID: u16 = 0x1ee9;
pub const VID_ALT: u16 = 0x1e19;
pub const VIDS: [u16; 2] = [VID, VID_ALT];

pub struct Driver {
    _device: UdevDevice,
}

impl Driver {
    pub fn new(udevice: UdevDevice) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let vid = udevice.id_vendor();
        let pid = udevice.id_product();
        if !VIDS.contains(&vid) || !PIDS.contains(&pid) {
            return Err(format!("'{}' is not an Zotac Zone controller", udevice.devnode()).into());
        }

        // Set the controller buttons to the correct values at startup
        let mut device = udevice.get_device()?;
        let Some(parent) = device.parent() else {
            return Err("Failed to get device parent".into());
        };
        let Some(driver) = parent.driver() else {
            return Err("Failed to identify device driver".into());
        };
        // Apply settings for the controller
        let drv_name = driver.to_str().unwrap();
        if drv_name == "zotac_zone_hid" || drv_name == "hid_zotac_zone" {
            let if_num = device.get_attribute_from_tree("bInterfaceNumber");
            if if_num.parse().unwrap_or(-1) == 3 {
                if set_attribute(&mut device, "qam_mode", "0").is_ok() {
                    log::debug!("Setting qam mode on interface {if_num}.");
                }
                if set_attribute(&mut device, "btn_m2/remap/keyboard", "home").is_ok() {
                    log::debug!("Setting btn_m2/remap/keyboard on interface {if_num} to 'home'.");
                }
                if set_attribute(&mut device, "btn_m1/remap/keyboard", "end").is_ok() {
                    log::debug!("Setting btn_m1/remap/keyboard on interface {if_num} to 'end'.");
                }
            }
        } else {
            return Err("Device is not using the zotac_zone driver.".into());
        }

        Ok(Self { _device: udevice })
    }
}

fn set_attribute(
    device: &mut Device,
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
