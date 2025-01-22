pub mod dualsense;
pub mod fts3528;
pub mod horipad_steam;
pub mod lego_dinput_combined;
pub mod lego_dinput_split;
pub mod lego_fps_mode;
pub mod lego_xinput;
pub mod opineo;
pub mod rog_ally;
pub mod steam_deck;
pub mod xpad_uhid;

use std::{error::Error, time::Duration};

use horipad_steam::HoripadSteam;
use rog_ally::RogAlly;
use xpad_uhid::XpadUhid;

use crate::{
    constants::BUS_SOURCES_PREFIX,
    drivers,
    input::composite_device::client::CompositeDeviceClient,
    udev::device::UdevDevice,
};

use self::{
    dualsense::DualSenseController, fts3528::Fts3528Touchscreen,
    lego_dinput_combined::LegionControllerDCombined, lego_dinput_split::LegionControllerDSplit,
    lego_fps_mode::LegionControllerFPS, lego_xinput::LegionControllerX,
    opineo::OrangePiNeoTouchpad, steam_deck::DeckController,
};

use super::{SourceDriver, SourceDriverOptions};

/// List of available drivers
enum DriverType {
    Unknown,
    DualSense,
    Fts3528Touchscreen,
    HoripadSteam,
    LegionGoDCombined,
    LegionGoDSplit,
    LegionGoFPS,
    LegionGoX,
    OrangePiNeo,
    RogAlly,
    SteamDeck,
    XpadUhid,
}

/// [HidRawDevice] represents an input device using the hidraw subsystem.
#[derive(Debug)]
pub enum HidRawDevice {
    DualSense(SourceDriver<DualSenseController>),
    Fts3528Touchscreen(SourceDriver<Fts3528Touchscreen>),
    HoripadSteam(SourceDriver<HoripadSteam>),
    LegionGoDCombined(SourceDriver<LegionControllerDCombined>),
    LegionGoDSplit(SourceDriver<LegionControllerDSplit>),
    LegionGoFPS(SourceDriver<LegionControllerFPS>),
    LegionGoX(SourceDriver<LegionControllerX>),
    OrangePiNeo(SourceDriver<OrangePiNeoTouchpad>),
    RogAlly(SourceDriver<RogAlly>),
    SteamDeck(SourceDriver<DeckController>),
    XpadUhid(SourceDriver<XpadUhid>),
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
            DriverType::LegionGoDCombined => {
                let device = LegionControllerDCombined::new(device_info.clone())?;
                let source_device = SourceDriver::new(composite_device, device, device_info);
                Ok(Self::LegionGoDCombined(source_device))
            }
            DriverType::LegionGoDSplit => {
                let device = LegionControllerDSplit::new(device_info.clone())?;
                let source_device = SourceDriver::new(composite_device, device, device_info);
                Ok(Self::LegionGoDSplit(source_device))
            }
            DriverType::LegionGoFPS => {
                let device = LegionControllerFPS::new(device_info.clone())?;
                let source_device = SourceDriver::new(composite_device, device, device_info);
                Ok(Self::LegionGoFPS(source_device))
            }
            DriverType::LegionGoX => {
                let device = LegionControllerX::new(device_info.clone())?;
                let source_device = SourceDriver::new(composite_device, device, device_info);
                Ok(Self::LegionGoX(source_device))
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
            DriverType::XpadUhid => {
                let device = XpadUhid::new(device_info.clone())?;
                let source_device = SourceDriver::new(composite_device, device, device_info);
                Ok(Self::XpadUhid(source_device))
            }
            DriverType::RogAlly => {
                let device = RogAlly::new(device_info.clone())?;
                let options = SourceDriverOptions {
                    poll_rate: Duration::from_millis(500),
                    buffer_size: 1024,
                };
                let source_device =
                    SourceDriver::new_with_options(composite_device, device, device_info, options);
                Ok(Self::RogAlly(source_device))
            }
            DriverType::HoripadSteam => {
                let device = HoripadSteam::new(device_info.clone())?;
                let source_device = SourceDriver::new(composite_device, device, device_info);
                Ok(Self::HoripadSteam(source_device))
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

        // Legion Go Dinput Combined
        if vid == drivers::lego::driver_dinput_combined::VID
            && pid == drivers::lego::driver_dinput_combined::PID
        {
            log::info!("Detected Legion Go DInput Combined Mode");
            return DriverType::LegionGoDCombined;
        }

        // Legion Go Dinput Split
        if vid == drivers::lego::driver_dinput_split::VID
            && pid == drivers::lego::driver_dinput_split::PID
        {
            log::info!("Detected Legion Go DInput Split Mode");
            return DriverType::LegionGoDSplit;
        }

        // Legion Go FPS Mode
        if vid == drivers::lego::driver_fps_mode::VID && pid == drivers::lego::driver_fps_mode::PID
        {
            log::info!("Detected Legion Go FPS Mode");
            return DriverType::LegionGoFPS;
        }

        // Legion Go XInput
        if vid == drivers::lego::driver_xinput::VID && pid == drivers::lego::driver_xinput::PID {
            log::info!("Detected Legion Go XInput Mode");
            return DriverType::LegionGoX;
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

        // Rog Ally
        if vid == drivers::rog_ally::driver::VID && drivers::rog_ally::driver::PIDS.contains(&pid) {
            log::info!("Detected ROG Ally");
            return DriverType::RogAlly;
        }

        // XpadUhid
        let drivers = device.drivers();
        if drivers.contains(&"microsoft".to_string()) {
            let syspath = device.syspath();
            if syspath.contains("uhid") {
                log::info!("Detected UHID XPAD");
                return DriverType::XpadUhid;
            }
        }

        // Horipad Steam Controller
        if vid == drivers::horipad_steam::driver::VID
            && drivers::horipad_steam::driver::PIDS.contains(&pid)
        {
            log::info!("Detected Horipad Steam Controller");
            return DriverType::HoripadSteam;
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
