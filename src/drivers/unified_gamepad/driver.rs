use packed_struct::prelude::*;
use std::{error::Error, ffi::CString};

use hidapi::HidDevice;

use crate::drivers::unified_gamepad::reports::{
    input_capability_report::INPUT_CAPABILITY_REPORT_SIZE, ReportType,
};

use super::{
    event::Event,
    reports::{
        input_capability_report::InputCapabilityReport,
        input_data_report::{InputDataReport, INPUT_DATA_REPORT_SIZE},
        UNIFIED_SPEC_VERSION_MAJOR, UNIFIED_SPEC_VERSION_MINOR,
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
    #[allow(dead_code)]
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
    #[allow(dead_code)]
    pub fn poll(&mut self) -> Result<Vec<Event>, Box<dyn Error + Send + Sync>> {
        // Check the device for its capabilities
        if self.capabilities.is_none() {
            let mut buf = [0; INPUT_CAPABILITY_REPORT_SIZE];
            buf[0] = ReportType::InputCapabilityReport as u8;
            let _bytes_read = self.device.get_feature_report(&mut buf)?;
            let report = InputCapabilityReport::unpack(&buf)?;
            self.capabilities = Some(report);
        }

        // Read data from the device into a buffer
        let mut buf = [0; INPUT_DATA_REPORT_SIZE];
        let bytes_read = self.device.read(&mut buf[..])?;
        let slice = &buf[..bytes_read];

        // Handle the incoming input report
        let events = self.handle_report(slice, bytes_read)?;

        Ok(events)
    }

    #[allow(dead_code)]
    fn handle_report(
        &mut self,
        buf: &[u8],
        _bytes_read: usize,
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
            ReportType::InputCapabilityReport => {
                let report = InputCapabilityReport::unpack(buf)?;
                self.capabilities = Some(report);
                // If the capabilities change, zero out the old state
                self.state = None;
            }
            ReportType::InputDataReport => {
                return self.handle_input_report(buf);
            }
            ReportType::OutputCapabilityReport => (),
            ReportType::OutputDataReport => (),
        }

        Ok(vec![])
    }

    fn handle_input_report(
        &mut self,
        buf: &[u8],
    ) -> Result<Vec<Event>, Box<dyn Error + Send + Sync>> {
        let buffer = buf.try_into()?;
        let report = InputDataReport::unpack(buffer)?;
        let old_state = self.state.take();
        self.state = Some(report);

        let Some(old_state) = old_state.as_ref() else {
            return Ok(vec![]);
        };
        let Some(state) = self.state.as_ref() else {
            return Ok(vec![]);
        };
        if state.state_version == old_state.state_version {
            return Ok(vec![]);
        }
        let Some(capabilities) = self.capabilities.as_ref() else {
            return Ok(vec![]);
        };

        let old_values = capabilities.decode_data_report(old_state)?;
        let values = capabilities.decode_data_report(state)?;
        let values_iter = old_values.iter().zip(values.iter());

        let mut events = Vec::new();
        for (info, (old_value, value)) in capabilities.get_capabilities().iter().zip(values_iter) {
            if old_value == value {
                continue;
            }
            let capability = info.capability;
            let event = Event {
                capability,
                value: value.to_owned(),
            };
            events.push(event);
        }

        Ok(events)
    }
}
