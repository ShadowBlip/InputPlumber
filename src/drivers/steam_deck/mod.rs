#[cfg(test)]
mod mod_test;

pub mod driver;
pub mod event;
pub mod hid_report;
pub mod report_descriptor;

use super::hash_id;

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

/// Returns the serial number to report, an uppercase alphanumeric string
/// with no separators, matching the shape of a real Steam Controller/Deck
/// unit serial (e.g. "FXA996190463B").
pub fn generate_serial(persistent_id: Option<&str>) -> String {
    let hash = match persistent_id.filter(|id| !id.is_empty()) {
        Some(id) => hash_id(id.as_bytes()),
        None => 0x01ae1c0b,
    };
    format!("{:010X}", hash & 0xFF_FFFF_FFFF)
}

/// Returns the board (PCB) serial number to report, in the "M0BA######"
/// shape Steam recognizes as a valid PCB revision code.
pub fn generate_board_serial(persistent_id: Option<&str>) -> String {
    let hash = match persistent_id.filter(|id| !id.is_empty()) {
        Some(id) => hash_id(id.as_bytes()),
        None => 0x01ae1c0b,
    };
    format!("M0BA{:06}", hash % 1_000_000)
}

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
