pub mod dualsense;
pub mod fts3528;
pub mod lego;
pub mod opineo;
pub mod steam_deck;

use std::{error::Error, time::Duration};

use crate::{
    constants::BUS_SOURCES_PREFIX, drivers, input::composite_device::client::CompositeDeviceClient,
    udev::device::UdevDevice,
};

use self::{
    dualsense::DualSenseController, fts3528::Fts3528Touchscreen, lego::LegionController,
    opineo::OrangePiNeoTouchpad, steam_deck::DeckController,
};

use super::{SourceDriver, SourceDriverOptions};

/// List of available drivers
enum DriverType {
    Unknown,
    DualSense,
    SteamDeck,
    LegionGo,
    OrangePiNeo,
    Fts3528Touchscreen,
}

/// [HidRawDevice] represents an input device using the hidraw subsystem.
#[derive(Debug)]
pub enum HidRawDevice {
    DualSense(SourceDriver<DualSenseController>),
    SteamDeck(SourceDriver<DeckController>),
    LegionGo(SourceDriver<LegionController>),
    OrangePiNeo(SourceDriver<OrangePiNeoTouchpad>),
    Fts3528Touchscreen(SourceDriver<Fts3528Touchscreen>),
}

impl HidRawDevice {
    /// Create a new [HidRawDevice] associated with the given device and
    /// composite device. The appropriate driver will be selected based on
    /// the provided device.
    pub fn new(
        device_info: UdevDevice,
        composite_device: CompositeDeviceClient,
    ) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let driver_type = HidRawDevice::get_driver_type(&device_info);

        match driver_type {
            DriverType::Unknown => Err("No driver for hidraw interface found".into()),
            DriverType::DualSense => {
                let options = SourceDriverOptions {
                    poll_rate: Duration::from_millis(1),
                    buffer_size: 2048,
                };
                let device = DualSenseController::new(device_info.clone())?;
                let source_device =
                    SourceDriver::new_with_options(composite_device, device, device_info, options);
                Ok(Self::DualSense(source_device))
            }
            DriverType::SteamDeck => {
                let options = SourceDriverOptions {
                    poll_rate: Duration::from_millis(1),
                    buffer_size: 2048,
                };
                let device = DeckController::new(device_info.clone())?;
                let source_device =
                    SourceDriver::new_with_options(composite_device, device, device_info, options);
                Ok(Self::SteamDeck(source_device))
            }
            DriverType::LegionGo => {
                let device = LegionController::new(device_info.clone())?;
                let source_device = SourceDriver::new(composite_device, device, device_info);
                Ok(Self::LegionGo(source_device))
            }
            DriverType::OrangePiNeo => {
                let device = OrangePiNeoTouchpad::new(device_info.clone())?;
                let source_device = SourceDriver::new(composite_device, device, device_info);
                Ok(Self::OrangePiNeo(source_device))
            }
            DriverType::Fts3528Touchscreen => {
                let device = Fts3528Touchscreen::new(device_info.clone())?;
                let source_device = SourceDriver::new(composite_device, device, device_info);
                Ok(Self::Fts3528Touchscreen(source_device))
            }
        }
    }

    /// Return the driver type for the given vendor and product
    fn get_driver_type(device: &UdevDevice) -> DriverType {
        log::debug!("Finding driver for interface: {:?}", device);
        let vid = device.id_vendor();
        let pid = device.id_product();

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
        log::warn!("No driver for hidraw interface found. VID: {vid}, PID: {pid}");
        DriverType::Unknown
    }
}

/// Returns the DBus path for a [HIDRawDevice] from a device path (E.g. /dev/hidraw0)
pub fn get_dbus_path(device_name: String) -> String {
    format!("{}/{}", BUS_SOURCES_PREFIX, device_name)
}
