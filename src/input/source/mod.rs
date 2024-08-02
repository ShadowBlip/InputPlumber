use std::{
    error::Error,
    sync::{Arc, Mutex, MutexGuard},
    thread,
    time::Duration,
};

use ::evdev::FFEffectData;
use thiserror::Error;
use tokio::sync::mpsc::{self, error::TryRecvError};

use crate::udev::device::UdevDevice;

use self::{
    client::SourceDeviceClient, command::SourceCommand, evdev::EventDevice, hidraw::HidRawDevice,
    iio::IioDevice,
};

use super::{
    capability::Capability,
    composite_device::client::CompositeDeviceClient,
    event::{native::NativeEvent, Event},
    output_event::OutputEvent,
};

pub mod client;
pub mod command;
pub mod evdev;
pub mod hidraw;
pub mod iio;

/// Size of the [SourceCommand] buffer for receiving output events
const BUFFER_SIZE: usize = 2048;
/// Default poll rate (2.5ms/400Hz)
const POLL_RATE: Duration = Duration::from_micros(2500);

/// Possible errors for a source device client
#[derive(Error, Debug)]
pub enum InputError {
    #[error("error occurred running device")]
    DeviceError(String),
}

impl From<&str> for InputError {
    fn from(value: &str) -> Self {
        InputError::DeviceError(value.to_string())
    }
}

impl From<String> for InputError {
    fn from(value: String) -> Self {
        InputError::DeviceError(value)
    }
}

impl From<Box<dyn Error>> for InputError {
    fn from(value: Box<dyn Error>) -> Self {
        InputError::DeviceError(value.to_string())
    }
}

impl From<Box<dyn Error + Send + Sync>> for InputError {
    fn from(value: Box<dyn Error + Send + Sync>) -> Self {
        InputError::DeviceError(value.to_string())
    }
}

/// Possible errors for a source device client
#[derive(Error, Debug)]
pub enum OutputError {
    #[error("behavior is not implemented")]
    NotImplemented,
    #[error("error occurred running device")]
    DeviceError(String),
}

impl From<&str> for OutputError {
    fn from(value: &str) -> Self {
        OutputError::DeviceError(value.to_string())
    }
}

impl From<String> for OutputError {
    fn from(value: String) -> Self {
        OutputError::DeviceError(value)
    }
}

impl From<Box<dyn Error>> for OutputError {
    fn from(value: Box<dyn Error>) -> Self {
        OutputError::DeviceError(value.to_string())
    }
}

impl From<Box<dyn Error + Send + Sync>> for OutputError {
    fn from(value: Box<dyn Error + Send + Sync>) -> Self {
        OutputError::DeviceError(value.to_string())
    }
}

/// A [SourceInputDevice] is a device implementation that is capable of emitting
/// input events.
pub trait SourceInputDevice {
    /// Poll the given input device for input events
    fn poll(&mut self) -> Result<Vec<NativeEvent>, InputError>;

    /// Returns the possible input events this device is capable of emitting
    fn get_capabilities(&self) -> Result<Vec<Capability>, InputError>;
}

/// A [SourceOutputDevice] is a device implementation that can handle output events
/// such as force feedback, etc.
pub trait SourceOutputDevice {
    /// Write the given output event to the source device. Output events are
    /// events that flow from an application (like a game) to the physical
    /// input device, such as force feedback events.
    fn write_event(&mut self, event: OutputEvent) -> Result<(), OutputError> {
        //log::trace!("Received output event: {event:?}");
        let _ = event;
        Ok(())
    }

    /// Upload the given force feedback effect data to the source device. Returns
    /// a device-specific id of the uploaded effect if it is successful. Return
    /// -1 if this device does not support FF events.
    fn upload_effect(&mut self, effect: FFEffectData) -> Result<i16, OutputError> {
        //log::trace!("Received upload effect: {effect:?}");
        let _ = effect;
        Ok(-1)
    }

    /// Update the effect with the given id using the given effect data.
    fn update_effect(&mut self, effect_id: i16, effect: FFEffectData) -> Result<(), OutputError> {
        //log::trace!("Received update effect: {effect_id:?} {effect:?}");
        let _ = effect;
        let _ = effect_id;
        Ok(())
    }

    /// Erase the effect with the given id from the source device.
    fn erase_effect(&mut self, effect_id: i16) -> Result<(), OutputError> {
        //log::trace!("Received erase effect: {effect_id:?}");
        let _ = effect_id;
        Ok(())
    }

    /// Stop the source device.
    fn stop(&mut self) -> Result<(), OutputError> {
        Ok(())
    }
}

