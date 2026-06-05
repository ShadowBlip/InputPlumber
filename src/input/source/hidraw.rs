pub mod blocked;
pub mod dualsense;
pub mod flydigi_vader_4_pro;
pub mod fts3528;
pub mod gpd_win_mini_macro_keyboard;
pub mod gpd_win_mini_touchpad;
pub mod horipad_steam;
pub mod legion_go;
pub mod legion_go2;
pub mod legos_imu;
pub mod legos_touchpad;
pub mod legos_xinput;
pub mod opineo;
pub mod oxp_hid;
pub mod rog_ally;
pub mod steam_deck;
pub mod ultimate_2;
pub mod xpad_uhid;
pub mod zotac_zone;

use std::{error::Error, time::Duration};

use crate::{
    config,
    constants::BUS_SOURCES_PREFIX,
    drivers,
    input::{
        capability::Capability, composite_device::client::CompositeDeviceClient,
        info::DeviceInfoRef, output_capability::OutputCapability,
        source::hidraw::ultimate_2::Ultimate2,
    },
    udev::device::UdevDevice,
};

use self::{
    blocked::BlockedHidrawDevice, dualsense::DualSenseController, flydigi_vader_4_pro::Vader4Pro,
    fts3528::Fts3528Touchscreen, gpd_win_mini_macro_keyboard::GpdWinMiniMacroKeyboard,
    gpd_win_mini_touchpad::GpdWinMiniTouchpad, horipad_steam::HoripadSteam,
    legion_go::LegionGoController, legion_go2::LegionGo2Controller,
    legos_imu::LegionSImuController, legos_touchpad::LegionSTouchpadController,
    legos_xinput::LegionSXInputController, opineo::OrangePiNeoTouchpad, oxp_hid::OxpHid,
    rog_ally::RogAlly, steam_deck::DeckController, xpad_uhid::XpadUhid, zotac_zone::ZotacZone,
};
use super::{InputError, OutputError, SourceDeviceCompatible, SourceDriver, SourceDriverOptions};

/// List of available drivers
enum DriverType {
    Blocked,
    DualSense,
    Fts3528Touchscreen,
    GpdWinMiniMacroKeyboard,
    GpdWinMiniTouchpad,
    HoripadSteam,
    LegionGo,
    LegionGo2,
    LegionGoSImu,
    LegionGoSTouchpad,
    LegionGoSXInput,
    OrangePiNeo,
    OxpHid,
    RogAlly,
    SteamDeck,
    Unknown,
    Vader4Pro,
    Ultimate2,
    XpadUhid,
    ZotacZone,
}

/// [HidRawDevice] represents an input device using the hidraw subsystem.
#[derive(Debug)]
pub enum HidRawDevice {
    Blocked(SourceDriver<BlockedHidrawDevice>),
    DualSense(SourceDriver<DualSenseController>),
    Fts3528Touchscreen(SourceDriver<Fts3528Touchscreen>),
    GpdWinMiniMacroKeyboard(SourceDriver<GpdWinMiniMacroKeyboard>),
    GpdWinMiniTouchpad(SourceDriver<GpdWinMiniTouchpad>),
    HoripadSteam(SourceDriver<HoripadSteam>),
    LegionGo(SourceDriver<LegionGoController>),
    LegionGo2(SourceDriver<LegionGo2Controller>),
    LegionGoSImu(SourceDriver<LegionSImuController>),
    LegionGoSTouchpad(SourceDriver<LegionSTouchpadController>),
    LegionGoSXInput(SourceDriver<LegionSXInputController>),
    OrangePiNeo(SourceDriver<OrangePiNeoTouchpad>),
    OxpHid(SourceDriver<OxpHid>),
    RogAlly(SourceDriver<RogAlly>),
    SteamDeck(SourceDriver<DeckController>),
    Vader4Pro(SourceDriver<Vader4Pro>),
    Ultimate2(SourceDriver<Ultimate2>),
    XpadUhid(SourceDriver<XpadUhid>),
    ZotacZone(SourceDriver<ZotacZone>),
}

