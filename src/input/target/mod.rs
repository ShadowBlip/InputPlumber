use tokio::sync::mpsc::Sender;

use super::{
    capability::Capability, composite_device::client::CompositeDeviceClient,
    event::native::NativeEvent,
};

pub mod dbus;
pub mod dualsense;
pub mod keyboard;
pub mod mouse;
pub mod steam_deck;
pub mod touchscreen_fts3528;
pub mod xb360;
pub mod xbox_elite;

/// A [TargetDevice] is any virtual input device that emits input events
#[derive(Debug)]
pub enum TargetDeviceType {
    Null,
    DBus(dbus::DBusDevice),
    Keyboard(keyboard::KeyboardDevice),
    Mouse(mouse::MouseDevice),
    Xbox360(xb360::XBox360Controller),
    XBoxElite(xbox_elite::XboxEliteController),
    SteamDeck(steam_deck::SteamDeckDevice),
    DualSense(dualsense::DualSenseDevice),
    Touchscreen(touchscreen_fts3528::Fts3528TouchscreenDevice),
}

/// A [TargetCommand] is a message that can be sent to a [TargetDevice] over
/// a channel.
#[derive(Debug, Clone)]
pub enum TargetCommand {
    WriteEvent(NativeEvent),
    SetCompositeDevice(CompositeDeviceClient),
    GetCapabilities(Sender<Vec<Capability>>),
    Stop,
}
