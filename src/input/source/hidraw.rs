pub mod blocked;
pub mod dualsense;
pub mod flydigi_vader_4_pro;
pub mod fts3528;
pub mod horipad_steam;
pub mod lego_dinput_combined;
pub mod lego_dinput_split;
pub mod lego_fps_mode;
pub mod lego_xinput;
pub mod legos_config;
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
use horipad_steam::HoripadSteam;
use legos_config::LegionSConfigController;
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
    dualsense::DualSenseController, fts3528::Fts3528Touchscreen,
    lego_dinput_combined::LegionControllerDCombined, lego_dinput_split::LegionControllerDSplit,
    lego_fps_mode::LegionControllerFPS, lego_xinput::LegionControllerX,
    legos_xinput::LegionSXInputController, opineo::OrangePiNeoTouchpad, steam_deck::DeckController,
};

use super::{InputError, OutputError, SourceDeviceCompatible, SourceDriver, SourceDriverOptions};

/// List of available drivers
enum DriverType {
    Blocked,
    DualSense,
    Fts3528Touchscreen,
    HoripadSteam,
    LegionGoDCombined,
    LegionGoDSplit,
    LegionGoFPS,
    LegionGoSConfig,
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
    HoripadSteam(SourceDriver<HoripadSteam>),
    LegionGoDCombined(SourceDriver<LegionControllerDCombined>),
    LegionGoDSplit(SourceDriver<LegionControllerDSplit>),
    LegionGoFPS(SourceDriver<LegionControllerFPS>),
    LegionGoSConfig(SourceDriver<LegionSConfigController>),
    LegionGoSImu(SourceDriver<LegionSImuController>),
    LegionGoSTouchpad(SourceDriver<LegionSTouchpadController>),
    LegionGoSXInput(SourceDriver<LegionSXInputController>),
    LegionGoXInput(SourceDriver<LegionControllerX>),
    MsiClaw(SourceDriver<MsiClaw>),
    OrangePiNeo(SourceDriver<OrangePiNeoTouchpad>),
    RogAlly(SourceDriver<RogAlly>),
    SteamDeck(SourceDriver<DeckController>),
    Vader4Pro(SourceDriver<Vader4Pro>),
    XpadUhid(SourceDriver<XpadUhid>),
    ZotacZone(SourceDriver<ZotacZone>),
}