/// Options for running a source device
#[derive(Debug)]
pub struct SourceDriverOptions {
    pub poll_rate: Duration,
    pub buffer_size: usize,
}

impl Default for SourceDriverOptions {
    fn default() -> Self {
        Self {
            poll_rate: POLL_RATE,
            buffer_size: BUFFER_SIZE,
        }
    }
}

/// A [SourceDriver] is any physical input device that emits input events
#[derive(Debug)]
pub struct SourceDriver<T: SourceInputDevice + SourceOutputDevice> {
    options: SourceDriverOptions,
    implementation: Arc<Mutex<T>>,
    device_info: UdevDevice,
    composite_device: CompositeDeviceClient,
    tx: mpsc::Sender<SourceCommand>,
    rx: mpsc::Receiver<SourceCommand>,
}

impl<T: SourceInputDevice + SourceOutputDevice + Send + 'static> SourceDriver<T> {
    /// Create a new source device with the given implementation
    pub fn new(
        composite_device: CompositeDeviceClient,
        device: T,
        device_info: UdevDevice,
    ) -> Self {
        let options = SourceDriverOptions::default();
        let (tx, rx) = mpsc::channel(options.buffer_size);
        Self {
            options,
            implementation: Arc::new(Mutex::new(device)),
            device_info,
            composite_device,
            tx,
            rx,
        }
    }

    /// Create a new source device with the given implementation and options
    pub fn new_with_options(
        composite_device: CompositeDeviceClient,
        device: T,
        device_info: UdevDevice,
        options: SourceDriverOptions,
    ) -> Self {
        let (tx, rx) = mpsc::channel(options.buffer_size);
        Self {
            options,
            implementation: Arc::new(Mutex::new(device)),
            device_info,
            composite_device,
            tx,
            rx,
        }
    }

    /// Returns a unique identifier for the source device (e.g. "hidraw://hidraw0")
    pub fn get_id(&self) -> String {
        self.device_info.get_id()
    }

    /// Returns the possible input events this device is capable of emitting
    pub fn get_capabilities(&self) -> Result<Vec<Capability>, InputError> {
        self.implementation.lock().unwrap().get_capabilities()
    }

    /// Returns the path to the device (e.g. "/dev/input/event0")
    pub fn get_device_path(&self) -> String {
        self.device_info.devnode()
    }

    /// Returns a transmitter channel that can be used to send events to this device
    pub fn client(&self) -> SourceDeviceClient {
        self.tx.clone().into()
    }

    /// Returns udev device information about the device
    pub fn info(&self) -> UdevDevice {
        self.device_info.clone()
    }

    /// Returns udev device information about the device as a reference
    pub fn info_ref(&self) -> &UdevDevice {
        &self.device_info
    }

    /// Run the source device, consuming the device.
    pub async fn run(self) -> Result<(), Box<dyn Error>> {
        let device_id = self.get_id();

        // Spawn a blocking task to run the source device.
        let task =
            tokio::task::spawn_blocking(move || -> Result<(), Box<dyn Error + Send + Sync>> {
                let mut rx = self.rx;
                let mut implementation = self.implementation.lock().unwrap();
                loop {
                    // Poll the implementation for events
                    let events = implementation.poll()?;
                    for event in events.into_iter() {
                        let event = Event::Native(event);
                        let result = self
                            .composite_device
                            .blocking_process_event(device_id.clone(), event);
                        if let Err(e) = result {
                            return Err(e.to_string().into());
                        }
                    }

                    // Receive commands/output events
                    if let Err(e) = SourceDriver::receive_commands(&mut rx, &mut implementation) {
                        log::debug!("Error receiving commands: {:?}", e);
                        break;
                    }

                    // Sleep for the configured duration
                    thread::sleep(self.options.poll_rate);
                }

                Ok(())
            });

        // Wait for the device to finish running.
        if let Err(e) = task.await? {
            return Err(e.to_string().into());
        }

        Ok(())
    }

    /// Read commands sent to this device from the channel until it is
    /// empty.
    fn receive_commands(
        rx: &mut mpsc::Receiver<SourceCommand>,
        implementation: &mut MutexGuard<'_, T>,
    ) -> Result<(), Box<dyn Error>> {
        const MAX_COMMANDS: u8 = 64;
        let mut commands_processed = 0;
        loop {
            match rx.try_recv() {
                Ok(cmd) => match cmd {
                    SourceCommand::UploadEffect(data, composite_dev) => {
                        let res = match implementation.upload_effect(data) {
                            Ok(id) => composite_dev.send(Ok(id)),
                            Err(e) => {
                                let err = format!("Failed to upload effect: {:?}", e);
                                composite_dev.send(Err(err.into()))
                            }
                        };
                        if let Err(err) = res {
                            log::error!("Failed to send upload result: {:?}", err);
                        }
                    }
                    SourceCommand::UpdateEffect(effect_id, data) => {
                        implementation.update_effect(effect_id, data)?;
                    }
                    SourceCommand::EraseEffect(id, composite_dev) => {
                        let res = match implementation.erase_effect(id) {
                            Ok(_) => Ok(()),
                            Err(e) => {
                                let err = format!("Failed to erase effect: {e:?}");
                                composite_dev.send(Err(err.into()))
                            }
                        };
                        if let Err(err) = res {
                            log::error!("Failed to send erase result: {:?}", err);
                        }
                    }
                    SourceCommand::WriteEvent(event) => {
                        log::trace!("Received output event: {:?}", event);
                        implementation.write_event(event)?;
                    }
                    SourceCommand::Stop => {
                        implementation.stop()?;
                        return Err("Device stopped".into());
                    }
                },
                Err(e) => match e {
                    TryRecvError::Empty => return Ok(()),
                    TryRecvError::Disconnected => {
                        log::debug!("Receive channel disconnected");
                        return Err("Receive channel disconnected".into());
                    }
                },
            };

            // Only process MAX_COMMANDS messages at a time
            commands_processed += 1;
            if commands_processed >= MAX_COMMANDS {
                return Ok(());
            }
        }
    }
}

