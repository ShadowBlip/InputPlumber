pub mod accel_gyro_3d;
pub mod accel_gyro_3d_new;
pub mod bmi_imu;
pub mod bmi_imu_new;

use std::error::Error;

use glob_match::glob_match;

use crate::{
    config, constants::BUS_SOURCES_PREFIX, input::composite_device::client::CompositeDeviceClient,
    udev::device::UdevDevice,
};

use self::{accel_gyro_3d_new::AccelGyro3dImu, bmi_imu_new::BmiImu};

use super::SourceDriver;

/// Size of the [SourceCommand] buffer for receiving output events
const BUFFER_SIZE: usize = 2048;

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

impl IioDevice {
    /// Create a new [IioDevice] associated with the given device and
    /// composite device. The appropriate driver will be selected based on
    /// the provided device.
    pub fn new(
        device_info: UdevDevice,
        composite_device: CompositeDeviceClient,
        config: Option<config::IIO>,
    ) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let driver_type = IioDevice::get_driver_type(&device_info);

        match driver_type {
            DriverType::Unknown => Err("No driver for iio interface found".into()),
            DriverType::BmiImu => {
                let device = BmiImu::new(device_info.clone(), config)?;
                let source_device = SourceDriver::new(composite_device, device, device_info);
                Ok(Self::BmiImu(source_device))
            }
            DriverType::AccelGryo3D => {
                let device = AccelGyro3dImu::new(device_info.clone(), config)?;
                let source_device = SourceDriver::new(composite_device, device, device_info);
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
        if glob_match("{i2c-10EC5280*,i2c-BMI*,bmi*-imu}", name) {
            log::info!("Detected Steam Deck");
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
