pub mod bmi_imu;

use std::error::Error;

use glob_match::glob_match;
use tokio::sync::broadcast;
use zbus::{fdo, Connection};
use zbus_macros::dbus_interface;

use crate::{
    constants::BUS_PREFIX,
    iio::device::Device,
    input::{capability::Capability, composite_device::Command},
};

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
    composite_tx: broadcast::Sender<Command>,
}

impl IIODevice {
    pub fn new(info: Device, composite_tx: broadcast::Sender<Command>) -> Self {
        Self { info, composite_tx }
    }

    pub fn get_capabilities(&self) -> Result<Vec<Capability>, Box<dyn Error>> {
        Ok(vec![])
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
    pub async fn run(&self) -> Result<(), Box<dyn Error>> {
        // Run the appropriate IIO driver
        let Some(name) = self.info.name.clone() else {
            return Err("Unable to determine IIO driver because no name was found".into());
        };

        // BMI Driver
        if glob_match("{i2c-BMI*,display_gyro}", name.as_str()) {
            log::info!("Detected BMI IMU: {name}");
            let tx = self.composite_tx.clone();
            let driver = bmi_imu::IMU::new(self.info.clone(), tx, self.get_id());
            driver.run().await?;
        } else {
            return Err(format!("Unsupported IIO device: {name}").into());
        }
        Ok(())
    }
}
