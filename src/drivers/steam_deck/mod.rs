pub mod driver;
pub mod event;
pub mod hid_report;
pub mod report_descriptor;

/// Target Device ProductIds, used to ID specific devices in SDL.
#[derive(Debug, Clone)]
pub enum ProductId {
    SteamDeck = 0x1205,
    Generic = 0x12f0,
    MsiClaw = 0x12fa,
    LenovoLegionGo2 = 0x12fb,
    ZotacZone = 0x12fc,
    AsusRogAlly = 0x12fd,
    LenovoLegionGo = 0x12fe,
    LenovoLegionGoS = 0x12ff,
}

/// Vendor ID
pub const VID: u16 = 0x28de;

/// Deck's accelerometer is +/-2g over a 16-bit signed range (16384 LSB/g).
pub const DECK_ACCEL_RAW_TO_MPS2: f64 = 9.80665 / 16384.0;
pub const DECK_MPS2_TO_ACCEL_RAW: f64 = 1.0 / DECK_ACCEL_RAW_TO_MPS2;
/// Deck's gyroscope is +/-2000 deg/s over a 16-bit signed range.
pub const DECK_GYRO_RAW_TO_RAD_S: f64 = (2000.0 / 32768.0) * (std::f64::consts::PI / 180.0);
pub const DECK_RAD_S_TO_GYRO_RAW: f64 = 1.0 / DECK_GYRO_RAW_TO_RAD_S;

impl ProductId {
    pub fn to_u16(&self) -> u16 {
        match self {
            ProductId::SteamDeck => ProductId::SteamDeck as u16,
            ProductId::Generic => ProductId::Generic as u16,
            ProductId::MsiClaw => ProductId::MsiClaw as u16,
            ProductId::LenovoLegionGo2 => ProductId::LenovoLegionGo2 as u16,
            ProductId::ZotacZone => ProductId::ZotacZone as u16,
            ProductId::AsusRogAlly => ProductId::AsusRogAlly as u16,
            ProductId::LenovoLegionGo => ProductId::LenovoLegionGo as u16,
            ProductId::LenovoLegionGoS => ProductId::LenovoLegionGoS as u16,
        }
    }

    pub fn to_u32(&self) -> u32 {
        match self {
            ProductId::SteamDeck => ProductId::SteamDeck as u32,
            ProductId::Generic => ProductId::Generic as u32,
            ProductId::MsiClaw => ProductId::MsiClaw as u32,
            ProductId::LenovoLegionGo2 => ProductId::LenovoLegionGo2 as u32,
            ProductId::ZotacZone => ProductId::ZotacZone as u32,
            ProductId::AsusRogAlly => ProductId::AsusRogAlly as u32,
            ProductId::LenovoLegionGo => ProductId::LenovoLegionGo as u32,
            ProductId::LenovoLegionGoS => ProductId::LenovoLegionGoS as u32,
        }
    }
}