/// A [SourceDevice] is any physical input device that emits input events
#[derive(Debug)]
pub enum SourceDevice {
    Event(EventDevice),
    HidRaw(HidRawDevice),
    Iio(IioDevice),
}

impl SourceDevice {
    /// Returns a copy of the devices UdevDevice
    pub fn get_device(&self) -> UdevDevice {
        match self {
            SourceDevice::Event(device) => match device {
                EventDevice::Gamepad(device) => device.info(),
                EventDevice::Blocked(device) => device.info(),
            },
            SourceDevice::HidRaw(device) => match device {
                HidRawDevice::DualSense(device) => device.info(),
                HidRawDevice::SteamDeck(device) => device.info(),
                HidRawDevice::LegionGo(device) => device.info(),
                HidRawDevice::OrangePiNeo(device) => device.info(),
                HidRawDevice::Fts3528Touchscreen(device) => device.info(),
                HidRawDevice::XpadUhid(device) => device.info(),
                HidRawDevice::RogAlly(device) => device.info(),
            },
            SourceDevice::Iio(device) => match device {
                IioDevice::BmiImu(device) => device.info(),
                IioDevice::AccelGryo3D(device) => device.info(),
            },
        }
    }

    /// Returns a copy of the UdevDevice
    pub fn get_device_ref(&self) -> &UdevDevice {
        match self {
            SourceDevice::Event(device) => match device {
                EventDevice::Gamepad(device) => device.info_ref(),
                EventDevice::Blocked(device) => device.info_ref(),
            },
            SourceDevice::HidRaw(device) => match device {
                HidRawDevice::DualSense(device) => device.info_ref(),
                HidRawDevice::SteamDeck(device) => device.info_ref(),
                HidRawDevice::LegionGo(device) => device.info_ref(),
                HidRawDevice::OrangePiNeo(device) => device.info_ref(),
                HidRawDevice::Fts3528Touchscreen(device) => device.info_ref(),
                HidRawDevice::XpadUhid(device) => device.info_ref(),
                HidRawDevice::RogAlly(device) => device.info_ref(),
            },
            SourceDevice::Iio(device) => match device {
                IioDevice::BmiImu(device) => device.info_ref(),
                IioDevice::AccelGryo3D(device) => device.info_ref(),
            },
        }
    }

    /// Returns a unique identifier for the source device.
    pub fn get_id(&self) -> String {
        match self {
            SourceDevice::Event(device) => match device {
                EventDevice::Gamepad(device) => device.get_id(),
                EventDevice::Blocked(device) => device.get_id(),
            },
            SourceDevice::HidRaw(device) => match device {
                HidRawDevice::DualSense(device) => device.get_id(),
                HidRawDevice::SteamDeck(device) => device.get_id(),
                HidRawDevice::LegionGo(device) => device.get_id(),
                HidRawDevice::OrangePiNeo(device) => device.get_id(),
                HidRawDevice::Fts3528Touchscreen(device) => device.get_id(),
                HidRawDevice::XpadUhid(device) => device.get_id(),
                HidRawDevice::RogAlly(device) => device.get_id(),
            },
            SourceDevice::Iio(device) => match device {
                IioDevice::BmiImu(device) => device.get_id(),
                IioDevice::AccelGryo3D(device) => device.get_id(),
            },
        }
    }

