use std::error::Error;
use std::ffi::CString;
use std::time::Duration;

use hidapi::HidDevice;
use packed_struct::PrimitiveEnum;
use udev::Device;

use crate::udev::device::{AttributeSetter, UdevDevice};

use super::hid_report::{
    ButtonId, ButtonMappingRequest, ButtonMappingResponse, HID_KEY_END, HID_KEY_HOME, REPORT_SIZE,
};

// Hardware ID's
const ZONE_PID: u16 = 0x1590;
const ZONE_CFG_IF_NUM: i32 = 3;
pub const PIDS: [u16; 1] = [ZONE_PID];
pub const VID: u16 = 0x1ee9;
pub const VID_ALT: u16 = 0x1e19;
pub const VIDS: [u16; 2] = [VID, VID_ALT];

/// How long to wait for the device to acknowledge a configuration command.
const COMMAND_TIMEOUT: Duration = Duration::from_millis(500);

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

        // Set the controller buttons to the correct values at startup. The
        // device is perfectly usable if this fails, so only warn about it.
        if udevice.interface_number() == ZONE_CFG_IF_NUM {
            configure_macro_buttons(&udevice);
        }

        Ok(Self { _device: udevice })
    }
}

/// Bind the rear macro buttons to Home and End, which the `zone1` capability
/// map translates into the paddle capabilities.
///
/// Attempts first to bind with the vendor kernel driver and falls back to
/// raw HID commands over hidraw.
fn configure_macro_buttons(udevice: &UdevDevice) {
    if configure_via_sysfs(udevice) {
        return;
    }

    if let Err(e) = configure_via_hidraw(udevice) {
        log::warn!("Could not configure Zotac Zone macro buttons: {e}");
    }
}

/// Configure the device through the sysfs attributes of the vendor kernel
/// driver, returning whether that driver was there to handle it.
fn configure_via_sysfs(udevice: &UdevDevice) -> bool {
    let Ok(mut device) = udevice.get_device() else {
        return false;
    };
    let Some(parent) = device.parent() else {
        return false;
    };
    let Some(driver) = parent.driver().and_then(|driver| driver.to_str()) else {
        return false;
    };
    if driver != "zotac_zone_hid" && driver != "hid_zotac_zone" {
        log::debug!(
            "Zotac Zone config interface is bound to '{driver}' rather than the vendor driver; \
             configuring it over hidraw instead."
        );
        return false;
    }

    set_attribute(&mut device, "qam_mode", "0");
    set_attribute(&mut device, "btn_m2/remap/keyboard", "home");
    set_attribute(&mut device, "btn_m1/remap/keyboard", "end");

    true
}

/// Configure the device over hidraw. This is a one-shot, uncommitted
/// configuration, so it doesn't override the user's saved device settings and
/// lasts only until the device is power cycled.
fn configure_via_hidraw(udevice: &UdevDevice) -> Result<(), Box<dyn Error + Send + Sync>> {
    let path = CString::new(udevice.devnode())?;
    let api = hidapi::HidApi::new()?;
    let device = api.open_path(&path)?;

    // M2 is the left paddle, M1 the right one. Each button is mapped
    // independently so that one of them failing still leaves the other bound.
    for (button_id, key) in [(ButtonId::M2, HID_KEY_HOME), (ButtonId::M1, HID_KEY_END)] {
        if let Err(e) = send_mapping(&device, button_id, key) {
            log::warn!(
                "Could not map Zotac Zone button {:#04x}: {e}",
                button_id.to_primitive()
            );
        }
    }

    Ok(())
}

/// Send a single button mapping command and wait for the device to accept it.
fn send_mapping(
    device: &HidDevice,
    button_id: ButtonId,
    key: u8,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    device.write(&ButtonMappingRequest::new(button_id, key).to_bytes()?)?;

    // The device also emits unsolicited status frames on this interface, so
    // read until its answer to *this* command turns up.
    let deadline = std::time::Instant::now() + COMMAND_TIMEOUT;
    loop {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        if remaining.is_zero() {
            return Err(
                format!("Timed out mapping button {:#04x}", button_id.to_primitive()).into(),
            );
        }

        let mut buf = [0; REPORT_SIZE];
        let bytes_read = device.read_timeout(&mut buf, remaining.as_millis() as i32)?;
        let Some(status) = ButtonMappingResponse::status(&buf[..bytes_read]) else {
            continue;
        };

        if status != 0 {
            return Err(format!(
                "Device rejected mapping for button {:#04x}: {status:#04x}",
                button_id.to_primitive()
            )
            .into());
        }

        log::debug!(
            "Mapped Zotac Zone button {:#04x} to key {key:#04x}",
            button_id.to_primitive()
        );
        return Ok(());
    }
}

#[inline(always)]
fn set_attribute(device: &mut Device, attribute: &str, value: &str) {
    match device.set_attribute_on_tree(attribute, value) {
        Ok(_) => log::debug!("set {attribute} to {value}"),
        Err(e) => log::error!("Could not set {attribute} to {value}: {e:?}"),
    }
}
