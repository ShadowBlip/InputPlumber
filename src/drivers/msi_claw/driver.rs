// References:
// - https://github.com/zezba9000/MSI-Claw-Gamepad-Mode/blob/main/main.c
// - https://github.com/NeroReflex/hid-msi-claw-dkms/blob/main/hid-msi-claw.c
use std::{error::Error, ffi::CString, time::Duration};

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
        device.set_blocking_mode(false)?;

        Ok(Self { device })
    }

    pub fn poll(&self) -> Result<Option<PackedCommandReport>, Box<dyn Error + Send + Sync>> {
        let mut buf = [0; 8];
        let bytes_read = self.device.read(&mut buf[..])?;
        if bytes_read == 0 {
            return Ok(None);
        }
        let slice = &buf[..bytes_read];

        log::debug!("Got response bytes: {slice:?}");
        let report = PackedCommandReport::unpack(&buf)?;
        log::debug!("Response: {report}");

        if report.command == Command::GamepadModeAck {
            let mode: GamepadMode = report.arg1.into();
            log::debug!("Current gamepad mode: {mode:?}");
        }

        Ok(Some(report))
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

        // The Claw appears to use a ring buffer of 64 bytes, so keep writing
        // the command until the ring buffer is full and an ACK response is
        // received. Attempts (buffer_size / report_size) number of times (8).
        for _ in 0..8 {
            // Write the SetMode command
            self.device.write(&data)?;
            std::thread::sleep(Duration::from_millis(50));

            // Poll the device for an acknowlgement response
            let Some(report) = self.poll()? else {
                continue;
            };

            // TODO: Validate that the device switched gamepad modes
            match report.command {
                Command::Ack | Command::GamepadModeAck => break,
                _ => break,
            }
        }

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
