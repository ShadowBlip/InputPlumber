use crate::{network::websocket::WebsocketClient, udev::device::UdevDevice};

#[derive(Debug, Clone)]
pub enum DeviceInfo {
    Udev(UdevDevice),
    Websocket(WebsocketClient),
}

impl DeviceInfo {
    /// Name of the device
    pub fn name(&self) -> String {
        match self {
            DeviceInfo::Udev(device) => device.name(),
            DeviceInfo::Websocket(client) => client.addr.to_string(),
        }
    }

    /// Return a unique identifier for the device based on the subsystem and
    /// sysname. E.g. "evdev://event3", "hidraw://hidraw0", "ws://127.0.0.1:8080::192.168.0.3:12345"
    pub fn get_id(&self) -> String {
        match self {
            DeviceInfo::Udev(device) => device.get_id(),
            DeviceInfo::Websocket(client) => client.get_id(),
        }
    }

    /// Path to the device (e.g. /dev/hidraw1, /dev/input/event12, ws://127.0.0.1:12345)
    pub fn path(&self) -> String {
        match self {
            DeviceInfo::Udev(device) => device.devnode(),
            DeviceInfo::Websocket(client) => format!("ws://{}", client.addr),
        }
    }

    /// The subsystem of the device if it is udev, or networking type
    pub fn kind(&self) -> String {
        match self {
            DeviceInfo::Udev(device) => device.subsystem(),
            DeviceInfo::Websocket(_) => "websocket".into(),
        }
    }
}

impl From<::udev::Device> for DeviceInfo {
    fn from(device: ::udev::Device) -> Self {
        Self::Udev(device.into())
    }
}

impl From<UdevDevice> for DeviceInfo {
    fn from(device: UdevDevice) -> Self {
        Self::Udev(device)
    }
}

impl From<WebsocketClient> for DeviceInfo {
    fn from(value: WebsocketClient) -> Self {
        Self::Websocket(value)
    }
}

/// Reference to device information
#[derive(Debug, Clone)]
pub enum DeviceInfoRef<'a> {
    Udev(&'a UdevDevice),
    Websocket(&'a WebsocketClient),
}

impl DeviceInfoRef<'_> {
    pub fn to_owned(&self) -> DeviceInfo {
        match self {
            DeviceInfoRef::Udev(device) => DeviceInfo::Udev(device.to_owned().clone()),
            DeviceInfoRef::Websocket(client) => DeviceInfo::Websocket(client.to_owned().clone()),
        }
    }
}

impl<'a> From<&'a UdevDevice> for DeviceInfoRef<'a> {
    fn from(device: &'a UdevDevice) -> Self {
        Self::Udev(device)
    }
}

impl<'a> From<&'a WebsocketClient> for DeviceInfoRef<'a> {
    fn from(value: &'a WebsocketClient) -> Self {
        Self::Websocket(value)
    }
}
