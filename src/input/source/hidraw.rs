pub mod blocked;
pub mod dualsense;
pub mod flydigi_vader_4_pro;
pub mod fts3528;
pub mod gpd_win_mini_touchpad;
pub mod gpd_win_mini_macro_keyboard;
pub mod horipad_steam;
pub mod legion_go;
pub mod legos_imu;
pub mod legos_touchpad;
pub mod legos_xinput;
pub mod msi_claw;
pub mod opineo;
pub mod rog_ally;
pub mod steam_deck;
pub mod xpad_uhid;
pub mod zotac_zone;

use std::{error::Error, time::Duration};

use blocked::BlockedHidrawDevice;
use flydigi_vader_4_pro::Vader4Pro;
use gpd_win_mini_touchpad::GpdWinMiniTouchpad;
use gpd_win_mini_macro_keyboard::GpdWinMiniMacroKeyboard;
use horipad_steam::HoripadSteam;
use legos_imu::LegionSImuController;
use legos_touchpad::LegionSTouchpadController;
use msi_claw::MsiClaw;
use rog_ally::RogAlly;
use xpad_uhid::XpadUhid;
use zotac_zone::ZotacZone;

use crate::{
    config,
    constants::BUS_SOURCES_PREFIX,
    drivers,
    input::{
        capability::Capability, composite_device::client::CompositeDeviceClient,
        info::DeviceInfoRef, output_capability::OutputCapability,
    },
    udev::device::UdevDevice,
};

use self::{
    dualsense::DualSenseController, fts3528::Fts3528Touchscreen, legion_go::LegionGoController,
    legos_xinput::LegionSXInputController, opineo::OrangePiNeoTouchpad, steam_deck::DeckController,
};

use super::{InputError, OutputError, SourceDeviceCompatible, SourceDriver, SourceDriverOptions};

/// List of available drivers
enum DriverType {
    Blocked,
    DualSense,
    Fts3528Touchscreen,
    GpdWinMiniTouchpad,
    GpdWinMiniMacroKeyboard,
    HoripadSteam,
    LegionGoSImu,
    LegionGoSTouchpad,
    LegionGoSXInput,
    LegionGoXInput,
    MsiClaw,
    OrangePiNeo,
    RogAlly,
    SteamDeck,
    Unknown,
    Vader4Pro,
    XpadUhid,
    ZotacZone,
}

/// [HidRawDevice] represents an input device using the hidraw subsystem.
#[derive(Debug)]
pub enum HidRawDevice {
    Blocked(SourceDriver<BlockedHidrawDevice>),
    DualSense(SourceDriver<DualSenseController>),
    Fts3528Touchscreen(SourceDriver<Fts3528Touchscreen>),
    GpdWinMiniTouchpad(SourceDriver<GpdWinMiniTouchpad>),
    GpdWinMiniMacroKeyboard(SourceDriver<GpdWinMiniMacroKeyboard>),
    HoripadSteam(SourceDriver<HoripadSteam>),
    LegionGoSImu(SourceDriver<LegionSImuController>),
    LegionGoSTouchpad(SourceDriver<LegionSTouchpadController>),
    LegionGoSXInput(SourceDriver<LegionSXInputController>),
    LegionGo(SourceDriver<LegionGoController>),
    MsiClaw(SourceDriver<MsiClaw>),
    OrangePiNeo(SourceDriver<OrangePiNeoTouchpad>),
    RogAlly(SourceDriver<RogAlly>),
    SteamDeck(SourceDriver<DeckController>),
    Vader4Pro(SourceDriver<Vader4Pro>),
    XpadUhid(SourceDriver<XpadUhid>),
    ZotacZone(SourceDriver<ZotacZone>),
}

