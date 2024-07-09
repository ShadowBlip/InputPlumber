pub mod accel_gyro_3d;
pub mod bmi_imu;

use std::error::Error;

use glob_match::glob_match;
use tokio::sync::mpsc;

use crate::{
    config,
    constants::BUS_PREFIX,
    input::{capability::Capability, composite_device::client::CompositeDeviceClient},
    udev::device::UdevDevice,
};

use super::{client::SourceDeviceClient, SourceCommand};

/// Size of the [SourceCommand] buffer for receiving output events
const BUFFER_SIZE: usize = 2048;

/// List of available drivers
enum DriverType {
    Unknown,
    BmiImu,
    AccelGryo3D,
}

/// Returns the DBus path for an [IIODevice] from a device id (E.g. iio:device0)
pub fn get_dbus_path(id: String) -> String {
    let name = id.replace(':', "_");
    format!("{}/devices/source/{}", BUS_PREFIX, name)
}

#[derive(Debug)]
pub struct IIODevice {
    device: UdevDevice,
    config: Option<config::IIO>,
    composite_device: CompositeDeviceClient,
    tx: mpsc::Sender<SourceCommand>,
    rx: Option<mpsc::Receiver<SourceCommand>>,
}

impl IIODevice {
    pub fn new(
        device: UdevDevice,
        config: Option<config::IIO>,
        composite_device: CompositeDeviceClient,
    ) -> Self {
        let (tx, rx) = mpsc::channel(BUFFER_SIZE);
        Self {
            device,
            config,
            composite_device,
            tx,
            rx: Some(rx),
        }
    }

    /// Returns a transmitter channel that can be used to send events to this device
    pub fn client(&self) -> SourceDeviceClient {
        self.tx.clone().into()
    }

    pub fn get_capabilities(&self) -> Result<Vec<Capability>, Box<dyn Error>> {
        Ok(vec![])
    }

    /// Returns a copy of the UdevDevice
    pub fn get_device(&self) -> UdevDevice {
        self.device.clone()
    }

    /// Returns a copy of the UdevDevice
    pub fn get_device_ref(&self) -> &UdevDevice {
        &self.device
    }

    /// Returns a unique identifier for the source device.
    pub fn get_id(&self) -> String {
        format!("iio://{}", self.device.sysname())
    }

    /// Returns the full path to the device handler (e.g. /sys/bus/iio/devices/iio:device0)
    pub fn get_device_path(&self) -> String {
        self.device.devnode()
    }

    /// Run the source IIO device
    pub async fn run(&mut self) -> Result<(), Box<dyn Error>> {
        // Run the appropriate IIO driver
        let name = self.device.name();

        match self.get_driver_type(&name) {
            DriverType::Unknown => Err(format!("No driver for IMU found. {}", name).into()),
            DriverType::BmiImu => {
                log::info!("Detected BMI_IMU: {name}");
                let composite_device = self.composite_device.clone();
                let rx = self.rx.take().unwrap();
                let mut driver = bmi_imu::IMU::new(
                    self.device.clone(),
                    self.config.clone(),
                    composite_device,
                    rx,
                    self.get_id(),
                );
                driver.run().await?;
                Ok(())
            }

            DriverType::AccelGryo3D => {
                log::info!("Detected IMU: {name}");
                let composite_device = self.composite_device.clone();
                let rx = self.rx.take().unwrap();
                let mut driver = accel_gyro_3d::IMU::new(
                    self.device.clone(),
                    self.config.clone(),
                    composite_device,
                    rx,
                    self.get_id(),
                );
                driver.run().await?;
                Ok(())
            }
        }
    }

    fn get_driver_type(&self, name: &str) -> DriverType {
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
