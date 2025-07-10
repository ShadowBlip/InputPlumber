use std::{
    collections::HashSet,
    env,
    error::Error,
    str::FromStr,
    sync::{Arc, Mutex, MutexGuard},
    thread,
    time::Duration,
};

use ::evdev::FFEffectData;
use led::LedDevice;
use thiserror::Error;
use tokio::sync::mpsc::{self, error::TryRecvError};

use crate::config;

use self::{
    client::SourceDeviceClient, command::SourceCommand, evdev::EventDevice, hidraw::HidRawDevice,
    iio::IioDevice,
};

use super::{
    capability::Capability,
    composite_device::client::CompositeDeviceClient,
    event::{context::EventContext, native::NativeEvent, Event},
    info::{DeviceInfo, DeviceInfoRef},
    output_capability::OutputCapability,
    output_event::OutputEvent,
};

pub mod client;
pub mod command;
pub mod evdev;
pub mod hidraw;
pub mod iio;
pub mod led;

/// Size of the [SourceCommand] buffer for receiving output events
const BUFFER_SIZE: usize = 2048;
/// Default poll rate (2.5ms/400Hz)
const POLL_RATE: Duration = Duration::from_micros(2500);

/// Possible errors for a source device client
#[derive(Error, Debug)]
pub enum InputError {
    #[error("InputError occurred running source device: {0}")]
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
    #[allow(dead_code)]
    #[error("Output behavior is not implemented")]
    NotImplemented,
    #[error("OutputError occurred running source device: {0}")]
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

    /// Returns the possible output events this device is capable of (e.g. force feedback, LED,
    /// etc.)
    fn get_output_capabilities(&self) -> Result<Vec<OutputCapability>, OutputError> {
        Ok(vec![])
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
    event_filter_enabled: bool,
    event_include_list: HashSet<Capability>,
    event_exclude_list: HashSet<Capability>,
    implementation: Arc<Mutex<T>>,
    device_info: DeviceInfo,
    composite_device: CompositeDeviceClient,
    tx: mpsc::Sender<SourceCommand>,
    rx: mpsc::Receiver<SourceCommand>,
}

impl<T: SourceInputDevice + SourceOutputDevice + Send + 'static> SourceDriver<T> {
    /// Create a new source device with the given implementation
    pub fn new(
        composite_device: CompositeDeviceClient,
        device: T,
        device_info: DeviceInfo,
        config: Option<config::SourceDevice>,
    ) -> Self {
        let options = SourceDriverOptions::default();
        Self::new_with_options(composite_device, device, device_info, options, config)
    }

    /// Create a new source device with the given implementation and options
    pub fn new_with_options(
        composite_device: CompositeDeviceClient,
        device: T,
        device_info: DeviceInfo,
        options: SourceDriverOptions,
        config: Option<config::SourceDevice>,
    ) -> Self {
        let (tx, rx) = mpsc::channel(options.buffer_size);

        // Check to see if the device configuration calls for event filtering
        let mut events_exclude = HashSet::new();
        let mut events_include = HashSet::new();
        if let Some(conf) = config.as_ref() {
            let events_to_exclude = conf
                .events
                .clone()
                .and_then(|e| e.exclude)
                .unwrap_or_default();
            let events_to_include = conf
                .events
                .clone()
                .and_then(|e| e.include)
                .unwrap_or_default();

            // Convert the capability strings into capabilities
            events_exclude = events_to_exclude
                .iter()
                .filter_map(|cap| Capability::from_str(cap.as_str()).ok())
                .collect();
            events_include = events_to_include
                .iter()
                .filter_map(|cap| Capability::from_str(cap.as_str()).ok())
                .collect();
        }
        let event_filter_enabled = !events_exclude.is_empty() || !events_include.is_empty();
        if event_filter_enabled {
            let devnode = device_info.path();
            if !events_include.is_empty() {
                log::debug!("Source device '{devnode}' filter includes events: {events_include:?}");
            }
            if !events_exclude.is_empty() {
                log::debug!("Source device '{devnode}' filter excludes events: {events_exclude:?}");
            }
        }

        Self {
            event_filter_enabled,
            event_include_list: events_include,
            event_exclude_list: events_exclude,
            options,
            implementation: Arc::new(Mutex::new(device)),
            device_info,
            composite_device,
            tx,
            rx,
        }
    }