impl SourceDeviceCompatible for HidRawDevice {
    fn get_device_ref(&self) -> DeviceInfoRef<'_> {
        match self {
            HidRawDevice::Blocked(source_driver) => source_driver.info_ref(),
            HidRawDevice::DualSense(source_driver) => source_driver.info_ref(),
            HidRawDevice::Fts3528Touchscreen(source_driver) => source_driver.info_ref(),
            HidRawDevice::GpdWinMiniTouchpad(source_driver) => source_driver.info_ref(),
            HidRawDevice::GpdWinMiniMacroKeyboard(source_driver) => source_driver.info_ref(),
            HidRawDevice::HoripadSteam(source_driver) => source_driver.info_ref(),
            HidRawDevice::LegionGoSImu(source_driver) => source_driver.info_ref(),
            HidRawDevice::LegionGoSTouchpad(source_driver) => source_driver.info_ref(),
            HidRawDevice::LegionGoSXInput(source_driver) => source_driver.info_ref(),
            HidRawDevice::LegionGo(source_driver) => source_driver.info_ref(),
            HidRawDevice::MsiClaw(source_driver) => source_driver.info_ref(),
            HidRawDevice::OrangePiNeo(source_driver) => source_driver.info_ref(),
            HidRawDevice::RogAlly(source_driver) => source_driver.info_ref(),
            HidRawDevice::SteamDeck(source_driver) => source_driver.info_ref(),
            HidRawDevice::Vader4Pro(source_driver) => source_driver.info_ref(),
            HidRawDevice::XpadUhid(source_driver) => source_driver.info_ref(),
            HidRawDevice::ZotacZone(source_driver) => source_driver.info_ref(),
        }
    }

    fn get_id(&self) -> String {
        match self {
            HidRawDevice::Blocked(source_driver) => source_driver.get_id(),
            HidRawDevice::DualSense(source_driver) => source_driver.get_id(),
            HidRawDevice::Fts3528Touchscreen(source_driver) => source_driver.get_id(),
            HidRawDevice::GpdWinMiniTouchpad(source_driver) => source_driver.get_id(),
            HidRawDevice::GpdWinMiniMacroKeyboard(source_driver) => source_driver.get_id(),
            HidRawDevice::HoripadSteam(source_driver) => source_driver.get_id(),
            HidRawDevice::LegionGoSImu(source_driver) => source_driver.get_id(),
            HidRawDevice::LegionGoSTouchpad(source_driver) => source_driver.get_id(),
            HidRawDevice::LegionGoSXInput(source_driver) => source_driver.get_id(),
            HidRawDevice::LegionGo(source_driver) => source_driver.get_id(),
            HidRawDevice::MsiClaw(source_driver) => source_driver.get_id(),
            HidRawDevice::OrangePiNeo(source_driver) => source_driver.get_id(),
            HidRawDevice::RogAlly(source_driver) => source_driver.get_id(),
            HidRawDevice::SteamDeck(source_driver) => source_driver.get_id(),
            HidRawDevice::Vader4Pro(source_driver) => source_driver.get_id(),
            HidRawDevice::XpadUhid(source_driver) => source_driver.get_id(),
            HidRawDevice::ZotacZone(source_driver) => source_driver.get_id(),
        }
    }

    fn client(&self) -> super::client::SourceDeviceClient {
        match self {
            HidRawDevice::Blocked(source_driver) => source_driver.client(),
            HidRawDevice::DualSense(source_driver) => source_driver.client(),
            HidRawDevice::Fts3528Touchscreen(source_driver) => source_driver.client(),
            HidRawDevice::GpdWinMiniTouchpad(source_driver) => source_driver.client(),
            HidRawDevice::GpdWinMiniMacroKeyboard(source_driver) => source_driver.client(),
            HidRawDevice::HoripadSteam(source_driver) => source_driver.client(),
            HidRawDevice::LegionGoSImu(source_driver) => source_driver.client(),
            HidRawDevice::LegionGoSTouchpad(source_driver) => source_driver.client(),
            HidRawDevice::LegionGoSXInput(source_driver) => source_driver.client(),
            HidRawDevice::LegionGo(source_driver) => source_driver.client(),
            HidRawDevice::MsiClaw(source_driver) => source_driver.client(),
            HidRawDevice::OrangePiNeo(source_driver) => source_driver.client(),
            HidRawDevice::RogAlly(source_driver) => source_driver.client(),
            HidRawDevice::SteamDeck(source_driver) => source_driver.client(),
            HidRawDevice::Vader4Pro(source_driver) => source_driver.client(),
            HidRawDevice::XpadUhid(source_driver) => source_driver.client(),
            HidRawDevice::ZotacZone(source_driver) => source_driver.client(),
        }
    }

    async fn run(self) -> Result<(), Box<dyn Error>> {
        match self {
            HidRawDevice::Blocked(source_driver) => source_driver.run().await,
            HidRawDevice::DualSense(source_driver) => source_driver.run().await,
            HidRawDevice::Fts3528Touchscreen(source_driver) => source_driver.run().await,
            HidRawDevice::GpdWinMiniTouchpad(source_driver) => source_driver.run().await,
            HidRawDevice::GpdWinMiniMacroKeyboard(source_driver) => source_driver.run().await,
            HidRawDevice::HoripadSteam(source_driver) => source_driver.run().await,
            HidRawDevice::LegionGoSImu(source_driver) => source_driver.run().await,
            HidRawDevice::LegionGoSTouchpad(source_driver) => source_driver.run().await,
            HidRawDevice::LegionGoSXInput(source_driver) => source_driver.run().await,
            HidRawDevice::LegionGo(source_driver) => source_driver.run().await,
            HidRawDevice::MsiClaw(source_driver) => source_driver.run().await,
            HidRawDevice::OrangePiNeo(source_driver) => source_driver.run().await,
            HidRawDevice::RogAlly(source_driver) => source_driver.run().await,
            HidRawDevice::SteamDeck(source_driver) => source_driver.run().await,
            HidRawDevice::Vader4Pro(source_driver) => source_driver.run().await,
            HidRawDevice::XpadUhid(source_driver) => source_driver.run().await,
            HidRawDevice::ZotacZone(source_driver) => source_driver.run().await,
        }
    }

    fn get_capabilities(&self) -> Result<Vec<Capability>, InputError> {
        match self {
            HidRawDevice::Blocked(source_driver) => source_driver.get_capabilities(),
            HidRawDevice::DualSense(source_driver) => source_driver.get_capabilities(),
            HidRawDevice::Fts3528Touchscreen(source_driver) => source_driver.get_capabilities(),
            HidRawDevice::GpdWinMiniTouchpad(source_driver) => source_driver.get_capabilities(),
            HidRawDevice::GpdWinMiniMacroKeyboard(source_driver) => source_driver.get_capabilities(),
            HidRawDevice::HoripadSteam(source_driver) => source_driver.get_capabilities(),
            HidRawDevice::LegionGoSImu(source_driver) => source_driver.get_capabilities(),
            HidRawDevice::LegionGoSTouchpad(source_driver) => source_driver.get_capabilities(),
            HidRawDevice::LegionGoSXInput(source_driver) => source_driver.get_capabilities(),
            HidRawDevice::LegionGo(source_driver) => source_driver.get_capabilities(),
            HidRawDevice::MsiClaw(source_driver) => source_driver.get_capabilities(),
            HidRawDevice::OrangePiNeo(source_driver) => source_driver.get_capabilities(),
            HidRawDevice::RogAlly(source_driver) => source_driver.get_capabilities(),
            HidRawDevice::SteamDeck(source_driver) => source_driver.get_capabilities(),
            HidRawDevice::Vader4Pro(source_driver) => source_driver.get_capabilities(),
            HidRawDevice::XpadUhid(source_driver) => source_driver.get_capabilities(),
            HidRawDevice::ZotacZone(source_driver) => source_driver.get_capabilities(),
        }
    }

    fn get_output_capabilities(&self) -> Result<Vec<OutputCapability>, OutputError> {
        match self {
            HidRawDevice::Blocked(source_driver) => source_driver.get_output_capabilities(),
            HidRawDevice::DualSense(source_driver) => source_driver.get_output_capabilities(),
            HidRawDevice::Fts3528Touchscreen(source_driver) => {
                source_driver.get_output_capabilities()
            }
            HidRawDevice::GpdWinMiniTouchpad(source_driver) => {
                source_driver.get_output_capabilities()
            }
            HidRawDevice::GpdWinMiniMacroKeyboard(source_driver) => {
                source_driver.get_output_capabilities()
            }
            HidRawDevice::HoripadSteam(source_driver) => source_driver.get_output_capabilities(),
            HidRawDevice::LegionGoSImu(source_driver) => source_driver.get_output_capabilities(),
            HidRawDevice::LegionGoSTouchpad(source_driver) => {
                source_driver.get_output_capabilities()
            }
            HidRawDevice::LegionGoSXInput(source_driver) => source_driver.get_output_capabilities(),
            HidRawDevice::LegionGo(source_driver) => source_driver.get_output_capabilities(),
            HidRawDevice::MsiClaw(source_driver) => source_driver.get_output_capabilities(),
            HidRawDevice::OrangePiNeo(source_driver) => source_driver.get_output_capabilities(),
            HidRawDevice::RogAlly(source_driver) => source_driver.get_output_capabilities(),
            HidRawDevice::SteamDeck(source_driver) => source_driver.get_output_capabilities(),
            HidRawDevice::Vader4Pro(source_driver) => source_driver.get_output_capabilities(),
            HidRawDevice::XpadUhid(source_driver) => source_driver.get_output_capabilities(),
            HidRawDevice::ZotacZone(source_driver) => source_driver.get_output_capabilities(),
        }
    }

    fn get_device_path(&self) -> String {
        match self {
            HidRawDevice::Blocked(source_driver) => source_driver.get_device_path(),
            HidRawDevice::DualSense(source_driver) => source_driver.get_device_path(),
            HidRawDevice::Fts3528Touchscreen(source_driver) => source_driver.get_device_path(),
            HidRawDevice::GpdWinMiniTouchpad(source_driver) => source_driver.get_device_path(),
            HidRawDevice::GpdWinMiniMacroKeyboard(source_driver) => source_driver.get_device_path(),
            HidRawDevice::HoripadSteam(source_driver) => source_driver.get_device_path(),
            HidRawDevice::LegionGoSImu(source_driver) => source_driver.get_device_path(),
            HidRawDevice::LegionGoSTouchpad(source_driver) => source_driver.get_device_path(),
            HidRawDevice::LegionGoSXInput(source_driver) => source_driver.get_device_path(),
            HidRawDevice::LegionGo(source_driver) => source_driver.get_device_path(),
            HidRawDevice::MsiClaw(source_driver) => source_driver.get_device_path(),
            HidRawDevice::OrangePiNeo(source_driver) => source_driver.get_device_path(),
            HidRawDevice::RogAlly(source_driver) => source_driver.get_device_path(),
            HidRawDevice::SteamDeck(source_driver) => source_driver.get_device_path(),
            HidRawDevice::Vader4Pro(source_driver) => source_driver.get_device_path(),
            HidRawDevice::XpadUhid(source_driver) => source_driver.get_device_path(),
            HidRawDevice::ZotacZone(source_driver) => source_driver.get_device_path(),
        }
    }
}