    /// Returns a client channel that can be used to send events to this device
    pub fn client(&self) -> SourceDeviceClient {
        match self {
            SourceDevice::Event(device) => match device {
                EventDevice::Gamepad(device) => device.client(),
                EventDevice::Blocked(device) => device.client(),
            },
            SourceDevice::HidRaw(device) => match device {
                HidRawDevice::DualSense(device) => device.client(),
                HidRawDevice::SteamDeck(device) => device.client(),
                HidRawDevice::LegionGo(device) => device.client(),
                HidRawDevice::OrangePiNeo(device) => device.client(),
                HidRawDevice::Fts3528Touchscreen(device) => device.client(),
                HidRawDevice::XpadUhid(device) => device.client(),
                HidRawDevice::RogAlly(device) => device.client(),
            },
            SourceDevice::Iio(device) => match device {
                IioDevice::BmiImu(device) => device.client(),
                IioDevice::AccelGryo3D(device) => device.client(),
            },
        }
    }

    /// Run the source device
    pub async fn run(self) -> Result<(), Box<dyn Error>> {
        match self {
            SourceDevice::Event(device) => match device {
                EventDevice::Gamepad(device) => device.run().await,
                EventDevice::Blocked(device) => device.run().await,
            },
            SourceDevice::HidRaw(device) => match device {
                HidRawDevice::DualSense(device) => device.run().await,
                HidRawDevice::SteamDeck(device) => device.run().await,
                HidRawDevice::LegionGo(device) => device.run().await,
                HidRawDevice::OrangePiNeo(device) => device.run().await,
                HidRawDevice::Fts3528Touchscreen(device) => device.run().await,
                HidRawDevice::XpadUhid(device) => device.run().await,
                HidRawDevice::RogAlly(device) => device.run().await,
            },
            SourceDevice::Iio(device) => match device {
                IioDevice::BmiImu(device) => device.run().await,
                IioDevice::AccelGryo3D(device) => device.run().await,
            },
        }
    }

    /// Returns the capabilities that this source device can fulfill.
    pub fn get_capabilities(&self) -> Result<Vec<Capability>, InputError> {
        match self {
            SourceDevice::Event(device) => match device {
                EventDevice::Gamepad(device) => device.get_capabilities(),
                EventDevice::Blocked(device) => device.get_capabilities(),
            },
            SourceDevice::HidRaw(device) => match device {
                HidRawDevice::DualSense(device) => device.get_capabilities(),
                HidRawDevice::SteamDeck(device) => device.get_capabilities(),
                HidRawDevice::LegionGo(device) => device.get_capabilities(),
                HidRawDevice::OrangePiNeo(device) => device.get_capabilities(),
                HidRawDevice::Fts3528Touchscreen(device) => device.get_capabilities(),
                HidRawDevice::XpadUhid(device) => device.get_capabilities(),
                HidRawDevice::RogAlly(device) => device.get_capabilities(),
            },
            SourceDevice::Iio(device) => match device {
                IioDevice::BmiImu(device) => device.get_capabilities(),
                IioDevice::AccelGryo3D(device) => device.get_capabilities(),
            },
        }
    }

    /// Returns the full path to the device handler (e.g. /dev/input/event3, /dev/hidraw0)
    pub fn get_device_path(&self) -> String {
        match self {
            SourceDevice::Event(device) => match device {
                EventDevice::Gamepad(device) => device.get_device_path(),
                EventDevice::Blocked(device) => device.get_device_path(),
            },
            SourceDevice::HidRaw(device) => match device {
                HidRawDevice::DualSense(device) => device.get_device_path(),
                HidRawDevice::SteamDeck(device) => device.get_device_path(),
                HidRawDevice::LegionGo(device) => device.get_device_path(),
                HidRawDevice::OrangePiNeo(device) => device.get_device_path(),
                HidRawDevice::Fts3528Touchscreen(device) => device.get_device_path(),
                HidRawDevice::XpadUhid(device) => device.get_device_path(),
                HidRawDevice::RogAlly(device) => device.get_device_path(),
            },
            SourceDevice::Iio(device) => match device {
                IioDevice::BmiImu(device) => device.get_device_path(),
                IioDevice::AccelGryo3D(device) => device.get_device_path(),
            },
        }
    }
}
