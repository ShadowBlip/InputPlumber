pub mod multicolor;
use self::multicolor::LedMultiColor;
use super::{SourceDeviceCompatible, SourceDriver};
use crate::{
    config,
    constants::BUS_SOURCES_PREFIX,
    input::{composite_device::client::CompositeDeviceClient, info::DeviceInfoRef},
    udev::device::UdevDevice,
};
use std::error::Error;
/// List of available drivers
enum DriverType {
    LedMultiColor,
}
/// [LedDevice] represents an input device using the leds subsystem.
#[derive(Debug)]
pub enum LedDevice {
    MultiColor(SourceDriver<LedMultiColor>),
}

impl SourceDeviceCompatible for LedDevice {
    fn get_device_ref(&self) -> DeviceInfoRef {
        match self {
            LedDevice::MultiColor(source_driver) => source_driver.info_ref(),
        }
    }

    fn get_id(&self) -> String {
        match self {
            LedDevice::MultiColor(source_driver) => source_driver.get_id(),
        }
    }

    fn client(&self) -> super::client::SourceDeviceClient {
        match self {
            LedDevice::MultiColor(source_driver) => source_driver.client(),
        }
    }

    async fn run(self) -> Result<(), Box<dyn Error>> {
        match self {
            LedDevice::MultiColor(source_driver) => source_driver.run().await,
        }
    }

    fn get_capabilities(
        &self,
    ) -> Result<Vec<crate::input::capability::Capability>, super::InputError> {
        match self {
            LedDevice::MultiColor(source_driver) => source_driver.get_capabilities(),
        }
    }

    fn get_device_path(&self) -> String {
        match self {
            LedDevice::MultiColor(source_driver) => source_driver.get_device_path(),
        }
    }
}

impl LedDevice {
    /// Create a new [LedDevice] associated with the given device and
    /// composite device. The appropriate driver will be selected based on
    /// the provided device.
    pub fn new(
        device_info: UdevDevice,
        composite_device: CompositeDeviceClient,
        conf: Option<config::SourceDevice>,
    ) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let driver_type = LedDevice::get_driver_type(&device_info);
        match driver_type {
            DriverType::LedMultiColor => {
                let device = LedMultiColor::new(device_info.clone())?;
                let source_device =
                    SourceDriver::new(composite_device, device, device_info.into(), conf);
                Ok(Self::MultiColor(source_device))
            }
        }
    }

    /// Return the driver type for the given device info
    fn get_driver_type(device: &UdevDevice) -> DriverType {
        let device_name = device.sysname();
        let name = device_name.as_str();
        log::debug!("Finding driver for LED interface: {name}");

        DriverType::LedMultiColor
    }
}
/// Returns the DBus path for an [LedDevice] from a device id (E.g. leds://input7__numlock)
pub fn get_dbus_path(id: String) -> String {
    let name = id.replace(':', "_");
    format!("{}/{}", BUS_SOURCES_PREFIX, name)
}
