use std::error::Error;

use self::client::TargetDeviceClient;

pub mod client;
pub mod command;
pub mod dbus;
pub mod dualsense;
pub mod keyboard;
pub mod mouse;
pub mod steam_deck;
pub mod touchscreen_fts3528;
pub mod xb360;
pub mod xbox_elite;
pub mod xbox_series;

/// A [TargetDevice] is any virtual input device that emits input events
#[derive(Debug)]
pub enum TargetDeviceType {
    Null,
    DBus(dbus::DBusDevice),
    Keyboard(keyboard::KeyboardDevice),
    Mouse(mouse::MouseDevice),
    XBox360(xb360::XBox360Controller),
    XBoxElite(xbox_elite::XboxEliteController),
    XBoxSeries(xbox_series::XboxSeriesController),
    SteamDeck(steam_deck::SteamDeckDevice),
    DualSense(dualsense::DualSenseDevice),
    Touchscreen(touchscreen_fts3528::Fts3528TouchscreenDevice),
}

impl TargetDeviceType {
    /// Returns a string of the base name that should be used for this kind
    /// of device. E.g. a gamepad will return "gamepad" so it can be named
    /// "gamepad0", "gamepad1", etc. when requesting a DBus path.
    pub fn dbus_device_class(&self) -> &str {
        match self {
            TargetDeviceType::Null => "null",
            TargetDeviceType::DBus(_) => "dbus",
            TargetDeviceType::Keyboard(_) => "keyboard",
            TargetDeviceType::Mouse(_) => "mouse",
            TargetDeviceType::XBox360(_) => "gamepad",
            TargetDeviceType::XBoxElite(_) => "gamepad",
            TargetDeviceType::XBoxSeries(_) => "gamepad",
            TargetDeviceType::SteamDeck(_) => "gamepad",
            TargetDeviceType::DualSense(_) => "gamepad",
            TargetDeviceType::Touchscreen(_) => "touchscreen",
        }
    }

    /// Returns a client channel that can be used to send events to this device
    pub fn client(&self) -> Option<TargetDeviceClient> {
        match self {
            TargetDeviceType::Null => None,
            TargetDeviceType::DBus(device) => Some(device.client()),
            TargetDeviceType::Keyboard(device) => Some(device.client()),
            TargetDeviceType::Mouse(device) => Some(device.client()),
            TargetDeviceType::XBox360(device) => Some(device.client()),
            TargetDeviceType::XBoxElite(device) => Some(device.client()),
            TargetDeviceType::XBoxSeries(device) => Some(device.client()),
            TargetDeviceType::SteamDeck(device) => Some(device.client()),
            TargetDeviceType::DualSense(device) => Some(device.client()),
            TargetDeviceType::Touchscreen(device) => Some(device.client()),
        }
    }

    /// Creates a new instance of the device interface on DBus.
    pub async fn listen_on_dbus(&mut self, path: String) -> Result<(), Box<dyn Error>> {
        match self {
            TargetDeviceType::Null => Ok(()),
            TargetDeviceType::DBus(device) => device.listen_on_dbus(path).await,
            TargetDeviceType::Keyboard(device) => device.listen_on_dbus(path).await,
            TargetDeviceType::Mouse(device) => device.listen_on_dbus(path).await,
            TargetDeviceType::XBox360(device) => device.listen_on_dbus(path).await,
            TargetDeviceType::XBoxElite(device) => device.listen_on_dbus(path).await,
            TargetDeviceType::XBoxSeries(device) => device.listen_on_dbus(path).await,
            TargetDeviceType::SteamDeck(device) => device.listen_on_dbus(path).await,
            TargetDeviceType::DualSense(device) => device.listen_on_dbus(path).await,
            TargetDeviceType::Touchscreen(device) => device.listen_on_dbus(path).await,
        }
    }

    /// Run the target device
    pub async fn run(&mut self) -> Result<(), Box<dyn Error>> {
        match self {
            TargetDeviceType::Null => Ok(()),
            TargetDeviceType::DBus(device) => device.run().await,
            TargetDeviceType::Keyboard(device) => device.run().await,
            TargetDeviceType::Mouse(device) => device.run().await,
            TargetDeviceType::XBox360(device) => device.run().await,
            TargetDeviceType::XBoxElite(device) => device.run().await,
            TargetDeviceType::XBoxSeries(device) => device.run().await,
            TargetDeviceType::SteamDeck(device) => device.run().await,
            TargetDeviceType::DualSense(device) => device.run().await,
            TargetDeviceType::Touchscreen(device) => device.run().await,
        }
    }
}
