pub mod fts3528;
pub mod lego;
pub mod opineo;
pub mod steam_deck;

use std::error::Error;

use hidapi::{DeviceInfo, HidApi};
use tokio::sync::mpsc;

use crate::{
    constants::BUS_PREFIX,
    drivers::{self},
    input::{capability::Capability, composite_device::command::Command},
};

use super::SourceCommand;

/// Size of the [SourceCommand] buffer for receiving output events
const BUFFER_SIZE: usize = 2048;

/// List of available drivers
enum DriverType {
    Unknown,
    SteamDeck,
    LegionGo,
    OrangePiNeo,
    Fts3528Touchscreen,
}

/// [HIDRawDevice] represents an input device using the input subsystem.
#[derive(Debug)]
pub struct HIDRawDevice {
    info: DeviceInfo,
    composite_tx: mpsc::Sender<Command>,
    tx: mpsc::Sender<SourceCommand>,
    rx: Option<mpsc::Receiver<SourceCommand>>,
}

impl HIDRawDevice {
    pub fn new(info: DeviceInfo, composite_tx: mpsc::Sender<Command>) -> Self {
        let (tx, rx) = mpsc::channel(BUFFER_SIZE);
        log::debug!("HIDRaw DeviceInfo: {info:?}");
        Self {
            info,
            composite_tx,
            tx,
            rx: Some(rx),
        }
    }

    /// Returns a transmitter channel that can be used to send events to this device
    pub fn transmitter(&self) -> mpsc::Sender<SourceCommand> {
        self.tx.clone()
    }

    /// Run the source device handler. HIDRaw devices require device-specific
    /// implementations. If one does not exist, an error will be returned.
    pub async fn run(&mut self) -> Result<(), Box<dyn Error>> {
        // Run the appropriate HIDRaw driver
        match self.get_driver_type() {
            DriverType::Unknown => Err(format!(
                "No driver for hidraw interface found. VID: {}, PID: {}",
                self.info.vendor_id(),
                self.info.product_id()
            )
            .into()),

            DriverType::SteamDeck => {
                let tx = self.composite_tx.clone();
                let rx = self.rx.take().unwrap();
                let mut driver =
                    steam_deck::DeckController::new(self.info.clone(), tx, rx, self.get_id());
                driver.run().await?;
                Ok(())
            }
            DriverType::LegionGo => {
                let tx = self.composite_tx.clone();
                let driver = lego::LegionController::new(self.info.clone(), tx, self.get_id());
                driver.run().await?;
                Ok(())
            }
            DriverType::OrangePiNeo => {
                let tx = self.composite_tx.clone();
                let driver = opineo::OrangePiNeoTouchpad::new(self.info.clone(), tx, self.get_id());
                driver.run().await?;
                Ok(())
            }
            DriverType::Fts3528Touchscreen => {
                let tx = self.composite_tx.clone();
                let rx = self.rx.take().unwrap();
                let mut driver =
                    fts3528::Fts3528TouchScreen::new(self.info.clone(), tx, rx, self.get_id());
                driver.run().await?;
                Ok(())
            }
        }
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

    /// Returns the full path to the device handler (e.g. /dev/hidraw0)
    pub fn get_device_path(&self) -> String {
        self.info.path().to_string_lossy().to_string()
    }

    /// Returns capabilities of this input device
    pub fn get_capabilities(&self) -> Result<Vec<Capability>, Box<dyn Error>> {
        match self.get_driver_type() {
            DriverType::Unknown => Err(format!(
                "No capabilities for interface found. VID: {}, PID: {}",
                self.info.vendor_id(),
                self.info.product_id()
            )
            .into()),
            DriverType::SteamDeck => Ok(Vec::from(steam_deck::CAPABILITIES)),
            DriverType::LegionGo => Ok(Vec::from(lego::CAPABILITIES)),
            DriverType::OrangePiNeo => Ok(Vec::from(opineo::CAPABILITIES)),
            DriverType::Fts3528Touchscreen => Ok(Vec::from(fts3528::CAPABILITIES)),
        }
    }

    fn get_driver_type(&self) -> DriverType {
        log::debug!("Finding driver for interface: {:?}", self.info);
        // Steam Deck
        if self.info.vendor_id() == steam_deck::VID && self.info.product_id() == steam_deck::PID {
            log::info!("Detected Steam Deck");
            return DriverType::SteamDeck;
        }

        // Legion Go
        if self.info.vendor_id() == drivers::lego::driver::VID
            && (self.info.product_id() == drivers::lego::driver::PID1
                || self.info.product_id() == drivers::lego::driver::PID2
                || self.info.product_id() == drivers::lego::driver::PID3)
        {
            log::info!("Detected Legion Go");
            return DriverType::LegionGo;
        }

        // OrangePi NEO
        if self.info.vendor_id() == drivers::opineo::driver::VID
            && self.info.product_id() == drivers::opineo::driver::PID
        {
            log::info!("Detected OrangePi NEO");
            return DriverType::OrangePiNeo;
        }

        // FTS3528 Touchscreen
        if self.info.vendor_id() == drivers::fts3528::driver::VID
            && self.info.product_id() == drivers::fts3528::driver::PID
        {
            log::info!("Detected FTS3528 Touchscreen");
            return DriverType::Fts3528Touchscreen;
        }

        // Unknown
        DriverType::Unknown
    }
}

/// Returns an array of all HIDRaw devices
pub fn list_devices() -> Result<Vec<DeviceInfo>, Box<dyn Error>> {
    let api = HidApi::new()?;
    let devices: Vec<DeviceInfo> = api.device_list().cloned().collect();

    Ok(devices)
}

/// Returns the DBus path for a [HIDRawDevice] from a device path (E.g. /dev/hidraw0)
pub fn get_dbus_path(device_path: String) -> String {
    let path = device_path.split('/').last().unwrap();
    format!("{}/devices/source/{}", BUS_PREFIX, path)
}
