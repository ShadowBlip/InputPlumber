pub mod dualsense;
pub mod fts3528;
pub mod lego;
pub mod opineo;
pub mod steam_deck;

use std::error::Error;

use tokio::sync::mpsc;

use crate::{
    constants::BUS_SOURCES_PREFIX,
    drivers,
    input::{capability::Capability, composite_device::client::CompositeDeviceClient},
    udev::device::UdevDevice,
};

use super::{client::SourceDeviceClient, SourceCommand};

/// Size of the [SourceCommand] buffer for receiving output events
const BUFFER_SIZE: usize = 2048;

/// List of available drivers
enum DriverType {
    Unknown,
    DualSense,
    SteamDeck,
    LegionGo,
    OrangePiNeo,
    Fts3528Touchscreen,
}

/// [HIDRawDevice] represents an input device using the input subsystem.
#[derive(Debug)]
pub struct HIDRawDevice {
    device: UdevDevice,
    composite_device: CompositeDeviceClient,
    tx: mpsc::Sender<SourceCommand>,
    rx: Option<mpsc::Receiver<SourceCommand>>,
}

impl HIDRawDevice {
    pub fn new(device: UdevDevice, composite_device: CompositeDeviceClient) -> Self {
        let (tx, rx) = mpsc::channel(BUFFER_SIZE);
        Self {
            device,
            composite_device,
            tx,
            rx: Some(rx),
        }
    }

    /// Returns a transmitter channel that can be used to send events to this device
    pub fn client(&self) -> SourceDeviceClient {
        self.tx.clone().into()
    }

    /// Run the source device handler. HIDRaw devices require device-specific
    /// implementations. If one does not exist, an error will be returned.
    pub async fn run(&mut self) -> Result<(), Box<dyn Error>> {
        // Run the appropriate HIDRaw driver
        match self.get_driver_type() {
            DriverType::Unknown => Err(format!(
                "No driver for hidraw interface found. VID: {}, PID: {}",
                self.device.id_vendor(),
                self.device.id_product()
            )
            .into()),
            DriverType::DualSense => {
                let composite_device = self.composite_device.clone();
                let rx = self.rx.take().unwrap();
                let mut driver = dualsense::DualSenseController::new(
                    self.device.clone(),
                    composite_device,
                    rx,
                    self.get_id(),
                );
                driver.run().await?;
                Ok(())
            }
            DriverType::SteamDeck => {
                let composite_device = self.composite_device.clone();
                let rx = self.rx.take().unwrap();
                let mut driver = steam_deck::DeckController::new(
                    self.device.clone(),
                    composite_device,
                    rx,
                    self.get_id(),
                );
                driver.run().await?;
                Ok(())
            }
            DriverType::LegionGo => {
                let composite_device = self.composite_device.clone();
                let driver = lego::LegionController::new(
                    self.device.clone(),
                    composite_device,
                    self.get_id(),
                );
                driver.run().await?;
                Ok(())
            }
            DriverType::OrangePiNeo => {
                let composite_device = self.composite_device.clone();
                let driver =
                    opineo::OrangePiNeoTouchpad::new(self.device.clone(), composite_device);
                driver.run().await?;
                Ok(())
            }
            DriverType::Fts3528Touchscreen => {
                let composite_device = self.composite_device.clone();
                let rx = self.rx.take().unwrap();
                let mut driver = fts3528::Fts3528TouchScreen::new(
                    self.device.clone(),
                    composite_device,
                    rx,
                    self.get_id(),
                );
                driver.run().await?;
                Ok(())
            }
        }
    }

    /// Returns a copy of the UdevDevice
    pub fn get_device(&self) -> UdevDevice {
        self.device.clone()
    }

    /// Returns a refrence to the UdevDevice
    pub fn get_device_ref(&self) -> &UdevDevice {
        &self.device
    }

    /// Returns a unique identifier for the source device.
    pub fn get_id(&self) -> String {
        format!("hidraw://{}", self.device.sysname())
    }

    /// Returns the full path to the device handler (e.g. /dev/hidraw0)
    pub fn get_device_path(&self) -> String {
        self.device.devnode()
    }

    /// Returns capabilities of this input device
    pub fn get_capabilities(&self) -> Result<Vec<Capability>, Box<dyn Error>> {
        match self.get_driver_type() {
            DriverType::Unknown => Err(format!(
                "No capabilities for interface found. VID: {}, PID: {}",
                self.device.id_vendor(),
                self.device.id_product()
            )
            .into()),
            DriverType::SteamDeck => Ok(Vec::from(steam_deck::CAPABILITIES)),
            DriverType::LegionGo => Ok(Vec::from(lego::CAPABILITIES)),
            DriverType::OrangePiNeo => Ok(Vec::from(opineo::CAPABILITIES)),
            DriverType::Fts3528Touchscreen => Ok(Vec::from(fts3528::CAPABILITIES)),
            DriverType::DualSense => Ok(Vec::from(dualsense::CAPABILITIES)),
        }
    }

    fn get_driver_type(&self) -> DriverType {
        log::debug!("Finding driver for interface: {:?}", self.device);
        let vid = self.device.id_vendor();
        let pid = self.device.id_product();

        // Sony DualSense
        if vid == dualsense::VID && dualsense::PIDS.contains(&pid) {
            log::info!("Detected Sony DualSense");
            return DriverType::DualSense;
        }

        // Steam Deck
        if vid == steam_deck::VID && pid == steam_deck::PID {
            log::info!("Detected Steam Deck");
            return DriverType::SteamDeck;
        }

        // Legion Go
        if vid == drivers::lego::driver::VID && drivers::lego::driver::PIDS.contains(&pid) {
            log::info!("Detected Legion Go");
            return DriverType::LegionGo;
        }

        // OrangePi NEO
        if vid == drivers::opineo::driver::VID && pid == drivers::opineo::driver::PID {
            log::info!("Detected OrangePi NEO");

            return DriverType::OrangePiNeo;
        }

        // FTS3528 Touchscreen
        if vid == drivers::fts3528::driver::VID && pid == drivers::fts3528::driver::PID {
            log::info!("Detected FTS3528 Touchscreen");
            return DriverType::Fts3528Touchscreen;
        }

        // Unknown
        DriverType::Unknown
    }
}

/// Returns the DBus path for a [HIDRawDevice] from a device path (E.g. /dev/hidraw0)
pub fn get_dbus_path(device_name: String) -> String {
    format!("{}/{}", BUS_SOURCES_PREFIX, device_name)
}
