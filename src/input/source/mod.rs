use std::error::Error;

use crate::udev::device::UdevDevice;

use self::{client::SourceDeviceClient, command::SourceCommand};

use super::capability::Capability;

pub mod client;
pub mod command;
pub mod evdev;
pub mod hidraw;
pub mod iio;

/// A [SourceDevice] is any physical input device that emits input events
#[derive(Debug)]
pub enum SourceDevice {
    Event(evdev::EventDevice),
    HIDRaw(hidraw::HIDRawDevice),
    Iio(iio::IIODevice),
}

impl SourceDevice {
    /// Returns a copy of the devices UdevDevice
    pub fn get_device(&self) -> UdevDevice {
        match self {
            SourceDevice::Event(device) => device.get_device(),
            SourceDevice::HIDRaw(device) => device.get_device(),
            SourceDevice::Iio(device) => device.get_device(),
        }
    }

    /// Returns a copy of the UdevDevice
    pub fn get_device_ref(&self) -> &UdevDevice {
        match self {
            SourceDevice::Event(device) => device.get_device_ref(),
            SourceDevice::HIDRaw(device) => device.get_device_ref(),
            SourceDevice::Iio(device) => device.get_device_ref(),
        }
    }

    /// Returns a unique identifier for the source device.
    pub fn get_id(&self) -> String {
        match self {
            SourceDevice::Event(device) => device.get_id(),
            SourceDevice::HIDRaw(device) => device.get_id(),
            SourceDevice::Iio(device) => device.get_id(),
        }
    }

    /// Returns a client channel that can be used to send events to this device
    pub fn client(&self) -> SourceDeviceClient {
        match self {
            SourceDevice::Event(device) => device.client(),
            SourceDevice::HIDRaw(device) => device.client(),
            SourceDevice::Iio(device) => device.client(),
        }
    }

    /// Run the source device
    pub async fn run(&mut self) -> Result<(), Box<dyn Error>> {
        match self {
            SourceDevice::Event(device) => device.run().await,
            SourceDevice::HIDRaw(device) => device.run().await,
            SourceDevice::Iio(device) => device.run().await,
        }
    }

    /// Returns the capabilities that this source device can fulfill.
    pub fn get_capabilities(&self) -> Result<Vec<Capability>, Box<dyn Error>> {
        match self {
            SourceDevice::Event(device) => device.get_capabilities(),
            SourceDevice::HIDRaw(device) => device.get_capabilities(),
            SourceDevice::Iio(device) => device.get_capabilities(),
        }
    }

    /// Returns the full path to the device handler (e.g. /dev/input/event3, /dev/hidraw0)
    pub fn get_device_path(&self) -> String {
        match self {
            SourceDevice::Event(device) => device.get_device_path(),
            SourceDevice::HIDRaw(device) => device.get_device_path(),
            SourceDevice::Iio(device) => device.get_device_path(),
        }
    }
}
