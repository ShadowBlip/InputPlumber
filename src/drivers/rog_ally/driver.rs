use crate::udev::device::{AttributeGetter, AttributeSetter, UdevDevice};
use hidapi::HidDevice;
use packed_struct::PackedStruct;
use std::{error::Error, ffi::CString, time::Duration};
use udev::Device;

use super::{
    event::OutputEvent, has_haptic_triggers, hid_report::PackedRumbleExtendedOutputReport, PIDS,
    VID,
};

pub struct Driver {
    _device: UdevDevice,
    /// HID device on the gamepad interface for haptics writes
    pub(crate) hid_device: Option<HidDevice>,
}

impl Driver {
    pub fn new(udevice: UdevDevice) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let vid = udevice.id_vendor();
        let pid = udevice.id_product();
        if VID != vid || !PIDS.contains(&pid) {
            return Err(format!("'{}' is not an ROG Ally controller", udevice.devnode()).into());
        }

        // Wait for device initialization due to powersave_mode
        std::thread::sleep(Duration::from_millis(300));

        let device = udevice.get_device()?;
        let Some(mut parent) = device.parent() else {
            return Err("Failed to get device parent".into());
        };

        let Some(driver) = parent.driver() else {
            return Err("Failed to identify device driver".into());
        };

        if driver != "asus_rog_ally" && driver != "asus_ally_hid" {
            return Err("Device is not using the asus_rog_ally driver.".into());
        }

        let mut hid_device: Option<HidDevice> = None;

        let if_num = device.get_attribute_from_tree("bInterfaceNumber");
        match if_num.as_str() {
            "02" => {
                log::trace!("Setting buttons and gamepad mode.");
                set_attribute(device.clone(), "btn_m1/remap", "KB_F15");
                set_attribute(device.clone(), "btn_m2/remap", "KB_F14");
                set_attribute(device.clone(), "gamepad_mode", "1");
                parent
                    .set_attribute_value("apply_all", "1")
                    .map_err(|e| log::warn!("Could not set apply_all to 1: {e:?}"))
                    .ok();
            }
            "05" => {
                // Configure QAM mode for Ally X
                log::trace!("Setting qam mode.");
                set_attribute(device, "qam_mode", "0");

                if has_haptic_triggers() {
                    let path = CString::new(udevice.devnode())?;
                    let api = hidapi::HidApi::new()?;
                    hid_device = Some(api.open_path(&path)?);
                }
            }
            _ => return Err(format!("Invalid interface number {if_num} provided.").into()),
        };

        Ok(Self {
            _device: udevice,
            hid_device,
        })
    }

    /// Write an output event to the controller
    pub fn write_event(&self, event: &OutputEvent) -> Result<(), Box<dyn Error + Send + Sync>> {
        let Some(hid_device) = &self.hid_device else {
            return Ok(());
        };

        match event {
            &OutputEvent::RumbleExtended {
                left_trigger,
                right_trigger,
                left_motor,
                right_motor,
            } => {
                let mut report = PackedRumbleExtendedOutputReport::off();
                report.magnitude_left_trigger = left_trigger.min(100);
                report.magnitude_right_trigger = right_trigger.min(100);
                report.magnitude_left = left_motor.min(100);
                report.magnitude_right = right_motor.min(100);

                let buf = report.pack()?;
                log::trace!("Writing rumble report: {report}");
                hid_device.write(&buf)?;
            }
        }

        Ok(())
    }
}

fn set_attribute(mut device: Device, attribute: &str, value: &str) {
    // log errors but don't bomb out of InputPlumber by returning them,
    // this should allow at least some usability of devices if errored
    match device.set_attribute_on_tree(attribute, value) {
        Ok(_) => log::debug!("set {attribute} to {value}"),
        Err(e) => log::warn!("Could not set {attribute} to {value}: {e:?}"),
    }
}
