use std::{error::Error, ffi::CString, fmt::Debug};

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

const HID_TIMEOUT: i32 = 10;
const READ_BUF_SIZE: usize = 64;

/// Generic hidraw button source device driven by a capability map.
pub struct GenericHidrawButtons {
    device: HidDevice,
    translator: HidrawEventTranslator,
}

impl GenericHidrawButtons {
    pub fn new(
        device_info: UdevDevice,
        capability_map: CapabilityMapConfigV2,
    ) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let path = device_info.devnode();
        let cs_path = CString::new(path.clone())?;
        let api = hidapi::HidApi::new()?;
        let device = api.open_path(&cs_path)?;
        device.set_blocking_mode(false)?;

        let translator = HidrawEventTranslator::new(&capability_map);
        if !translator.has_hid_translation() {
            return Err(format!(
                "Capability map '{}' has no hidraw button mappings",
                capability_map.name
            )
            .into());
        }

        log::info!(
            "Opened generic hidraw button device at {path} with {} mapping(s)",
            translator.capabilities().len(),
        );

        Ok(Self { device, translator })
    }
}

impl SourceInputDevice for GenericHidrawButtons {
    fn poll(&mut self) -> Result<Vec<NativeEvent>, InputError> {
        let mut buf = [0u8; READ_BUF_SIZE];
        let bytes_read = self
            .device
            .read_timeout(&mut buf[..], HID_TIMEOUT)
            .map_err(|e| InputError::DeviceError(e.to_string()))?;

        if bytes_read == 0 {
            return Ok(vec![]);
        }

        let events = self.translator.translate(&buf[..bytes_read]);
        Ok(events)
    }

    fn get_capabilities(&self) -> Result<Vec<Capability>, InputError> {
        Ok(self.translator.capabilities())
    }
}

impl SourceOutputDevice for GenericHidrawButtons {}

impl Debug for GenericHidrawButtons {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GenericHidrawButtons").finish()
    }
}