impl SourceDeviceCompatible for HidRawDevice {
    fn get_device_ref(&self) -> DeviceInfoRef {
        match self {
            HidRawDevice::Blocked(source_driver) => source_driver.info_ref(),
            HidRawDevice::DualSense(source_driver) => source_driver.info_ref(),
            HidRawDevice::Fts3528Touchscreen(source_driver) => source_driver.info_ref(),
            HidRawDevice::HoripadSteam(source_driver) => source_driver.info_ref(),
            HidRawDevice::LegionGoDCombined(source_driver) => source_driver.info_ref(),
            HidRawDevice::LegionGoDSplit(source_driver) => source_driver.info_ref(),
            HidRawDevice::LegionGoFPS(source_driver) => source_driver.info_ref(),
            HidRawDevice::LegionGoSConfig(source_driver) => source_driver.info_ref(),
            HidRawDevice::LegionGoSImu(source_driver) => source_driver.info_ref(),
            HidRawDevice::LegionGoSTouchpad(source_driver) => source_driver.info_ref(),
            HidRawDevice::LegionGoSXInput(source_driver) => source_driver.info_ref(),
            HidRawDevice::LegionGoXInput(source_driver) => source_driver.info_ref(),
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
            HidRawDevice::HoripadSteam(source_driver) => source_driver.get_id(),
            HidRawDevice::LegionGoDCombined(source_driver) => source_driver.get_id(),
            HidRawDevice::LegionGoDSplit(source_driver) => source_driver.get_id(),
            HidRawDevice::LegionGoFPS(source_driver) => source_driver.get_id(),
            HidRawDevice::LegionGoSConfig(source_driver) => source_driver.get_id(),
            HidRawDevice::LegionGoSImu(source_driver) => source_driver.get_id(),
            HidRawDevice::LegionGoSTouchpad(source_driver) => source_driver.get_id(),
            HidRawDevice::LegionGoSXInput(source_driver) => source_driver.get_id(),
            HidRawDevice::LegionGoXInput(source_driver) => source_driver.get_id(),
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
            HidRawDevice::HoripadSteam(source_driver) => source_driver.client(),
            HidRawDevice::LegionGoDCombined(source_driver) => source_driver.client(),
            HidRawDevice::LegionGoDSplit(source_driver) => source_driver.client(),
            HidRawDevice::LegionGoFPS(source_driver) => source_driver.client(),
            HidRawDevice::LegionGoSConfig(source_driver) => source_driver.client(),
            HidRawDevice::LegionGoSImu(source_driver) => source_driver.client(),
            HidRawDevice::LegionGoSTouchpad(source_driver) => source_driver.client(),
            HidRawDevice::LegionGoSXInput(source_driver) => source_driver.client(),
            HidRawDevice::LegionGoXInput(source_driver) => source_driver.client(),
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
            HidRawDevice::HoripadSteam(source_driver) => source_driver.run().await,
            HidRawDevice::LegionGoDCombined(source_driver) => source_driver.run().await,
            HidRawDevice::LegionGoDSplit(source_driver) => source_driver.run().await,
            HidRawDevice::LegionGoFPS(source_driver) => source_driver.run().await,
            HidRawDevice::LegionGoSConfig(source_driver) => source_driver.run().await,
            HidRawDevice::LegionGoSImu(source_driver) => source_driver.run().await,
            HidRawDevice::LegionGoSTouchpad(source_driver) => source_driver.run().await,
            HidRawDevice::LegionGoSXInput(source_driver) => source_driver.run().await,
            HidRawDevice::LegionGoXInput(source_driver) => source_driver.run().await,
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
            HidRawDevice::HoripadSteam(source_driver) => source_driver.get_capabilities(),
            HidRawDevice::LegionGoDCombined(source_driver) => source_driver.get_capabilities(),
            HidRawDevice::LegionGoDSplit(source_driver) => source_driver.get_capabilities(),
            HidRawDevice::LegionGoFPS(source_driver) => source_driver.get_capabilities(),
            HidRawDevice::LegionGoSConfig(source_driver) => source_driver.get_capabilities(),
            HidRawDevice::LegionGoSImu(source_driver) => source_driver.get_capabilities(),
            HidRawDevice::LegionGoSTouchpad(source_driver) => source_driver.get_capabilities(),
            HidRawDevice::LegionGoSXInput(source_driver) => source_driver.get_capabilities(),
            HidRawDevice::LegionGoXInput(source_driver) => source_driver.get_capabilities(),
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
            HidRawDevice::HoripadSteam(source_driver) => source_driver.get_output_capabilities(),
            HidRawDevice::LegionGoDCombined(source_driver) => {
                source_driver.get_output_capabilities()
            }
            HidRawDevice::LegionGoDSplit(source_driver) => source_driver.get_output_capabilities(),
            HidRawDevice::LegionGoFPS(source_driver) => source_driver.get_output_capabilities(),
            HidRawDevice::LegionGoSConfig(source_driver) => source_driver.get_output_capabilities(),
            HidRawDevice::LegionGoSImu(source_driver) => source_driver.get_output_capabilities(),
            HidRawDevice::LegionGoSTouchpad(source_driver) => {
                source_driver.get_output_capabilities()
            }
            HidRawDevice::LegionGoSXInput(source_driver) => source_driver.get_output_capabilities(),
            HidRawDevice::LegionGoXInput(source_driver) => source_driver.get_output_capabilities(),
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
            HidRawDevice::HoripadSteam(source_driver) => source_driver.get_device_path(),
            HidRawDevice::LegionGoDCombined(source_driver) => source_driver.get_device_path(),
            HidRawDevice::LegionGoDSplit(source_driver) => source_driver.get_device_path(),
            HidRawDevice::LegionGoFPS(source_driver) => source_driver.get_device_path(),
            HidRawDevice::LegionGoSConfig(source_driver) => source_driver.get_device_path(),
            HidRawDevice::LegionGoSImu(source_driver) => source_driver.get_device_path(),
            HidRawDevice::LegionGoSTouchpad(source_driver) => source_driver.get_device_path(),
            HidRawDevice::LegionGoSXInput(source_driver) => source_driver.get_device_path(),
            HidRawDevice::LegionGoXInput(source_driver) => source_driver.get_device_path(),
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
            DriverType::LegionGoDCombined => {
                let device = LegionControllerDCombined::new(device_info.clone())?;
                let source_device =
                    SourceDriver::new(composite_device, device, device_info.into(), conf);
                Ok(Self::LegionGoDCombined(source_device))
            }
            DriverType::LegionGoDSplit => {
                let device = LegionControllerDSplit::new(device_info.clone())?;
                let source_device =
                    SourceDriver::new(composite_device, device, device_info.into(), conf);
                Ok(Self::LegionGoDSplit(source_device))
            }
            DriverType::LegionGoFPS => {
                let device = LegionControllerFPS::new(device_info.clone())?;
                let source_device =
                    SourceDriver::new(composite_device, device, device_info.into(), conf);
                Ok(Self::LegionGoFPS(source_device))
            }
            DriverType::LegionGoXInput => {
                let device = LegionControllerX::new(device_info.clone())?;
                let source_device =
                    SourceDriver::new(composite_device, device, device_info.into(), conf);
                Ok(Self::LegionGoXInput(source_device))
            }
            DriverType::LegionGoSConfig => {
                let options = SourceDriverOptions {
                    poll_rate: Duration::from_secs(1),
                    buffer_size: 2048,
                };
                let device = LegionSConfigController::new(device_info.clone())?;
                let source_device = SourceDriver::new_with_options(
                    composite_device,
                    device,
                    device_info.into(),
                    options,
                    conf,
                );
                Ok(Self::LegionGoSConfig(source_device))
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
        if vid == steam_deck::VID && pid == steam_deck::PID {
            log::info!("Detected Steam Deck");
            return DriverType::SteamDeck;
        }

        // Legion Go Dinput Combined
        if vid == drivers::lego::driver_dinput_combined::VID
            && drivers::lego::driver_dinput_combined::PIDS.contains(&pid)
        {
            log::info!("Detected Legion Go DInput Combined Mode");
            return DriverType::LegionGoDCombined;
        }

        // Legion Go Dinput Split
        if vid == drivers::lego::driver_dinput_split::VID
            && drivers::lego::driver_dinput_split::PIDS.contains(&pid)
        {
            log::info!("Detected Legion Go DInput Split Mode");
            return DriverType::LegionGoDSplit;
        }

        // Legion Go FPS Mode
        if vid == drivers::lego::driver_fps_mode::VID && drivers::lego::driver_fps_mode::PIDS.contains(&pid)
        {
            log::info!("Detected Legion Go FPS Mode");
            return DriverType::LegionGoFPS;
        }

        // Legion Go XInput
        if vid == drivers::lego::driver_xinput::VID && drivers::lego::driver_xinput::PIDS.contains(&pid) {
            log::info!("Detected Legion Go XInput Mode");
            return DriverType::LegionGoXInput;
        }

        // Legion Go S Config
        if vid == drivers::legos::VID
            && drivers::legos::PIDS.contains(&pid)
            && iid == drivers::legos::CFG_IID
        {
            log::info!("Detected Legion Go S Config");
            return DriverType::LegionGoSConfig;
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

        // Unknown
        log::warn!("No driver for hidraw interface found. VID: {vid}, PID: {pid}");
        DriverType::Unknown
    }
}

/// Returns the DBus path for a [HIDRawDevice] from a device path (E.g. /dev/hidraw0)
pub fn get_dbus_path(device_name: String) -> String {
    format!("{}/{}", BUS_SOURCES_PREFIX, device_name)
}
