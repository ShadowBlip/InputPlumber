use std::error::Error;

use device::CecRawDevice;

use crate::{
    config,
    input::{capability::Capability, composite_device::client::CompositeDeviceClient},
    udev::device::UdevDevice,
};

use super::{client::SourceDeviceClient, SourceDeviceCompatible, SourceDriver};

pub mod device;

/// [CecDevice] represents an input device using CEC
#[derive(Debug)]
pub enum CecDevice {
    Device(SourceDriver<CecRawDevice>),
}

impl SourceDeviceCompatible for CecDevice {
    fn get_device_ref(&self) -> &UdevDevice {
        match self {
            Self::Device(source_driver) => source_driver.info_ref(),
        }
    }

    fn get_id(&self) -> String {
        match self {
            Self::Device(source_driver) => source_driver.get_id(),
        }
    }

    fn client(&self) -> SourceDeviceClient {
        match self {
            Self::Device(source_driver) => source_driver.client(),
        }
    }

    async fn run(self) -> Result<(), Box<dyn Error>> {
        match self {
            Self::Device(source_driver) => source_driver.run().await,
        }
    }

    fn get_capabilities(&self) -> Result<Vec<Capability>, super::InputError> {
        match self {
            Self::Device(source_driver) => source_driver.get_capabilities(),
        }
    }

    fn get_device_path(&self) -> String {
        match self {
            Self::Device(source_driver) => source_driver.get_device_path(),
        }
    }
}

impl CecDevice {
    pub fn new(
        device_info: UdevDevice,
        composite_device: CompositeDeviceClient,
        conf: Option<config::SourceDevice>,
    ) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let device = CecRawDevice::new();
        let source_device = SourceDriver::new(composite_device, device, device_info, conf);
        Ok(Self::Device(source_device))
    }
}
