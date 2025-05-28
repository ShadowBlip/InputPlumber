use std::{collections::HashSet, env, error::Error, future::Future, str::FromStr, time::Duration};

use ::evdev::FFEffectData;
use led::LedDevice;
use thiserror::Error;
use tokio::sync::mpsc;

use crate::{
    config,
    udev::{device::UdevDevice, hide_device, unhide_device},
};

use self::{
    client::SourceDeviceClient, command::SourceCommand, evdev::EventDevice, hidraw::HidRawDevice,
    iio::IioDevice,
};

use super::{
    capability::Capability,
    composite_device::client::CompositeDeviceClient,
    event::{context::EventContext, native::NativeEvent, Event},
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

/// Options for running a source device
#[derive(Debug)]
pub struct SourceDriverOptions {
    pub buffer_size: usize,
}

impl Default for SourceDriverOptions {
    fn default() -> Self {
        Self {
            buffer_size: BUFFER_SIZE,
        }
    }
}

/// A [SourceInputDevice] is a device implementation that is capable of emitting
/// input events.
pub trait SourceInputDevice {
    /// Poll the source device for input events
    fn poll(&mut self) -> impl Future<Output = Result<Vec<NativeEvent>, InputError>> + Send {
        async {
            tokio::time::sleep(Duration::from_secs(60 * 60)).await;
            Ok(Vec::new())
        }
    }

    /// Input capabilities of the source device
    fn get_capabilities(&self) -> Result<Vec<Capability>, InputError> {
        Ok(Vec::new())
    }
}

/// A [SourceOutputDevice] is a device implementation that can handle output events
/// such as force feedback, etc.
pub trait SourceOutputDevice {
    /// Write the given output event to the source device. Output events are
    /// events that flow from an application (like a game) to the physical
    /// input device, such as force feedback events.
    fn write_event(&mut self, event: OutputEvent) -> impl Future<Output = Result<(), OutputError>> {
        //log::trace!("Received output event: {event:?}");
        let _ = event;
        async { Ok(()) }
    }

    /// Upload the given force feedback effect data to the source device. Returns
    /// a device-specific id of the uploaded effect if it is successful. Return
    /// -1 if this device does not support FF events.
    fn upload_effect(
        &mut self,
        effect: FFEffectData,
    ) -> impl Future<Output = Result<i16, OutputError>> {
        //log::trace!("Received upload effect: {effect:?}");
        let _ = effect;
        async { Ok(-1) }
    }

    /// Update the effect with the given id using the given effect data.
    fn update_effect(
        &mut self,
        effect_id: i16,
        effect: FFEffectData,
    ) -> impl Future<Output = Result<(), OutputError>> {
        //log::trace!("Received update effect: {effect_id:?} {effect:?}");
        let _ = effect;
        let _ = effect_id;
        async { Ok(()) }
    }

    /// Erase the effect with the given id from the source device.
    fn erase_effect(&mut self, effect_id: i16) -> impl Future<Output = Result<(), OutputError>> {
        //log::trace!("Received erase effect: {effect_id:?}");
        let _ = effect_id;
        async { Ok(()) }
    }

    /// Stop the source device.
    fn stop(&mut self) -> impl Future<Output = Result<(), OutputError>> {
        async { Ok(()) }
    }
}

/// A [SourceDriver] is any physical input device that emits input events
#[derive(Debug)]
pub struct SourceDriver<T: SourceInputDevice + SourceOutputDevice> {
    config: Option<config::SourceDevice>,
    is_hidden: bool,
    event_filter_enabled: bool,
    event_include_list: HashSet<Capability>,
    event_exclude_list: HashSet<Capability>,
    implementation: T,
    device_info: UdevDevice,
    composite_device: CompositeDeviceClient,
    tx: mpsc::Sender<SourceCommand>,
    rx: mpsc::Receiver<SourceCommand>,
    metrics_enabled: bool,
}

impl<T: SourceInputDevice + SourceOutputDevice + Send> SourceDriver<T> {
    /// Create a new source device with the given implementation
    pub fn new(
        composite_device: CompositeDeviceClient,
        device: T,
        device_info: UdevDevice,
        config: Option<config::SourceDevice>,
    ) -> Self {
        let options = SourceDriverOptions::default();
        Self::new_with_options(composite_device, device, device_info, options, config)
    }

    /// Create a new source device with the given implementation and options
    pub fn new_with_options(
        composite_device: CompositeDeviceClient,
        device: T,
        device_info: UdevDevice,
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
            let devnode = device_info.devnode();
            if !events_include.is_empty() {
                log::debug!("Source device '{devnode}' filter includes events: {events_include:?}");
            }
            if !events_exclude.is_empty() {
                log::debug!("Source device '{devnode}' filter excludes events: {events_exclude:?}");
            }
        }

        let metrics_enabled = match env::var("ENABLE_METRICS") {
            Ok(value) => value.as_str() == "1",
            Err(_) => false,
        };

        Self {
            config,
            is_hidden: false,
            event_filter_enabled,
            event_include_list: events_include,
            event_exclude_list: events_exclude,
            implementation: device,
            device_info,
            composite_device,
            tx,
            rx,
            metrics_enabled,
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
        let caps = self.implementation.get_capabilities()?;

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

    /// Returns the path to the device (e.g. "/dev/input/event0")
    pub fn get_device_path(&self) -> String {
        self.device_info.devnode()
    }

    /// Returns a transmitter channel that can be used to send events to this device
    pub fn client(&self) -> SourceDeviceClient {
        self.tx.clone().into()
    }

    /// Returns udev device information about the device as a reference
    pub fn info_ref(&self) -> &UdevDevice {
        &self.device_info
    }

    /// Run the source device
    pub async fn run(&mut self) -> Result<(), Box<dyn Error>> {
        // Hide the device if specified
        let should_passthru = self
            .config
            .as_ref()
            .and_then(|c| c.passthrough)
            .unwrap_or(false);
        let subsystem = self.device_info.subsystem();
        let should_hide = !should_passthru && subsystem.as_str() != "iio";
        if should_hide {
            let source_path = self.device_info.devnode();
            if let Err(e) = hide_device(source_path.as_str()).await {
                log::warn!("Failed to hide device '{source_path}': {e:?}");
            } else {
                log::debug!("Finished hiding device: {source_path}");
                self.is_hidden = true;
            }
        }

        // TODO: If the source device is blocked, don't bother polling it.

        // Run the main loop
        loop {
            // Create a context with performance metrics for each event
            let mut context = if self.metrics_enabled {
                let mut context = EventContext::new();
                {
                    let root_span = context.metrics_mut().create_span("root");
                    root_span.start();
                }
                let poll_span = context
                    .metrics_mut()
                    .create_child_span("root", "source_poll");
                poll_span.start();
                Some(context)
            } else {
                None
            };

            tokio::select! {
                // Poll the implementation for events
                result = self.implementation.poll() => {
                    let events = result?;
                    self.process_events(&mut context, events).await?;
                }
                // Receive commands/output events
                result = self.rx.recv() => {
                    let Some(cmd) = result else {
                        return Err("Receive channel disconnected".into());
                    };
                    self.process_command(cmd).await?;
                }
            }
        }
    }

    /// Process the given events and write them to the composite device
    async fn process_events(
        &self,
        context: &mut Option<EventContext>,
        events: Vec<NativeEvent>,
    ) -> Result<(), Box<dyn Error>> {
        if let Some(context) = context {
            let poll_span = context.metrics_mut().get_mut("source_poll").unwrap();
            poll_span.finish();
        }
        let device_id = self.get_id();
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
            if let Some(context) = context {
                let mut context = context.clone();
                let send_span = context
                    .metrics_mut()
                    .create_child_span("root", "source_send");
                send_span.start();
                event.set_context(context);
            }
            let event = Event::Native(event);
            let composite_device = self.composite_device.clone();
            let device_id = device_id.clone();
            // NOTE: Spawning a task to send the event to the composite device
            // appears to be significantly more performant.
            // TODO: Don't spawn a task for each event
            tokio::task::spawn(async move {
                let _ = composite_device.process_event(device_id, event).await;
            });
        }

        Ok(())
    }

    /// Read commands sent to this device from the channel
    async fn process_command(&mut self, cmd: SourceCommand) -> Result<(), Box<dyn Error>> {
        match cmd {
            SourceCommand::UploadEffect(data, composite_dev) => {
                let res = match self.implementation.upload_effect(data).await {
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
                self.implementation.update_effect(effect_id, data).await?;
            }
            SourceCommand::EraseEffect(id, composite_dev) => {
                let res = match self.implementation.erase_effect(id).await {
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
                self.implementation.write_event(event).await?;
            }
            SourceCommand::Stop => {
                self.implementation.stop().await?;
                return Err("Device stopped".into());
            }
        }

        Ok(())
    }
}

impl<T: SourceInputDevice + SourceOutputDevice> Drop for SourceDriver<T> {
    fn drop(&mut self) {
        // Unhide the device
        if !self.is_hidden {
            return;
        }
        let source_path = self.device_info.devnode();
        tokio::task::spawn(async move {
            if let Err(e) = unhide_device(source_path).await {
                log::warn!("Unable to unhide device: {e}");
            }
        });
    }
}

pub trait SourceDeviceCompatible {
    /// Returns a copy of the UdevDevice
    #[allow(dead_code)]
    fn get_device_ref(&self) -> &UdevDevice;

    /// Returns a unique identifier for the source device.
    fn get_id(&self) -> String;

    /// Returns a client channel that can be used to send events to this device
    fn client(&self) -> SourceDeviceClient;

    /// Run the source device
    fn run(self) -> impl Future<Output = Result<(), Box<dyn Error>>>;

    /// Returns the capabilities that this source device can fulfill.
    fn get_capabilities(&self) -> Result<Vec<Capability>, InputError>;

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
    /// Returns a copy of the UdevDevice
    #[allow(dead_code)]
    pub fn get_device_ref(&self) -> &UdevDevice {
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
