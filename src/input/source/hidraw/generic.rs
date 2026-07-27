use std::{error::Error, ffi::CString};

use hidapi::HidDevice;

use crate::{
    config::capability_map::CapabilityMapConfigV2,
    input::{
        capability::Capability,
        event::{hidraw::translator::HidrawEventTranslator, native::NativeEvent},
        source::{InputError, SourceInputDevice, SourceOutputDevice},
    },
    udev::device::UdevDevice,
};

const READ_BUFFER_SIZE: usize = 256;

#[derive(Debug)]
pub struct GenericDevice {
    device: HidDevice,
    translator: HidrawEventTranslator,
    capabilities: Vec<Capability>,
}

impl GenericDevice {
    pub fn new(
        device_info: UdevDevice,
        capability_map: &CapabilityMapConfigV2,
    ) -> Result<Self, Box<dyn Error + Send + Sync>> {
        // Open a handle to the hidraw device
        let path = device_info.devnode();
        let c_path = CString::new(path)?;
        let api = hidapi::HidApi::new()?;
        let device = api.open_path(&c_path)?;

        // Generate the capabilities based on the capability map
        let mut capabilities = vec![];
        for mapping in capability_map.mapping.iter() {
            let capability = mapping.target_event.clone().into();
            capabilities.push(capability);
        }

        // Create a translator instance which will translate hidraw input reports
        // into inputplumber events
        let translator = HidrawEventTranslator::new(capability_map);

        Ok(Self {
            device,
            translator,
            capabilities,
        })
    }
}

impl SourceInputDevice for GenericDevice {
    fn poll(&mut self) -> Result<Vec<NativeEvent>, InputError> {
        let mut buf = [0u8; READ_BUFFER_SIZE];
        let bytes_read = self
            .device
            .read(&mut buf[..])
            .map_err(|e| InputError::DeviceError(e.to_string()))?;

        if bytes_read == 0 {
            return Ok(vec![]);
        }

        let events = self.translator.translate(&buf[..bytes_read]);
        Ok(events)
    }

    fn get_capabilities(&self) -> Result<Vec<Capability>, InputError> {
        Ok(self.capabilities.clone())
    }
}

impl SourceOutputDevice for GenericDevice {}
