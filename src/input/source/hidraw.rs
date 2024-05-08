pub mod lego;
pub mod steam_deck;

use std::error::Error;

use hidapi::{DeviceInfo, HidApi};
use tokio::sync::mpsc;

use crate::{
    constants::BUS_PREFIX,
    drivers::{self},
    input::{capability::Capability, composite_device::Command},
};

use super::SourceCommand;

/// Size of the [SourceCommand] buffer for receiving output events
const BUFFER_SIZE: usize = 2048;

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
        if self.info.vendor_id() == steam_deck::VID && self.info.product_id() == steam_deck::PID {
            log::info!("Detected Steam Deck");
            let tx = self.composite_tx.clone();
            let rx = self.rx.take().unwrap();
            let mut driver =
                steam_deck::DeckController::new(self.info.clone(), tx, rx, self.get_id());
            driver.run().await?;
        } else if self.info.vendor_id() == drivers::lego::driver::VID
            && (self.info.product_id() == drivers::lego::driver::PID
                || self.info.product_id() == drivers::lego::driver::PID2)
        {
            log::info!("Detected Legion Go");
            let tx = self.composite_tx.clone();
            let driver = lego::LegionController::new(self.info.clone(), tx, self.get_id());
            driver.run().await?;
        } else {
            return Err(format!(
                "No driver for hidraw interface found. VID: {}, PID: {}",
                self.info.vendor_id(),
                self.info.product_id()
            )
            .into());
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

    /// Returns the full path to the device handler (e.g. /dev/hidraw0)
    pub fn get_device_path(&self) -> String {
        self.info.path().to_string_lossy().to_string()
    }

    /// Returns capabilities of this input device
    pub fn get_capabilities(&self) -> Result<Vec<Capability>, Box<dyn Error>> {
        if self.info.vendor_id() == steam_deck::VID && self.info.product_id() == steam_deck::PID {
            Ok(Vec::from(steam_deck::CAPABILITIES))
        } else if self.info.vendor_id() == drivers::lego::driver::VID
            && (self.info.product_id() == drivers::lego::driver::PID
                || self.info.product_id() == drivers::lego::driver::PID2)
        {
            Ok(Vec::from(lego::CAPABILITIES))
        } else {
            Err(format!(
                "No driver for hidraw interface found. VID: {}, PID: {}",
                self.info.vendor_id(),
                self.info.product_id()
            )
            .into())
        }
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
