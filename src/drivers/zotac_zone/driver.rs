use std::error::Error;

use udev::Device;

use crate::udev::device::{AttributeGetter, AttributeSetter, UdevDevice};

// Hardware ID's
const ZONE_PID: u16 = 0x1590;
const ZONE_CFG_IF_NUM: i32 = 3;
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
        // Apply settings for the controller. Do error and crash IP as the
        // device is still usable.
        let drv_name = driver.to_str().unwrap();
        if drv_name == "zotac_zone_hid" || drv_name == "hid_zotac_zone" {
            if device.interface_number() == ZONE_CFG_IF_NUM {
                set_attribute(&mut device, "qam_mode", "0");
                set_attribute(&mut device, "btn_m2/remap/keyboard", "home");
                set_attribute(&mut device, "btn_m1/remap/keyboard", "end");
            }
        } else {
            return Err("Device is not using the zotac_zone driver.".into());
        }

        Ok(Self { _device: udevice })
    }
}

#[inline(always)]
fn set_attribute(device: &mut Device, attribute: &str, value: &str) {
    match device.set_attribute_on_tree(attribute, value) {
        Ok(_) => log::debug!("set {attribute} to {value}"),
        Err(e) => log::error!("Could not set {attribute} to {value}: {e:?}"),
    }
}
