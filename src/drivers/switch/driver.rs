use std::{error::Error, ffi::CString};

use hidapi::HidDevice;
use packed_struct::prelude::*;

use super::{
    event::Event,
    hid_report::{PackedInputDataReport, ReportType},
};

// Hardware IDs
pub const VID: u16 = 0x057e;
pub const PID: u16 = 0x2009;

/// Size of the HID packet
const PACKET_SIZE: usize = 64 + 35;

/// Nintendo Switch input driver
pub struct Driver {
    state: Option<PackedInputDataReport>,
    device: HidDevice,
}

impl Driver {
    pub fn new(path: String) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let path = CString::new(path)?;
        let api = hidapi::HidApi::new()?;
        let device = api.open_path(&path)?;
        let info = device.get_device_info()?;
        if info.vendor_id() != VID || info.product_id() != PID {
            return Err("Device '{path}' is not a Switch Controller".into());
        }

        Ok(Self {
            device,
            state: None,
        })
    }

    /// Poll the device and read input reports
    pub fn poll(&mut self) -> Result<Vec<Event>, Box<dyn Error + Send + Sync>> {
        log::debug!("Polling device");

        // Read data from the device into a buffer
        let mut buf = [0; PACKET_SIZE];
        let bytes_read = self.device.read(&mut buf[..])?;

        // Handle the incoming input report
        let events = self.handle_input_report(buf, bytes_read)?;

        Ok(events)
    }

    /// Unpacks the buffer into a [PackedInputDataReport] structure and updates
    /// the internal gamepad state
    fn handle_input_report(
        &mut self,
        buf: [u8; PACKET_SIZE],
        bytes_read: usize,
    ) -> Result<Vec<Event>, Box<dyn Error + Send + Sync>> {
        // Read the report id
        let report_id = buf[0];
        let report_type = ReportType::try_from(report_id)?;
        log::debug!("Received report: {report_type:?}");

        let slice = &buf[..bytes_read];
        match report_type {
            ReportType::CommandOutputReport => todo!(),
            ReportType::McuUpdateOutputReport => todo!(),
            ReportType::BasicOutputReport => todo!(),
            ReportType::McuOutputReport => todo!(),
            ReportType::AttachmentOutputReport => todo!(),
            ReportType::CommandInputReport => todo!(),
            ReportType::McuUpdateInputReport => todo!(),
            ReportType::BasicInputReport => {
                let sized_buf = slice.try_into()?;
                let input_report = PackedInputDataReport::unpack(sized_buf)?;

                // Print input report for debugging
                log::debug!("\x1B[2J\x1B[1;1H");
                log::debug!("--- Input report ---");
                log::debug!("{input_report}");
                log::debug!("{}", input_report.left_stick.get_x());
                log::debug!("---- End Report ----");
            }
            ReportType::McuInputReport => todo!(),
            ReportType::AttachmentInputReport => todo!(),
            ReportType::_Unused1 => todo!(),
            ReportType::GenericInputReport => todo!(),
            ReportType::OtaEnableFwuReport => todo!(),
            ReportType::OtaSetupReadReport => todo!(),
            ReportType::OtaReadReport => todo!(),
            ReportType::OtaWriteReport => todo!(),
            ReportType::OtaEraseReport => todo!(),
            ReportType::OtaLaunchReport => todo!(),
            ReportType::ExtGripOutputReport => todo!(),
            ReportType::ExtGripInputReport => todo!(),
            ReportType::_Unused2 => todo!(),
        }

        // Update the state
        //let old_state = self.update_state(input_report);

        // Translate the state into a stream of input events
        //let events = self.translate(old_state);

        Ok(vec![])
    }
}
