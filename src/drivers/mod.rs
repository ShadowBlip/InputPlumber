pub mod dualsense;
pub mod flydigi_vader_4_pro;
pub mod fts3528;
pub mod gpd_device;
pub mod horipad_steam;
pub mod iio_imu;
pub mod lego;
pub mod legos;
pub mod msi_claw;
pub mod opineo;
pub mod oxp_hid;
pub mod oxp_tty;
pub mod rog_ally;
pub mod steam_deck;
pub mod ultimate_2;
pub mod unified_gamepad;
pub mod xpad_uhid;
pub mod zotac_zone;

/// Hashes `bytes` using FNV-1a.
pub fn hash_id(bytes: &[u8]) -> u64 {
    /// Starting value of the hash, before any bytes are mixed in.
    const OFFSET_BASIS: u64 = 0xcbf29ce484222325;
    /// Multiplied into the hash after each byte to spread its bits.
    const PRIME: u64 = 0x100000001b3;
    bytes.iter().fold(OFFSET_BASIS, |hash, &b| {
        (hash ^ b as u64).wrapping_mul(PRIME)
    })
}
