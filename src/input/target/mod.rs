use std::convert::TryFrom;
use std::error::Error;
use std::fmt::Display;

use zbus::Connection;

use self::client::TargetDeviceClient;
use self::dbus::DBusDevice;
use self::dualsense::{DualSenseDevice, DualSenseHardware};
use self::keyboard::KeyboardDevice;
use self::mouse::MouseDevice;
use self::steam_deck::SteamDeckDevice;
use self::touchscreen::TouchscreenDevice;
use self::xb360::XBox360Controller;
use self::xbox_elite::XboxEliteController;
use self::xbox_series::XboxSeriesController;

pub mod client;
pub mod command;
pub mod dbus;
pub mod dualsense;
pub mod keyboard;
pub mod mouse;
pub mod steam_deck;
pub mod touchscreen;
pub mod xb360;
pub mod xbox_elite;
pub mod xbox_series;

/// TargetDeviceTypeId is a string representation of a supported TargetDevice.
/// When a new target device is added, an entry should be added to the list of
/// supported types.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct TargetDeviceTypeId {
    id: &'static str,
    name: &'static str,
}

impl TargetDeviceTypeId {
    /// Returns a list of all supported target device types
    pub fn supported_types() -> Vec<TargetDeviceTypeId> {
        vec![
            TargetDeviceTypeId {
                id: "null",
                name: "Null Device",
            },
            TargetDeviceTypeId {
                id: "dbus",
                name: "DBus Device",
            },
            TargetDeviceTypeId {
                id: "keyboard",
                name: "InputPlumber Keyboard",
            },
            TargetDeviceTypeId {
                id: "mouse",
                name: "InputPlumber Mouse",
            },
            TargetDeviceTypeId {
                id: "gamepad",
                name: "InputPlumber Gamepad",
            },
            TargetDeviceTypeId {
                id: "touchscreen",
                name: "InputPlumber Touchscreen",
            },
            TargetDeviceTypeId {
                id: "xb360",
                name: "Microsoft X-Box 360 pad",
            },
            TargetDeviceTypeId {
                id: "xbox-elite",
                name: "Microsoft X-Box One Elite pad",
            },
            TargetDeviceTypeId {
                id: "xbox-series",
                name: "Microsoft Xbox Series S|X Controller",
            },
            TargetDeviceTypeId {
                id: "deck",
                name: "Valve Steam Deck Controller",
            },
            TargetDeviceTypeId {
                id: "ds5",
                name: "Sony Interactive Entertainment DualSense Wireless Controller",
            },
            TargetDeviceTypeId {
                id: "ds5-edge",
                name: "Sony Interactive Entertainment DualSense Edge Wireless Controller",
            },
        ]
    }

    /// Return the identifier as a string
    pub fn as_str(&self) -> &str {
        self.id
    }

    /// Return the name associated with the identifier
    pub fn name(&self) -> &str {
        self.name
    }
}

impl Display for TargetDeviceTypeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.id)
    }
}

impl TryFrom<&str> for TargetDeviceTypeId {
    type Error = bool;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let supported_types = TargetDeviceTypeId::supported_types();
        for supported_type in supported_types {
            if supported_type.id == value {
                return Ok(supported_type);
            }
        }

        Err(false)
    }
}

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
    Touchscreen(touchscreen::TouchscreenDevice),
}

impl TargetDeviceType {
    /// Create a new target device from the given target device type id
    pub fn from_type_id(id: TargetDeviceTypeId, dbus: Connection) -> Self {
        match id.as_str() {
            "dbus" => Self::DBus(DBusDevice::new(dbus)),
            "deck" => Self::SteamDeck(SteamDeckDevice::new(dbus)),
            "ds5" | "ds5-usb" | "ds5-bt" | "ds5-edge" | "ds5-edge-usb" | "ds5-edge-bt" => {
                let hw = match id.as_str() {
                    "ds5" | "ds5-usb" => DualSenseHardware::new(
                        dualsense::ModelType::Normal,
                        dualsense::BusType::Usb,
                    ),
                    "ds5-bt" => DualSenseHardware::new(
                        dualsense::ModelType::Normal,
                        dualsense::BusType::Bluetooth,
                    ),
                    "ds5-edge" | "ds5-edge-usb" => {
                        DualSenseHardware::new(dualsense::ModelType::Edge, dualsense::BusType::Usb)
                    }
                    "ds5-edge-bt" => DualSenseHardware::new(
                        dualsense::ModelType::Edge,
                        dualsense::BusType::Bluetooth,
                    ),
                    _ => DualSenseHardware::default(),
                };
                Self::DualSense(DualSenseDevice::new(dbus, hw))
            }
            // Deprecated, retained for backwards compatibility
            "gamepad" => Self::XBox360(XBox360Controller::new(dbus)),
            "keyboard" => Self::Keyboard(KeyboardDevice::new(dbus)),
            "mouse" => Self::Mouse(MouseDevice::new(dbus)),
            "touchscreen" => Self::Touchscreen(TouchscreenDevice::new(dbus)),
            "xb360" => Self::XBox360(XBox360Controller::new(dbus)),
            "xbox-elite" => Self::XBoxElite(XboxEliteController::new(dbus)),
            "xbox-series" => Self::XBoxSeries(XboxSeriesController::new(dbus)),
            "null" => Self::Null,
            _ => Self::Null,
        }
    }

    /// Returns string identifiers of the target device. This string is used
    /// in some interfaces that want to specify a type of input device to use
    /// such as an input profile. E.g. "xb360", "xbox-elite", "ds5-edge"
    pub fn _type_identifiers(&self) -> Vec<TargetDeviceTypeId> {
        match self {
            TargetDeviceType::Null => vec!["null".try_into().unwrap()],
            TargetDeviceType::DBus(_) => vec!["dbus".try_into().unwrap()],
            TargetDeviceType::Keyboard(_) => vec!["keyboard".try_into().unwrap()],
            TargetDeviceType::Mouse(_) => vec!["mouse".try_into().unwrap()],
            TargetDeviceType::XBox360(_) => {
                vec!["xb360".try_into().unwrap(), "gamepad".try_into().unwrap()]
            }
            TargetDeviceType::XBoxElite(_) => vec!["xbox-elite".try_into().unwrap()],
            TargetDeviceType::XBoxSeries(_) => vec!["xbox-series".try_into().unwrap()],
            TargetDeviceType::SteamDeck(_) => vec!["deck".try_into().unwrap()],
            TargetDeviceType::DualSense(_) => vec![
                "ds5".try_into().unwrap(),
                "ds5-usb".try_into().unwrap(),
                "ds5-bt".try_into().unwrap(),
                "ds5-edge".try_into().unwrap(),
                "ds5-edge-usb".try_into().unwrap(),
                "ds5-edge-bt".try_into().unwrap(),
            ],
            TargetDeviceType::Touchscreen(_) => vec!["touchscreen".try_into().unwrap()],
        }
    }

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
