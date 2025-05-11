pub mod accel_gyro_3d;
pub mod bmi_imu;

use std::error::Error;

use glob_match::glob_match;

use crate::{
    config, constants::BUS_SOURCES_PREFIX, input::composite_device::client::CompositeDeviceClient,
    udev::device::UdevDevice,
};

use self::{accel_gyro_3d::AccelGyro3dImu, bmi_imu::BmiImu};

use super::{SourceDeviceCompatible, SourceDriver};

/// List of available drivers
enum DriverType {
    Unknown,
    BmiImu,
    AccelGryo3D,
}

/// [IioDevice] represents an input device using the iio subsystem.
#[derive(Debug)]
pub enum IioDevice {
    BmiImu(SourceDriver<BmiImu>),
    AccelGryo3D(SourceDriver<AccelGyro3dImu>),
}

impl SourceDeviceCompatible for IioDevice {
    fn get_device_ref(&self) -> &UdevDevice {
        match self {
            IioDevice::BmiImu(source_driver) => source_driver.info_ref(),
            IioDevice::AccelGryo3D(source_driver) => source_driver.info_ref(),
        }
    }

    fn get_id(&self) -> String {
        match self {
            IioDevice::BmiImu(source_driver) => source_driver.get_id(),
            IioDevice::AccelGryo3D(source_driver) => source_driver.get_id(),
        }
    }

    fn client(&self) -> super::client::SourceDeviceClient {
        match self {
            IioDevice::BmiImu(source_driver) => source_driver.client(),
            IioDevice::AccelGryo3D(source_driver) => source_driver.client(),
        }
    }

    async fn run(self) -> Result<(), Box<dyn Error>> {
        match self {
            IioDevice::BmiImu(source_driver) => source_driver.run().await,
            IioDevice::AccelGryo3D(source_driver) => source_driver.run().await,
        }
    }

    fn get_capabilities(
        &self,
    ) -> Result<Vec<crate::input::capability::Capability>, super::InputError> {
        match self {
            IioDevice::BmiImu(source_driver) => source_driver.get_capabilities(),
            IioDevice::AccelGryo3D(source_driver) => source_driver.get_capabilities(),
        }
    }

    fn get_device_path(&self) -> String {
        match self {
            IioDevice::BmiImu(source_driver) => source_driver.get_device_path(),
            IioDevice::AccelGryo3D(source_driver) => source_driver.get_device_path(),
        }
    }
}

impl IioDevice {
    /// Create a new [IioDevice] associated with the given device and
    /// composite device. The appropriate driver will be selected based on
    /// the provided device.
    pub fn new(
        device_info: UdevDevice,
        composite_device: CompositeDeviceClient,
        conf: Option<config::SourceDevice>,
    ) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let driver_type = IioDevice::get_driver_type(&device_info);
        let iio_config = conf.as_ref().and_then(|c| c.iio.clone());

        match driver_type {
            DriverType::Unknown => Err("No driver for iio interface found".into()),
            DriverType::BmiImu => {
                let device = BmiImu::new(device_info.clone(), iio_config)?;
                let source_device = SourceDriver::new(composite_device, device, device_info, conf);
                Ok(Self::BmiImu(source_device))
            }
            DriverType::AccelGryo3D => {
                let device = AccelGyro3dImu::new(device_info.clone(), iio_config)?;
                let source_device = SourceDriver::new(composite_device, device, device_info, conf);
                Ok(Self::AccelGryo3D(source_device))
            }
        }
    }

    /// Return the driver type for the given device info
    fn get_driver_type(device: &UdevDevice) -> DriverType {
        let device_name = device.name();
        let name = device_name.as_str();
        log::debug!("Finding driver for IIO interface: {name}");
        // BMI_IMU
        if glob_match("{i2c-10EC5280*,i2c-BOSC*,i2c-BMI*,bmi*-imu,bmi260}", name) {
            log::info!("Detected BMI IMU");
            return DriverType::BmiImu;
        }

        // AccelGryo3D
        if glob_match("{gyro_3d,accel_3d}", name) {
            log::info!("Detected Legion Go");
            return DriverType::AccelGryo3D;
        }
        log::debug!("No driver found for IIO Interface: {name}");
        // Unknown
        DriverType::Unknown
    }
}

/// Returns the DBus path for an [IIODevice] from a device id (E.g. iio:device0)
pub fn get_dbus_path(id: String) -> String {
    let name = id.replace(':', "_");
    format!("{}/{}", BUS_SOURCES_PREFIX, name)
}