impl SourceDeviceCompatible for HidRawDevice {
    fn get_device_ref(&self) -> DeviceInfoRef<'_> {
        match self {
            HidRawDevice::Blocked(source_driver) => source_driver.info_ref(),
            HidRawDevice::DualSense(source_driver) => source_driver.info_ref(),
            HidRawDevice::Fts3528Touchscreen(source_driver) => source_driver.info_ref(),
            HidRawDevice::GpdWinMiniMacroKeyboard(source_driver) => source_driver.info_ref(),
            HidRawDevice::GpdWinMiniTouchpad(source_driver) => source_driver.info_ref(),
            HidRawDevice::HoripadSteam(source_driver) => source_driver.info_ref(),
            HidRawDevice::LegionGo(source_driver) => source_driver.info_ref(),
            HidRawDevice::LegionGo2(source_driver) => source_driver.info_ref(),
            HidRawDevice::LegionGoSImu(source_driver) => source_driver.info_ref(),
            HidRawDevice::LegionGoSTouchpad(source_driver) => source_driver.info_ref(),
            HidRawDevice::LegionGoSXInput(source_driver) => source_driver.info_ref(),
            HidRawDevice::OrangePiNeo(source_driver) => source_driver.info_ref(),
            HidRawDevice::OxpHid(source_driver) => source_driver.info_ref(),
            HidRawDevice::RogAlly(source_driver) => source_driver.info_ref(),
            HidRawDevice::SteamDeck(source_driver) => source_driver.info_ref(),
            HidRawDevice::Vader4Pro(source_driver) => source_driver.info_ref(),
            HidRawDevice::Ultimate2(source_driver) => source_driver.info_ref(),
            HidRawDevice::XpadUhid(source_driver) => source_driver.info_ref(),
            HidRawDevice::ZotacZone(source_driver) => source_driver.info_ref(),
        }
    }

    fn get_id(&self) -> String {
        match self {
            HidRawDevice::Blocked(source_driver) => source_driver.get_id(),
            HidRawDevice::DualSense(source_driver) => source_driver.get_id(),
            HidRawDevice::Fts3528Touchscreen(source_driver) => source_driver.get_id(),
            HidRawDevice::GpdWinMiniMacroKeyboard(source_driver) => source_driver.get_id(),
            HidRawDevice::GpdWinMiniTouchpad(source_driver) => source_driver.get_id(),
            HidRawDevice::HoripadSteam(source_driver) => source_driver.get_id(),
            HidRawDevice::LegionGo(source_driver) => source_driver.get_id(),
            HidRawDevice::LegionGo2(source_driver) => source_driver.get_id(),
            HidRawDevice::LegionGoSImu(source_driver) => source_driver.get_id(),
            HidRawDevice::LegionGoSTouchpad(source_driver) => source_driver.get_id(),
            HidRawDevice::LegionGoSXInput(source_driver) => source_driver.get_id(),
            HidRawDevice::OrangePiNeo(source_driver) => source_driver.get_id(),
            HidRawDevice::OxpHid(source_driver) => source_driver.get_id(),
            HidRawDevice::RogAlly(source_driver) => source_driver.get_id(),
            HidRawDevice::SteamDeck(source_driver) => source_driver.get_id(),
            HidRawDevice::Vader4Pro(source_driver) => source_driver.get_id(),
            HidRawDevice::Ultimate2(source_driver) => source_driver.get_id(),
            HidRawDevice::XpadUhid(source_driver) => source_driver.get_id(),
            HidRawDevice::ZotacZone(source_driver) => source_driver.get_id(),
        }
    }

    fn client(&self) -> super::client::SourceDeviceClient {
        match self {
            HidRawDevice::Blocked(source_driver) => source_driver.client(),
            HidRawDevice::DualSense(source_driver) => source_driver.client(),
            HidRawDevice::Fts3528Touchscreen(source_driver) => source_driver.client(),
            HidRawDevice::GpdWinMiniMacroKeyboard(source_driver) => source_driver.client(),
            HidRawDevice::GpdWinMiniTouchpad(source_driver) => source_driver.client(),
            HidRawDevice::HoripadSteam(source_driver) => source_driver.client(),
            HidRawDevice::LegionGo(source_driver) => source_driver.client(),
            HidRawDevice::LegionGo2(source_driver) => source_driver.client(),
            HidRawDevice::LegionGoSImu(source_driver) => source_driver.client(),
            HidRawDevice::LegionGoSTouchpad(source_driver) => source_driver.client(),
            HidRawDevice::LegionGoSXInput(source_driver) => source_driver.client(),
            HidRawDevice::OrangePiNeo(source_driver) => source_driver.client(),
            HidRawDevice::OxpHid(source_driver) => source_driver.client(),
            HidRawDevice::RogAlly(source_driver) => source_driver.client(),
            HidRawDevice::SteamDeck(source_driver) => source_driver.client(),
            HidRawDevice::Vader4Pro(source_driver) => source_driver.client(),
            HidRawDevice::Ultimate2(source_driver) => source_driver.client(),
            HidRawDevice::XpadUhid(source_driver) => source_driver.client(),
            HidRawDevice::ZotacZone(source_driver) => source_driver.client(),
        }
    }

    async fn run(self) -> Result<(), Box<dyn Error>> {
        match self {
            HidRawDevice::Blocked(source_driver) => source_driver.run().await,
            HidRawDevice::DualSense(source_driver) => source_driver.run().await,
            HidRawDevice::Fts3528Touchscreen(source_driver) => source_driver.run().await,
            HidRawDevice::GpdWinMiniMacroKeyboard(source_driver) => source_driver.run().await,
            HidRawDevice::GpdWinMiniTouchpad(source_driver) => source_driver.run().await,
            HidRawDevice::HoripadSteam(source_driver) => source_driver.run().await,
            HidRawDevice::LegionGo(source_driver) => source_driver.run().await,
            HidRawDevice::LegionGo2(source_driver) => source_driver.run().await,
            HidRawDevice::LegionGoSImu(source_driver) => source_driver.run().await,
            HidRawDevice::LegionGoSTouchpad(source_driver) => source_driver.run().await,
            HidRawDevice::LegionGoSXInput(source_driver) => source_driver.run().await,
            HidRawDevice::OrangePiNeo(source_driver) => source_driver.run().await,
            HidRawDevice::OxpHid(source_driver) => source_driver.run().await,
            HidRawDevice::RogAlly(source_driver) => source_driver.run().await,
            HidRawDevice::SteamDeck(source_driver) => source_driver.run().await,
            HidRawDevice::Vader4Pro(source_driver) => source_driver.run().await,
            HidRawDevice::Ultimate2(source_driver) => source_driver.run().await,
            HidRawDevice::XpadUhid(source_driver) => source_driver.run().await,
            HidRawDevice::ZotacZone(source_driver) => source_driver.run().await,
        }
    }

    fn get_capabilities(&self) -> Result<Vec<Capability>, InputError> {
        match self {
            HidRawDevice::Blocked(source_driver) => source_driver.get_capabilities(),
            HidRawDevice::DualSense(source_driver) => source_driver.get_capabilities(),
            HidRawDevice::Fts3528Touchscreen(source_driver) => source_driver.get_capabilities(),
            HidRawDevice::GpdWinMiniMacroKeyboard(source_driver) => {
                source_driver.get_capabilities()
            }
            HidRawDevice::GpdWinMiniTouchpad(source_driver) => source_driver.get_capabilities(),
            HidRawDevice::HoripadSteam(source_driver) => source_driver.get_capabilities(),
            HidRawDevice::LegionGo(source_driver) => source_driver.get_capabilities(),
            HidRawDevice::LegionGo2(source_driver) => source_driver.get_capabilities(),
            HidRawDevice::LegionGoSImu(source_driver) => source_driver.get_capabilities(),
            HidRawDevice::LegionGoSTouchpad(source_driver) => source_driver.get_capabilities(),
            HidRawDevice::LegionGoSXInput(source_driver) => source_driver.get_capabilities(),
            HidRawDevice::OrangePiNeo(source_driver) => source_driver.get_capabilities(),
            HidRawDevice::OxpHid(source_driver) => source_driver.get_capabilities(),
            HidRawDevice::RogAlly(source_driver) => source_driver.get_capabilities(),
            HidRawDevice::SteamDeck(source_driver) => source_driver.get_capabilities(),
            HidRawDevice::Vader4Pro(source_driver) => source_driver.get_capabilities(),
            HidRawDevice::Ultimate2(source_driver) => source_driver.get_capabilities(),
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
            HidRawDevice::GpdWinMiniMacroKeyboard(source_driver) => {
                source_driver.get_output_capabilities()
            }
            HidRawDevice::GpdWinMiniTouchpad(source_driver) => {
                source_driver.get_output_capabilities()
            }
            HidRawDevice::HoripadSteam(source_driver) => source_driver.get_output_capabilities(),
            HidRawDevice::LegionGo(source_driver) => source_driver.get_output_capabilities(),
            HidRawDevice::LegionGo2(source_driver) => source_driver.get_output_capabilities(),
            HidRawDevice::LegionGoSImu(source_driver) => source_driver.get_output_capabilities(),
            HidRawDevice::LegionGoSTouchpad(source_driver) => {
                source_driver.get_output_capabilities()
            }
            HidRawDevice::LegionGoSXInput(source_driver) => source_driver.get_output_capabilities(),
            HidRawDevice::OrangePiNeo(source_driver) => source_driver.get_output_capabilities(),
            HidRawDevice::OxpHid(source_driver) => source_driver.get_output_capabilities(),
            HidRawDevice::RogAlly(source_driver) => source_driver.get_output_capabilities(),
            HidRawDevice::SteamDeck(source_driver) => source_driver.get_output_capabilities(),
            HidRawDevice::Vader4Pro(source_driver) => source_driver.get_output_capabilities(),
            HidRawDevice::Ultimate2(source_driver) => source_driver.get_output_capabilities(),
            HidRawDevice::XpadUhid(source_driver) => source_driver.get_output_capabilities(),
            HidRawDevice::ZotacZone(source_driver) => source_driver.get_output_capabilities(),
        }
    }

    fn get_device_path(&self) -> String {
        match self {
            HidRawDevice::Blocked(source_driver) => source_driver.get_device_path(),
            HidRawDevice::DualSense(source_driver) => source_driver.get_device_path(),
            HidRawDevice::Fts3528Touchscreen(source_driver) => source_driver.get_device_path(),
            HidRawDevice::GpdWinMiniMacroKeyboard(source_driver) => source_driver.get_device_path(),
            HidRawDevice::GpdWinMiniTouchpad(source_driver) => source_driver.get_device_path(),
            HidRawDevice::HoripadSteam(source_driver) => source_driver.get_device_path(),
            HidRawDevice::LegionGo(source_driver) => source_driver.get_device_path(),
            HidRawDevice::LegionGo2(source_driver) => source_driver.get_device_path(),
            HidRawDevice::LegionGoSImu(source_driver) => source_driver.get_device_path(),
            HidRawDevice::LegionGoSTouchpad(source_driver) => source_driver.get_device_path(),
            HidRawDevice::LegionGoSXInput(source_driver) => source_driver.get_device_path(),
            HidRawDevice::OrangePiNeo(source_driver) => source_driver.get_device_path(),
            HidRawDevice::OxpHid(source_driver) => source_driver.get_device_path(),
            HidRawDevice::RogAlly(source_driver) => source_driver.get_device_path(),
            HidRawDevice::SteamDeck(source_driver) => source_driver.get_device_path(),
            HidRawDevice::Vader4Pro(source_driver) => source_driver.get_device_path(),
            HidRawDevice::Ultimate2(source_driver) => source_driver.get_device_path(),
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
            DriverType::LegionGo => {
                let device = LegionGoController::new(device_info.clone())?;
                let source_device =
                    SourceDriver::new(composite_device, device, device_info.into(), conf);
                Ok(Self::LegionGo(source_device))
            }
            DriverType::LegionGo2 => {
                let device = LegionGo2Controller::new(device_info.clone())?;
                let source_device =
                    SourceDriver::new(composite_device, device, device_info.into(), conf);
                Ok(Self::LegionGo2(source_device))
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
            DriverType::OxpHid => {
                let device = OxpHid::new(device_info.clone())?;
                let source_device =
                    SourceDriver::new(composite_device, device, device_info.into(), conf);
                Ok(Self::OxpHid(source_device))
            }

            DriverType::Ultimate2 => {
                let device = Ultimate2::new(device_info.clone())?;
                let source_device =
                    SourceDriver::new(composite_device, device, device_info.into(), conf);
                Ok(Self::Ultimate2(source_device))
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

        // Legion Go
        if vid == drivers::lego::VID
            && drivers::lego::GO1_PIDS.contains(&pid)
            && iid == drivers::lego::GP_IID
        {
            log::info!("Detected Legion Go Controller");
            return DriverType::LegionGo;
        }

        // Legion Go 2
        if vid == drivers::lego::VID
            && drivers::lego::GO2_PIDS.contains(&pid)
            && iid == drivers::lego::GP_IID
        {
            log::info!("Detected Legion Go 2 Controller");
            return DriverType::LegionGo2;
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

        // OXP X1 HID Controller
        if vid == drivers::oxp_hid::driver::VID
            && pid == drivers::oxp_hid::driver::PID
            && iid == drivers::oxp_hid::driver::IID
        {
            log::info!("Detected OXP X1 HID controller");
            return DriverType::OxpHid;
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

        if vid == drivers::ultimate_2::VID && pid == drivers::ultimate_2::PID {
            log::info!("Detected 8BitDo Ultimate 2 Gamepad");
            return DriverType::Ultimate2;
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