    /// Returns true if the given event capability should be filtered out.
    fn should_filter(
        exclude_list: &HashSet<Capability>,
        include_list: &HashSet<Capability>,
        cap: &Capability,
    ) -> bool {
        // If the exclude list is empty, assume that all events should be filtered
        // EXCEPT for those in the include list.
        if exclude_list.is_empty() {
            // If the include list has the event, this event should not be filtered.
            if include_list.contains(cap) {
                return false;
            }
            return true;
        }

        // If the include list is empty, assume that all events should be included
        // EXCEPT for the ones in the exclude list.
        if include_list.is_empty() {
            if exclude_list.contains(cap) {
                return true;
            }
            return false;
        }

        if exclude_list.contains(cap) {
            return true;
        }
        if include_list.contains(cap) {
            return false;
        }

        false
    }

    /// Returns a unique identifier for the source device (e.g. "hidraw://hidraw0")
    pub fn get_id(&self) -> String {
        self.device_info.get_id()
    }

    /// Returns the possible input events this device is capable of emitting
    pub fn get_capabilities(&self) -> Result<Vec<Capability>, InputError> {
        let caps = { self.implementation.lock().unwrap().get_capabilities()? };

        if self.event_filter_enabled {
            return Ok(caps
                .into_iter()
                .filter(|cap| {
                    !Self::should_filter(&self.event_exclude_list, &self.event_include_list, cap)
                })
                .collect());
        }

        Ok(caps)
    }

    /// Returns the possible output events this device is capable of. (e.g. force feedback, LEDs,
    /// etc.)
    pub fn get_output_capabilities(&self) -> Result<Vec<OutputCapability>, OutputError> {
        self.implementation
            .lock()
            .unwrap()
            .get_output_capabilities()
    }

    /// Returns the path to the device (e.g. "/dev/input/event0")
    pub fn get_device_path(&self) -> String {
        self.device_info.path()
    }

    /// Returns a transmitter channel that can be used to send events to this device
    pub fn client(&self) -> SourceDeviceClient {
        self.tx.clone().into()
    }

    /// Returns udev device information about the device as a reference
    pub fn info_ref(&self) -> DeviceInfoRef {
        match &self.device_info {
            DeviceInfo::Udev(device) => device.into(),
        }
    }

