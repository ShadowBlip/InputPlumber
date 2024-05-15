use std::{error::Error, sync::mpsc::Sender};

use ::evdev::FFEffectData;
use tokio::sync::mpsc;

use super::{capability::Capability, output_event::OutputEvent};

pub mod evdev;
pub mod hidraw;
pub mod iio;

/// A [SourceDevice] is any physical input device that emits input events
#[derive(Debug)]
pub enum SourceDevice {
    EventDevice(evdev::EventDevice),
    HIDRawDevice(hidraw::HIDRawDevice),
    IIODevice(iio::IIODevice),
}

impl SourceDevice {
    /// Returns a unique identifier for the source device.
    pub fn get_id(&self) -> String {
        match self {
            SourceDevice::EventDevice(device) => device.get_id(),
            SourceDevice::HIDRawDevice(device) => device.get_id(),
            SourceDevice::IIODevice(device) => device.get_id(),
        }
    }

    /// Returns a transmitter channel that can be used to send events to this device
    pub fn transmitter(&self) -> mpsc::Sender<SourceCommand> {
        match self {
            SourceDevice::EventDevice(device) => device.transmitter(),
            SourceDevice::HIDRawDevice(device) => device.transmitter(),
            SourceDevice::IIODevice(device) => device.transmitter(),
        }
    }

    /// Run the source device
    pub async fn run(&mut self) -> Result<(), Box<dyn Error>> {
        match self {
            SourceDevice::EventDevice(device) => device.run().await,
            SourceDevice::HIDRawDevice(device) => device.run().await,
            SourceDevice::IIODevice(device) => device.run().await,
        }
    }

    /// Returns the capabilities that this source device can fulfill.
    pub fn get_capabilities(&self) -> Result<Vec<Capability>, Box<dyn Error>> {
        match self {
            SourceDevice::EventDevice(device) => device.get_capabilities(),
            SourceDevice::HIDRawDevice(device) => device.get_capabilities(),
            SourceDevice::IIODevice(device) => device.get_capabilities(),
        }
    }

    /// Returns the full path to the device handler (e.g. /dev/input/event3, /dev/hidraw0)
    pub fn get_device_path(&self) -> String {
        match self {
            SourceDevice::EventDevice(device) => device.get_device_path(),
            SourceDevice::HIDRawDevice(device) => device.get_device_path(),
            SourceDevice::IIODevice(device) => device.get_device_path(),
        }
    }
}

/// A [SourceCommand] is a message that can be sent to a [SourceDevice] over
/// a channel.
#[derive(Debug, Clone)]
pub enum SourceCommand {
    WriteEvent(OutputEvent),
    UploadEffect(
        FFEffectData,
        Sender<Result<i16, Box<dyn Error + Send + Sync>>>,
    ),
    UpdateEffect(i16, FFEffectData),
    EraseEffect(i16, Sender<Result<(), Box<dyn Error + Send + Sync>>>),
    GetSampleRate(String, Sender<Result<f64, Box<dyn Error + Send + Sync>>>),
    GetSampleRatesAvail(
        String,
        Sender<Result<Vec<f64>, Box<dyn Error + Send + Sync>>>,
    ),
    SetSampleRate(
        String,
        f64,
        Sender<Result<(), Box<dyn Error + Send + Sync>>>,
    ),
    GetScale(String, Sender<Result<f64, Box<dyn Error + Send + Sync>>>),
    GetScalesAvail(
        String,
        Sender<Result<Vec<f64>, Box<dyn Error + Send + Sync>>>,
    ),
    SetScale(
        String,
        f64,
        Sender<Result<(), Box<dyn Error + Send + Sync>>>,
    ),
    Stop,
}
