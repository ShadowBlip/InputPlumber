pub mod steam_deck;

use std::error::Error;

use hidapi::{DeviceInfo, HidApi};
use tokio::sync::broadcast;
use zbus::{fdo, Connection};
use zbus_macros::dbus_interface;

use crate::{constants::BUS_PREFIX, input::composite_device::Command};

/// DBusInterface exposing information about a HIDRaw device
pub struct DBusInterface {
    info: DeviceInfo,
}

impl DBusInterface {
    pub fn new(info: DeviceInfo) -> DBusInterface {
        DBusInterface { info }
    }

    /// Creates a new instance of the source hidraw interface on DBus. Returns
    /// a structure with information about the source device.
    pub async fn listen_on_dbus(conn: Connection, info: DeviceInfo) -> Result<(), Box<dyn Error>> {
        let path = get_dbus_path(info.path().to_string_lossy().to_string());
        let iface = DBusInterface::new(info);
        conn.object_server().at(path, iface).await?;
        Ok(())
    }
}
#[dbus_interface(name = "org.shadowblip.Input.Source.HIDRawDevice")]
impl DBusInterface {
    #[dbus_interface(property)]
    async fn path(&self) -> fdo::Result<String> {
        Ok(self.info.path().to_string_lossy().to_string())
    }

    #[dbus_interface(property)]
    async fn vendor_id(&self) -> fdo::Result<String> {
        Ok(format!("{:04x}", self.info.vendor_id()))
    }

    #[dbus_interface(property)]
    async fn product_id(&self) -> fdo::Result<String> {
        Ok(format!("{:04x}", self.info.product_id()))
    }

    #[dbus_interface(property)]
    async fn serial_number(&self) -> fdo::Result<String> {
        Ok(self.info.serial_number().unwrap_or_default().to_string())
    }

    #[dbus_interface(property)]
    async fn release_number(&self) -> fdo::Result<String> {
        Ok(format!("{:04x}", self.info.release_number()))
    }

    #[dbus_interface(property)]
    async fn manufacturer(&self) -> fdo::Result<String> {
        Ok(self
            .info
            .manufacturer_string()
            .unwrap_or_default()
            .to_string())
    }

    #[dbus_interface(property)]
    async fn product(&self) -> fdo::Result<String> {
        Ok(self.info.product_string().unwrap_or_default().to_string())
    }

    #[dbus_interface(property)]
    async fn interface_number(&self) -> fdo::Result<i32> {
        Ok(self.info.interface_number())
    }
}

/// Returns the DBus path for a [HIDRawDevice] from a device path (E.g. /dev/hidraw0)
pub fn get_dbus_path(device_path: String) -> String {
    let path = device_path.split('/').last().unwrap();
    format!("{}/devices/source/{}", BUS_PREFIX, path)
}

/// [HIDRawDevice] represents an input device using the input subsystem.
#[derive(Debug)]
pub struct HIDRawDevice {
    info: DeviceInfo,
    composite_tx: broadcast::Sender<Command>,
}

impl HIDRawDevice {
    pub fn new(info: DeviceInfo, composite_tx: broadcast::Sender<Command>) -> Self {
        Self { info, composite_tx }
    }

    /// Run the source device handler. HIDRaw devices require device-specific
    /// implementations. If one does not exist, an error will be returned.
    pub async fn run(&self) -> Result<(), Box<dyn Error>> {
        // Run the appropriate HIDRaw driver
        if self.info.vendor_id() == steam_deck::VID && self.info.product_id() == steam_deck::PID {
            log::info!("Detected Steam Deck");
            let tx = self.composite_tx.clone();
            let driver = steam_deck::DeckController::new(self.info.clone(), tx);
            driver.run().await?;
        }

        Ok(())
    }

    /// Returns a unique identifier for the source device.
    pub fn get_id(&self) -> String {
        //let name = format!(
        //    "{:04x}:{:04x}",
        //    self.info.vendor_id(),
        //    self.info.product_id()
        //);
        let device_path = self.info.path().to_string_lossy().to_string();
        let name = device_path.split('/').last().unwrap();
        format!("hidraw://{}", name)
    }
}

/// Returns an array of all HIDRaw devices
pub fn list_devices() -> Result<Vec<DeviceInfo>, Box<dyn Error>> {
    let api = HidApi::new()?;
    let devices: Vec<DeviceInfo> = api.device_list().cloned().collect();

    Ok(devices)
}