    /// Run the source device, consuming the device.
    pub async fn run(self) -> Result<(), Box<dyn Error>> {
        let device_id = self.get_id();
        let metrics_enabled = match env::var("ENABLE_METRICS") {
            Ok(value) => value.as_str() == "1",
            Err(_) => false,
        };

        // Spawn a blocking task to run the source device.
        let task =
            tokio::task::spawn_blocking(move || -> Result<(), Box<dyn Error + Send + Sync>> {
                let mut rx = self.rx;
                let mut implementation = self.implementation.lock().unwrap();
                loop {
                    // Create a context with performance metrics for each event
                    let mut context = if metrics_enabled {
                        Some(EventContext::new())
                    } else {
                        None
                    };
                    if let Some(ref mut context) = context {
                        let root_span = context.metrics_mut().create_span("root");
                        root_span.start();
                    }

                    // Poll the implementation for events
                    if let Some(ref mut context) = context {
                        let poll_span = context
                            .metrics_mut()
                            .create_child_span("root", "source_poll");
                        poll_span.start();
                    }
                    let events = implementation.poll()?;
                    if let Some(ref mut context) = context {
                        let poll_span = context.metrics_mut().get_mut("source_poll").unwrap();
                        poll_span.finish();
                    }

                    // Process each event
                    for mut event in events.into_iter() {
                        if self.event_filter_enabled
                            && Self::should_filter(
                                &self.event_exclude_list,
                                &self.event_include_list,
                                &event.as_capability(),
                            )
                        {
                            continue;
                        }
                        if let Some(ref context) = context {
                            let mut context = context.clone();
                            let send_span = context
                                .metrics_mut()
                                .create_child_span("root", "source_send");
                            send_span.start();
                            event.set_context(context);
                        }
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

pub(crate) trait SourceDeviceCompatible {
    /// Returns a copy of the UdevDevice
    fn get_device_ref(&self) -> DeviceInfoRef;

    /// Returns a unique identifier for the source device.
    fn get_id(&self) -> String;

    /// Returns a client channel that can be used to send events to this device
    fn client(&self) -> SourceDeviceClient;

    /// Run the source device
    async fn run(self) -> Result<(), Box<dyn Error>>;

    /// Returns the capabilities that this source device can fulfill.
    fn get_capabilities(&self) -> Result<Vec<Capability>, InputError>;

    /// Returns the output capabilities that this source device can fulfill.
    fn get_output_capabilities(&self) -> Result<Vec<OutputCapability>, OutputError>;

    /// Returns the full path to the device handler (e.g. /dev/input/event3, /dev/hidraw0)
    fn get_device_path(&self) -> String;
}

/// A [SourceDevice] is any physical input device that emits input events
#[derive(Debug)]
pub enum SourceDevice {
    Event(EventDevice),
    HidRaw(HidRawDevice),
    Iio(IioDevice),
    Led(LedDevice),
}

impl SourceDevice {
    /// Returns a copy of the DeviceInfo
    pub fn get_device_ref(&self) -> DeviceInfoRef {
        match self {
            SourceDevice::Event(device) => device.get_device_ref(),
            SourceDevice::HidRaw(device) => device.get_device_ref(),
            SourceDevice::Iio(device) => device.get_device_ref(),
            SourceDevice::Led(device) => device.get_device_ref(),
        }
    }

    /// Returns a unique identifier for the source device.
    pub fn get_id(&self) -> String {
        match self {
            SourceDevice::Event(device) => device.get_id(),
            SourceDevice::HidRaw(device) => device.get_id(),
            SourceDevice::Iio(device) => device.get_id(),
            SourceDevice::Led(device) => device.get_id(),
        }
    }

    /// Returns a client channel that can be used to send events to this device
    pub fn client(&self) -> SourceDeviceClient {
        match self {
            SourceDevice::Event(device) => device.client(),
            SourceDevice::HidRaw(device) => device.client(),
            SourceDevice::Iio(device) => device.client(),
            SourceDevice::Led(device) => device.client(),
        }
    }

    /// Run the source device
    pub async fn run(self) -> Result<(), Box<dyn Error>> {
        match self {
            SourceDevice::Event(device) => device.run().await,
            SourceDevice::HidRaw(device) => device.run().await,
            SourceDevice::Iio(device) => device.run().await,
            SourceDevice::Led(device) => device.run().await,
        }
    }

    /// Returns the capabilities that this source device can fulfill.
    pub fn get_capabilities(&self) -> Result<Vec<Capability>, InputError> {
        match self {
            SourceDevice::Event(device) => device.get_capabilities(),
            SourceDevice::HidRaw(device) => device.get_capabilities(),
            SourceDevice::Iio(device) => device.get_capabilities(),
            SourceDevice::Led(device) => device.get_capabilities(),
        }
    }

    /// Returns the output capabilities that this source device can fulfill.
    pub fn get_output_capabilities(&self) -> Result<Vec<OutputCapability>, OutputError> {
        match self {
            SourceDevice::Event(device) => device.get_output_capabilities(),
            SourceDevice::HidRaw(device) => device.get_output_capabilities(),
            SourceDevice::Iio(device) => device.get_output_capabilities(),
            SourceDevice::Led(device) => device.get_output_capabilities(),
        }
    }

    /// Returns the full path to the device handler (e.g. /dev/input/event3, /dev/hidraw0)
    pub fn get_device_path(&self) -> String {
        match self {
            SourceDevice::Event(device) => device.get_device_path(),
            SourceDevice::HidRaw(device) => device.get_device_path(),
            SourceDevice::Iio(device) => device.get_device_path(),
            SourceDevice::Led(device) => device.get_device_path(),
        }
    }
}
