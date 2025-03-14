use std::error::Error;

use websockets::WebsocketDevice;

use crate::{
    config,
    input::{composite_device::client::CompositeDeviceClient, info::DeviceInfoRef},
    network::websocket::WebsocketClient,
};

use super::{SourceDeviceCompatible, SourceDriver};

pub mod websockets;

#[derive(Debug)]
pub enum NetworkDevice {
    Websocket(SourceDriver<WebsocketDevice>),
}

impl NetworkDevice {
    pub fn new(
        device_info: WebsocketClient,
        composite_device: CompositeDeviceClient,
        conf: Option<config::SourceDevice>,
    ) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let device = WebsocketDevice::new(device_info.clone())?;
        let source_device = SourceDriver::new(composite_device, device, device_info.into(), conf);
        Ok(Self::Websocket(source_device))
    }
}

impl SourceDeviceCompatible for NetworkDevice {
    fn get_device_ref(&self) -> DeviceInfoRef {
        match self {
            Self::Websocket(source_driver) => source_driver.info_ref(),
        }
    }

    fn get_id(&self) -> String {
        match self {
            Self::Websocket(source_driver) => source_driver.get_id(),
        }
    }

    fn client(&self) -> super::client::SourceDeviceClient {
        match self {
            Self::Websocket(source_driver) => source_driver.client(),
        }
    }

    async fn run(self) -> Result<(), Box<dyn Error>> {
        match self {
            Self::Websocket(source_driver) => source_driver.run().await,
        }
    }

    fn get_capabilities(
        &self,
    ) -> Result<Vec<crate::input::capability::Capability>, super::InputError> {
        match self {
            Self::Websocket(source_driver) => source_driver.get_capabilities(),
        }
    }

    fn get_device_path(&self) -> String {
        match self {
            Self::Websocket(source_driver) => source_driver.get_device_path(),
        }
    }
}
