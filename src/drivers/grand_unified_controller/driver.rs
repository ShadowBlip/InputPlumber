use packed_struct::prelude::*;
use std::{error::Error, ffi::CString};

use hidapi::HidDevice;

use super::{
    event::Event,
    hid_report::{
        InputCapabilityReport, InputDataReport, ReportType, INPUT_CAPABILITY_REPORT_SIZE,
        INPUT_DATA_REPORT_SIZE, UNIFIED_SPEC_VERSION_MAJOR, UNIFIED_SPEC_VERSION_MINOR,
    },
};

// TODO: Find an appropriate VID/PID to use. For the time being, this
// is using the OpenInput VID/PID. We may be able to ask for a VID/PID
// from OpenMoko: https://github.com/openmoko/openmoko-usb-oui/tree/master
pub const UNIFIED_CONTROLLER_VID: u16 = 0x1d50;
pub const UNIFIED_CONTROLLER_PID: u16 = 0x616a;

/// Unified Controller driver for reading gamepad input
pub struct Driver {
    device: HidDevice,
    capabilities: Option<InputCapabilityReport>,
    state: Option<InputDataReport>,
}

impl Driver {
    pub fn new(path: String) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let c_path = CString::new(path.clone())?;
        let api = hidapi::HidApi::new()?;
        let device = api.open_path(&c_path)?;
        let info = device.get_device_info()?;
        let vid = info.vendor_id();
        let pid = info.product_id();
        if vid != UNIFIED_CONTROLLER_VID || pid != UNIFIED_CONTROLLER_PID {
            return Err(format!("Device '{path}' is not a Unified Controller: {vid}:{pid}").into());
        }

        Ok(Self {
            device,
            capabilities: None,
            state: None,
        })
    }

    /// Poll the device and read input reports
    pub fn poll(&mut self) -> Result<Vec<Event>, Box<dyn Error + Send + Sync>> {
        // Check the device for its capabilities
        if self.capabilities.is_none() {
            let mut buf = [0; INPUT_CAPABILITY_REPORT_SIZE];
            buf[0] = ReportType::InputCapabilityReport as u8;
            let bytes_read = self.device.get_feature_report(&mut buf)?;
            println!("GOT FEATURE REPORT: {bytes_read}");

            let buffer = &buf.try_into()?;
            let report = InputCapabilityReport::unpack(buffer)?;

            println!("Got capabilities: {report}");
            self.capabilities = Some(report);

            //
        }

        // Read data from the device into a buffer
        let mut buf = [0; INPUT_DATA_REPORT_SIZE];
        let bytes_read = self.device.read(&mut buf[..])?;
        let slice = &buf[..bytes_read];

        // Handle the incoming input report
        let events = self.handle_report(slice, bytes_read)?;

        Ok(events)
    }

    fn handle_report(
        &mut self,
        buf: &[u8],
        bytes_read: usize,
    ) -> Result<Vec<Event>, Box<dyn Error + Send + Sync>> {
        // The first and second bytes contain the specification version
        let major_ver = buf[0];
        let minor_ver = buf[1];

        // The major version indicates there are breaking changes.
        if major_ver != UNIFIED_SPEC_VERSION_MAJOR {
            return Err(format!("Device major version (v{major_ver}) is not compatible with this implementation (v{UNIFIED_SPEC_VERSION_MAJOR})").into());
        }
        // Minor versions are backwards compatible
        if minor_ver > UNIFIED_SPEC_VERSION_MINOR {
            return Err(format!("Device minor version (v{minor_ver}) is newer than this implementation supports (v{UNIFIED_SPEC_VERSION_MINOR})").into());
        }

        let report_type = ReportType::from(buf[2]);
        match report_type {
            ReportType::Unknown => (),
            ReportType::InputCapabilityReport => (),
            ReportType::InputDataReport => {
                let buffer = buf.try_into()?;
                let report = InputDataReport::unpack(buffer)?;
                //println!("Got report: {report}");
            }
            ReportType::OutputCapabilityReport => (),
            ReportType::OutputDataReport => (),
        }

        Ok(vec![])
    }
}
