pub mod ssc_imu;

use std::error::Error;

use crate::{
    config,
    constants::BUS_SOURCES_PREFIX,
    input::{
        capability::Capability, composite_device::client::CompositeDeviceClient,
        info::DeviceInfoRef, output_capability::OutputCapability, source::fastrpc::ssc_imu::SscImu,
    },
    udev::device::UdevDevice,
};

use super::{InputError, OutputError, SourceDeviceCompatible, SourceDriver};

enum DriverType {
    // Unknown,
    SscImu,
}

/// [FastRpcDevice] represents a device (likely the Sensor Core) using the Qualcomm FastRPC subsystem.
#[derive(Debug)]
pub enum FastRpcDevice {
    SscImu(SourceDriver<SscImu>),
}

impl SourceDeviceCompatible for FastRpcDevice {
    fn get_device_ref(&self) -> DeviceInfoRef<'_> {
        match self {
            FastRpcDevice::SscImu(source_driver) => source_driver.info_ref(),
        }
    }

    fn get_id(&self) -> String {
        match self {
            FastRpcDevice::SscImu(source_driver) => source_driver.get_id(),
        }
    }

    fn client(&self) -> super::client::SourceDeviceClient {
        match self {
            FastRpcDevice::SscImu(source_driver) => source_driver.client(),
        }
    }

    async fn run(self) -> Result<(), Box<dyn Error>> {
        match self {
            FastRpcDevice::SscImu(source_driver) => source_driver.run().await,
        }
    }

    fn get_capabilities(&self) -> Result<Vec<Capability>, InputError> {
        match self {
            FastRpcDevice::SscImu(source_driver) => source_driver.get_capabilities(),
        }
    }

    fn get_output_capabilities(&self) -> Result<Vec<OutputCapability>, OutputError> {
        Ok(vec![])
    }

    fn get_device_path(&self) -> String {
        match self {
            FastRpcDevice::SscImu(source_driver) => source_driver.get_device_path(),
        }
    }
}

impl FastRpcDevice {
    /// Create a new [FastRpcDevice] associated with the given device and
    /// composite device. The appropriate driver will be selected based on
    /// the provided device.
    pub fn new(
        device_info: UdevDevice,
        composite_device: CompositeDeviceClient,
        conf: Option<config::SourceDevice>,
    ) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let driver_type = FastRpcDevice::get_driver_type(&device_info);

        let imu_config = conf
            .as_ref()
            .and_then(|c| c.config.clone())
            .and_then(|c| c.imu);

        match driver_type {
            // DriverType::Unknown => Err("No driver for FastRPC interface found".into()),
            DriverType::SscImu => {
                let device = SscImu::new(device_info.clone(), imu_config)?;
                let source_device =
                    SourceDriver::new(composite_device, device, device_info.into(), conf);
                Ok(Self::SscImu(source_device))
            }
        }
    }

    /// Return the driver type for the given device info
    fn get_driver_type(device: &UdevDevice) -> DriverType {
        let device_name = device.name();
        let name = device_name.as_str();
        log::debug!("Finding driver for FastRPC interface: {name}");

        // TODO(gio)
        DriverType::SscImu
    }
}

/// Returns the DBus path for an [FastRpcDevice] from a device id (E.g. fastrpc:device0)
pub fn get_dbus_path(id: String) -> String {
    let name = id.replace([':', '-', '.'], "_");
    format!("{}/{}", BUS_SOURCES_PREFIX, name)
}
