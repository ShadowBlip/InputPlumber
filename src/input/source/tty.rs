use std::{error::Error, time::Duration};

pub mod oxflyserial;

use super::{InputError, OutputError, SourceDeviceCompatible, SourceDriver};
use crate::{
    config,
    constants::BUS_SOURCES_PREFIX,
    input::{
        capability::Capability,
        composite_device::client::CompositeDeviceClient,
        info::DeviceInfoRef,
        output_capability::OutputCapability,
        source::{tty::oxflyserial::OneXFlySerial, SourceDriverOptions},
    },
    udev::device::UdevDevice,
};
/// List of available drivers
enum DriverType {
    OneXFlySerial,
}
/// [TtyDevice] represents an input device using the tty subsystem.
#[derive(Debug)]
pub enum TtyDevice {
    OneXFlySerial(SourceDriver<OneXFlySerial>),
}

impl SourceDeviceCompatible for TtyDevice {
    fn get_device_ref(&self) -> DeviceInfoRef<'_> {
        match self {
            TtyDevice::OneXFlySerial(source_driver) => source_driver.info_ref(),
        }
    }

    fn get_id(&self) -> String {
        match self {
            TtyDevice::OneXFlySerial(source_driver) => source_driver.get_id(),
        }
    }

    fn client(&self) -> super::client::SourceDeviceClient {
        match self {
            TtyDevice::OneXFlySerial(source_driver) => source_driver.client(),
        }
    }

    async fn run(self) -> Result<(), Box<dyn Error>> {
        match self {
            TtyDevice::OneXFlySerial(source_driver) => source_driver.run().await,
        }
    }

    fn get_capabilities(&self) -> Result<Vec<Capability>, InputError> {
        match self {
            TtyDevice::OneXFlySerial(source_driver) => source_driver.get_capabilities(),
        }
    }

    fn get_output_capabilities(&self) -> Result<Vec<OutputCapability>, OutputError> {
        Ok(vec![])
    }

    fn get_device_path(&self) -> String {
        match self {
            TtyDevice::OneXFlySerial(source_driver) => source_driver.get_device_path(),
        }
    }
}

impl TtyDevice {
    /// Create a new [TtyDevice] associated with the given device and
    /// composite device. The appropriate driver will be selected based on
    /// the provided device.
    pub fn new(
        device_info: UdevDevice,
        composite_device: CompositeDeviceClient,
        conf: Option<config::SourceDevice>,
    ) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let driver_type = TtyDevice::get_driver_type(&device_info);
        match driver_type {
            DriverType::OneXFlySerial => {
                let options = SourceDriverOptions {
                    poll_rate: Duration::from_millis(1),
                    buffer_size: 4096,
                };
                let device = OneXFlySerial::new(device_info.clone())?;
                let source_device = SourceDriver::new_with_options(
                    composite_device,
                    device,
                    device_info.into(),
                    options,
                    conf,
                );
                Ok(Self::OneXFlySerial(source_device))
            }
        }
    }

    /// Return the driver type for the given device info
    fn get_driver_type(device: &UdevDevice) -> DriverType {
        let device_name = device.sysname();
        let name = device_name.as_str();
        log::debug!("Finding driver for tty interface: {name}");
        //TODO: Actual detection
        DriverType::OneXFlySerial
    }
}
/// Returns the DBus path for an [TtyDevice] from a device id (E.g. tty://ttyS0)
pub fn get_dbus_path(id: String) -> String {
    let name = id.replace(':', "_").replace("-", "_");
    format!("{}/{}", BUS_SOURCES_PREFIX, name)
}
