// References:
// - https://github.com/zezba9000/MSI-Claw-Gamepad-Mode/blob/main/main.c
// - https://github.com/NeroReflex/hid-msi-claw-dkms/blob/main/hid-msi-claw.c
use std::{error::Error, ffi::CString};

use hidapi::HidDevice;
use packed_struct::PackedStruct;

use crate::{drivers::msi_claw::hid_report::Command, udev::device::UdevDevice};

use super::hid_report::{GamepadMode, MkeysFunction, PackedCommandReport};

// Hardware ID's
pub const VID: u16 = 0x0db0;
pub const PID: u16 = 0x1901;

pub struct Driver {
    device: HidDevice,
}

impl Driver {
    pub fn new(udevice: UdevDevice) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let vid = udevice.id_vendor();
        let pid = udevice.id_product();
        if VID != vid || PID != pid {
            return Err(format!("'{}' is not a MSI Claw controller", udevice.devnode()).into());
        }

        // Open the hidraw device
        let path = udevice.devnode();
        let path = CString::new(path)?;
        let api = hidapi::HidApi::new()?;
        let device = api.open_path(&path)?;

        Ok(Self { device })
    }

    pub fn poll(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        let mut buf = [0; 8];
        let bytes_read = self.device.read(&mut buf[..])?;
        let slice = &buf[..bytes_read];

        if bytes_read > 0 {
            log::debug!("Got response bytes: {slice:?}");
            let report = PackedCommandReport::unpack(&buf)?;
            log::debug!("Response: {report}");

            if report.command == Command::GamepadModeAck {
                let mode: GamepadMode = report.arg1.into();
                log::debug!("Current gamepad mode: {mode:?}");
            }
        }

        Ok(())
    }

    // Configure the device to be in the given mode
    // TODO: Update to use sysfs interface when kernel support is upstreamed
    pub fn set_mode(
        &self,
        mode: GamepadMode,
        mkeys: MkeysFunction,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let report = PackedCommandReport::switch_mode(mode, mkeys);
        let data = report.pack()?;
        self.device.write(&data)?;

        Ok(())
    }

    /// Send a get mode request to the device
    pub fn get_mode(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        let report = PackedCommandReport::read_mode();
        let data = report.pack()?;
        self.device.write(&data)?;

        Ok(())
    }
}