impl HidRawDevice {
    /// Create a new [HidRawDevice] associated with the given device and
    /// composite device. The appropriate driver will be selected based on
    /// the provided device.
    pub fn new(
        device_info: UdevDevice,
        composite_device: CompositeDeviceClient,
        conf: Option<config::SourceDevice>,
    ) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let is_blocked = conf.as_ref().and_then(|c| c.blocked).unwrap_or(false);
        let driver_type = HidRawDevice::get_driver_type(&device_info, is_blocked);

        match driver_type {
            DriverType::Unknown => Err("No driver for hidraw interface found".into()),
            DriverType::Blocked => {
                let options = SourceDriverOptions {
                    poll_rate: Duration::from_millis(200),
                    buffer_size: 4096,
                };
                let device = BlockedHidrawDevice::new(device_info.clone())?;
                let source_device = SourceDriver::new_with_options(
                    composite_device,
                    device,
                    device_info.into(),
                    options,
                    conf,
                );
                Ok(Self::Blocked(source_device))
            }
            DriverType::DualSense => {
                let options = SourceDriverOptions {
                    poll_rate: Duration::from_millis(1),
                    buffer_size: 2048,
                };
                let device = DualSenseController::new(device_info.clone())?;
                let source_device = SourceDriver::new_with_options(
                    composite_device,
                    device,
                    device_info.into(),
                    options,
                    conf,
                );
                Ok(Self::DualSense(source_device))
            }
            DriverType::SteamDeck => {
                let options = SourceDriverOptions {
                    poll_rate: Duration::from_millis(1),
                    buffer_size: 2048,
                };
                let device = DeckController::new(device_info.clone())?;
                let source_device = SourceDriver::new_with_options(
                    composite_device,
                    device,
                    device_info.into(),
                    options,
                    conf,
                );
                Ok(Self::SteamDeck(source_device))
            }
            DriverType::LegionGoXInput => {
                let device = LegionGoController::new(device_info.clone())?;
                let source_device =
                    SourceDriver::new(composite_device, device, device_info.into(), conf);
                Ok(Self::LegionGo(source_device))
            }
            DriverType::LegionGoSImu => {
                let options = SourceDriverOptions {
                    poll_rate: Duration::from_millis(4),
                    buffer_size: 2048,
                };
                let device = LegionSImuController::new(device_info.clone())?;
                let source_device = SourceDriver::new_with_options(
                    composite_device,
                    device,
                    device_info.into(),
                    options,
                    conf,
                );
                Ok(Self::LegionGoSImu(source_device))
            }
            DriverType::LegionGoSTouchpad => {
                let options = SourceDriverOptions {
                    poll_rate: Duration::from_millis(8),
                    buffer_size: 2048,
                };
                let device = LegionSTouchpadController::new(device_info.clone())?;
                let source_device = SourceDriver::new_with_options(
                    composite_device,
                    device,
                    device_info.into(),
                    options,
                    conf,
                );
                Ok(Self::LegionGoSTouchpad(source_device))
            }
            DriverType::LegionGoSXInput => {
                let options = SourceDriverOptions {
                    poll_rate: Duration::from_millis(4),
                    buffer_size: 2048,
                };
                let device = LegionSXInputController::new(device_info.clone())?;
                let source_device = SourceDriver::new_with_options(
                    composite_device,
                    device,
                    device_info.into(),
                    options,
                    conf,
                );
                Ok(Self::LegionGoSXInput(source_device))
            }
            DriverType::OrangePiNeo => {
                let device = OrangePiNeoTouchpad::new(device_info.clone())?;
                let source_device =
                    SourceDriver::new(composite_device, device, device_info.into(), conf);
                Ok(Self::OrangePiNeo(source_device))
            }
            DriverType::MsiClaw => {
                let device = MsiClaw::new(device_info.clone())?;
                let source_device =
                    SourceDriver::new(composite_device, device, device_info.into(), conf);
                Ok(Self::MsiClaw(source_device))
            }
            DriverType::Fts3528Touchscreen => {
                let device = Fts3528Touchscreen::new(device_info.clone())?;
                let source_device =
                    SourceDriver::new(composite_device, device, device_info.into(), conf);
                Ok(Self::Fts3528Touchscreen(source_device))
            }
            DriverType::XpadUhid => {
                let device = XpadUhid::new(device_info.clone())?;
                let source_device =
                    SourceDriver::new(composite_device, device, device_info.into(), conf);
                Ok(Self::XpadUhid(source_device))
            }
            DriverType::RogAlly => {
                let device = RogAlly::new(device_info.clone())?;
                let options = SourceDriverOptions {
                    poll_rate: Duration::from_millis(500),
                    buffer_size: 1024,
                };
                let source_device = SourceDriver::new_with_options(
                    composite_device,
                    device,
                    device_info.into(),
                    options,
                    conf,
                );
                Ok(Self::RogAlly(source_device))
            }
            DriverType::HoripadSteam => {
                let device = HoripadSteam::new(device_info.clone())?;
                let source_device =
                    SourceDriver::new(composite_device, device, device_info.into(), conf);
                Ok(Self::HoripadSteam(source_device))
            }
            DriverType::Vader4Pro => {
                let device = Vader4Pro::new(device_info.clone())?;
                let options = SourceDriverOptions {
                    poll_rate: Duration::from_millis(0),
                    buffer_size: 1024,
                };
                let source_device = SourceDriver::new_with_options(
                    composite_device,
                    device,
                    device_info.into(),
                    options,
                    conf,
                );
                Ok(Self::Vader4Pro(source_device))
            }
            DriverType::ZotacZone => {
                let device = ZotacZone::new(device_info.clone())?;
                let options = SourceDriverOptions {
                    poll_rate: Duration::from_millis(300),
                    buffer_size: 1024,
                };
                let source_device = SourceDriver::new_with_options(
                    composite_device,
                    device,
                    device_info.into(),
                    options,
                    conf,
                );
                Ok(Self::ZotacZone(source_device))
            }
            DriverType::GpdWinMiniTouchpad => {
                let device = GpdWinMiniTouchpad::new(device_info.clone())?;
                let source_device =
                    SourceDriver::new(composite_device, device, device_info.into(), conf);
                Ok(Self::GpdWinMiniTouchpad(source_device))
            }
            DriverType::GpdWinMiniMacroKeyboard => {
                let device = GpdWinMiniMacroKeyboard::new(device_info.clone())?;
                let source_device =
                    SourceDriver::new(composite_device, device, device_info.into(), conf);
                Ok(Self::GpdWinMiniMacroKeyboard(source_device))
            }
        }
    }

    /// Return the driver type for the given vendor and product
    fn get_driver_type(device: &UdevDevice, is_blocked: bool) -> DriverType {
        log::debug!("Finding driver for interface: {:?}", device);
        if is_blocked {
            return DriverType::Blocked;
        }
        let vid = device.id_vendor();
        let pid = device.id_product();
        let iid = device.interface_number();

        // Sony DualSense
        if vid == dualsense::VID && dualsense::PIDS.contains(&pid) {
            log::info!("Detected Sony DualSense");
            return DriverType::DualSense;
        }

        // Steam Deck
        if vid == drivers::steam_deck::VID
            && pid == drivers::steam_deck::ProductId::SteamDeck.to_u16()
        {
            log::info!("Detected Steam Deck");
            return DriverType::SteamDeck;
        }

        // Legion Go XInput
        if vid == drivers::lego::VID
            && drivers::lego::PIDS.contains(&pid)
            && iid == drivers::lego::GP_IID
        {
            log::info!("Detected Legion Go Controller");
            return DriverType::LegionGoXInput;
        }

        // Legion Go S IMU
        if vid == drivers::legos::VID
            && drivers::legos::PIDS.contains(&pid)
            && iid == drivers::legos::IMU_IID
        {
            log::info!("Detected Legion Go S IMU");
            return DriverType::LegionGoSImu;
        }

        // Legion Go S Touchpad
        if vid == drivers::legos::VID
            && drivers::legos::PIDS.contains(&pid)
            && iid == drivers::legos::TP_IID
        {
            log::info!("Detected Legion Go S Touchpad");
            return DriverType::LegionGoSTouchpad;
        }

        // Legion Go S XInput
        if vid == drivers::legos::VID
            && drivers::legos::PIDS.contains(&pid)
            && iid == drivers::legos::GP_IID
        {
            log::info!("Detected Legion Go S Controller");
            return DriverType::LegionGoSXInput;
        }

        // OrangePi NEO
        if vid == drivers::opineo::driver::VID && pid == drivers::opineo::driver::PID {
            log::info!("Detected OrangePi NEO");

            return DriverType::OrangePiNeo;
        }

        // MSI Claw
        if vid == drivers::msi_claw::driver::VID && pid == drivers::msi_claw::driver::PID {
            log::info!("Detected MSI Claw");

            return DriverType::MsiClaw;
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

        // Flydigi Vader 4 Pro
        if vid == drivers::flydigi_vader_4_pro::driver::VID
            && pid == drivers::flydigi_vader_4_pro::driver::PID
        {
            log::info!("Detected Flydigi Vader 4 Pro");
            return DriverType::Vader4Pro;
        }

        // Zotac Zone
        if vid == drivers::zotac_zone::driver::VID
            && drivers::zotac_zone::driver::PIDS.contains(&pid)
        {
            log::info!("Detected ZOTAC Zone");
            return DriverType::ZotacZone;
        }

        // GPD Win Mini
        if vid == drivers::gpd_win_mini::touchpad_driver::VID
            && pid == drivers::gpd_win_mini::touchpad_driver::PID
            && iid == drivers::gpd_win_mini::touchpad_driver::IID
        {
            log::info!("Detected GPD Win Mini Touchpad");
            return DriverType::GpdWinMiniTouchpad;
        }

        if vid == drivers::gpd_win_mini::macro_keyboard_driver::VID
            && pid == drivers::gpd_win_mini::macro_keyboard_driver::PID
            && iid == drivers::gpd_win_mini::macro_keyboard_driver::IID
        {
            log::info!("Detected GPD Win Mini Macro keyboard");
            return DriverType::GpdWinMiniMacroKeyboard;
        }

        // Unknown
        log::warn!("No driver for hidraw interface found. VID: {vid}, PID: {pid}");
        DriverType::Unknown
    }
}

/// Returns the DBus path for a [HIDRawDevice] from a device path (E.g. /dev/hidraw0)
pub fn get_dbus_path(device_name: String) -> String {
    format!("{BUS_SOURCES_PREFIX}/{device_name}")
}
