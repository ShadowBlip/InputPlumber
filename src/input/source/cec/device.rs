use cec_rs::{CecConnectionCfgBuilder, CecDeviceType, CecDeviceTypeVec};

use crate::input::{
    capability::Capability,
    source::{InputError, SourceInputDevice, SourceOutputDevice},
};

#[derive(Debug)]
pub struct CecRawDevice {}

impl CecRawDevice {
    pub fn new() -> Self {
        let cfg = CecConnectionCfgBuilder::default()
            .device_types(CecDeviceTypeVec::new(CecDeviceType::AudioSystem))
            .build();
        Self {}
    }
}

impl Default for CecRawDevice {
    fn default() -> Self {
        Self::new()
    }
}

impl SourceInputDevice for CecRawDevice {
    fn poll(&mut self) -> Result<Vec<crate::input::event::native::NativeEvent>, InputError> {
        todo!()
    }

    fn get_capabilities(&self) -> Result<Vec<Capability>, InputError> {
        todo!()
    }
}

impl SourceOutputDevice for CecRawDevice {}
