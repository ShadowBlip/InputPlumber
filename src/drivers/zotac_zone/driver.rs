use std::error::Error;
use std::ffi::CString;
use std::time::Duration;

use hidapi::HidDevice;
use udev::Device;

use crate::udev::device::{AttributeSetter, UdevDevice};

use super::hid_report::{
    mapping_status, set_keyboard_mapping, BUTTON_M1, BUTTON_M2, HID_KEY_END, HID_KEY_HOME,
    REPORT_SIZE,
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
/// The buttons ship with an empty mapping, so the firmware sends nothing at all
/// while they're unbound - they show up on no evdev node and in no HID report.
/// The vendor kernel driver binds them through sysfs; where that driver isn't
/// loaded, the same thing can be asked of the device directly over hidraw.
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

/// Configure the device by speaking the vendor protocol over hidraw.
///
/// The mapping is deliberately not committed with the protocol's save command,
/// so it lasts only until the device is power cycled and the user's stored
/// configuration is left untouched.
fn configure_via_hidraw(udevice: &UdevDevice) -> Result<(), Box<dyn Error + Send + Sync>> {
    let path = CString::new(udevice.devnode())?;
    let api = hidapi::HidApi::new()?;
    let device = api.open_path(&path)?;

    // M2 is the left paddle, M1 the right one. Each button is mapped
    // independently so that one of them failing still leaves the other bound.
    for (sequence, button_id, key) in [(0, BUTTON_M2, HID_KEY_HOME), (1, BUTTON_M1, HID_KEY_END)] {
        if let Err(e) = send_mapping(&device, sequence, button_id, key) {
            log::warn!("Could not map Zotac Zone button {button_id:#04x}: {e}");
        }
    }

    Ok(())
}

/// Send a single button mapping command and wait for the device to accept it.
fn send_mapping(
    device: &HidDevice,
    sequence: u8,
    button_id: u8,
    key: u8,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    device.write(&set_keyboard_mapping(sequence, button_id, key))?;

    // The device also emits unsolicited status frames on this interface, so
    // read until its answer to *this* command turns up.
    let deadline = std::time::Instant::now() + COMMAND_TIMEOUT;
    loop {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        if remaining.is_zero() {
            return Err(format!("Timed out mapping button {button_id:#04x}").into());
        }

        let mut buf = [0; REPORT_SIZE];
        let bytes_read = device.read_timeout(&mut buf, remaining.as_millis() as i32)?;
        let Some(status) = mapping_status(&buf[..bytes_read]) else {
            continue;
        };

        if status != 0 {
            return Err(format!(
                "Device rejected mapping for button {button_id:#04x}: {status:#04x}"
            )
            .into());
        }

        log::debug!("Mapped Zotac Zone button {button_id:#04x} to key {key:#04x}");
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
