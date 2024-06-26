pub mod accel_gyro_3d;
pub mod bmi_imu;

use std::error::Error;

use glob_match::glob_match;
use tokio::sync::mpsc;
use zbus::{fdo, Connection};
use zbus_macros::dbus_interface;

use crate::{
    config,
    constants::BUS_PREFIX,
    iio::device::Device,
    input::{capability::Capability, composite_device::client::CompositeDeviceClient},
};

use super::SourceCommand;

/// Size of the [SourceCommand] buffer for receiving output events
const BUFFER_SIZE: usize = 2048;

/// List of available drivers
enum DriverType {
    Unknown,
    BmiImu,
    AccelGryo3D,
}
/// DBusInterface exposing information about a IIO device
pub struct DBusInterface {
    info: Device,
}

impl DBusInterface {
    pub fn new(info: Device) -> DBusInterface {
        DBusInterface { info }
    }

    /// Creates a new instance of the source hidraw interface on DBus. Returns
    /// a structure with information about the source device.
    pub async fn listen_on_dbus(conn: Connection, info: Device) -> Result<(), Box<dyn Error>> {
        let Some(id) = info.id.clone() else {
            return Err("Failed to get ID of IIO device".into());
        };
        let path = get_dbus_path(id);
        let iface = DBusInterface::new(info);
        conn.object_server().at(path, iface).await?;
        Ok(())
    }
}
#[dbus_interface(name = "org.shadowblip.Input.Source.IIODevice")]
impl DBusInterface {
    #[dbus_interface(property)]
    async fn id(&self) -> fdo::Result<String> {
        Ok(self.info.id.clone().unwrap_or_default())
    }

    #[dbus_interface(property)]
    async fn name(&self) -> fdo::Result<String> {
        Ok(self.info.name.clone().unwrap_or_default())
    }
}

/// Returns the DBus path for an [IIODevice] from a device id (E.g. iio:device0)
pub fn get_dbus_path(id: String) -> String {
    let name = id.replace(':', "_");
    format!("{}/devices/source/{}", BUS_PREFIX, name)
}

#[derive(Debug)]
pub struct IIODevice {
    info: Device,
    config: Option<config::IIO>,
    composite_device: CompositeDeviceClient,
    tx: mpsc::Sender<SourceCommand>,
    rx: Option<mpsc::Receiver<SourceCommand>>,
}

impl IIODevice {
    pub fn new(
        info: Device,
        config: Option<config::IIO>,
        composite_device: CompositeDeviceClient,
    ) -> Self {
        let (tx, rx) = mpsc::channel(BUFFER_SIZE);
        Self {
            info,
            config,
            composite_device,
            tx,
            rx: Some(rx),
        }
    }

    /// Returns a transmitter channel that can be used to send events to this device
    pub fn transmitter(&self) -> mpsc::Sender<SourceCommand> {
        self.tx.clone()
    }

    pub fn get_capabilities(&self) -> Result<Vec<Capability>, Box<dyn Error>> {
        Ok(vec![])
    }

    pub fn get_info(&self) -> Device {
        self.info.clone()
    }

    /// Returns a unique identifier for the source device.
    pub fn get_id(&self) -> String {
        let name = self.info.id.clone().unwrap_or_default();
        format!("iio://{}", name)
    }

    /// Returns the full path to the device handler (e.g. /sys/bus/iio/devices/iio:device0)
    pub fn get_device_path(&self) -> String {
        let Some(id) = self.info.id.clone() else {
            return "".to_string();
        };
        format!("/sys/bus/iio/devices/{id}")
    }

    /// Run the source IIO device
    pub async fn run(&mut self) -> Result<(), Box<dyn Error>> {
        // Run the appropriate IIO driver
        let Some(name) = self.info.name.clone() else {
            return Err("Unable to determine IIO driver because no name was found".into());
        };

        match self.get_driver_type(&name) {
            DriverType::Unknown => Err(format!("No driver for IMU found. {}", name).into()),
            DriverType::BmiImu => {
                log::info!("Detected BMI_IMU");
                let composite_device = self.composite_device.clone();
                let rx = self.rx.take().unwrap();
                let mut driver = bmi_imu::IMU::new(
                    self.info.clone(),
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
                    self.info.clone(),
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
        log::debug!("Finding driver for interface: {:?}", self.info);
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

        // Unknown
        DriverType::Unknown
    }
}
