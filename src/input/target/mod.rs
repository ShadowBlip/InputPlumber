use std::{
    collections::HashSet,
    env,
    error::Error,
    io,
    sync::{Arc, Mutex, MutexGuard},
    time::Duration,
};

use debug::DebugDevice;
use horipad_steam::HoripadSteamDevice;
use steam_deck_uhid::SteamDeckUhidDevice;
use thiserror::Error;
use tokio::{
    sync::mpsc::{self, error::TryRecvError},
    task::JoinHandle,
};
use unified_gamepad::UnifiedGamepadDevice;

use crate::{
    dbus::interface::{
        performance::PerformanceInterface,
        target::{gamepad::TargetGamepadInterface, TargetInterface},
        DBusInterfaceManager,
    },
    input::target::xpad::XBoxController,
};

use super::{
    capability::Capability,
    composite_device::client::{ClientError, CompositeDeviceClient},
    event::{
        context::EventContext,
        native::{NativeEvent, ScheduledNativeEvent},
    },
    output_capability::OutputCapability,
    output_event::OutputEvent,
};

use std::convert::TryFrom;
use std::fmt::Display;

use self::client::TargetDeviceClient;
use self::command::TargetCommand;
use self::dbus::DBusDevice;
use self::dualsense::{DualSenseDevice, DualSenseHardware};
use self::keyboard::KeyboardDevice;
use self::mouse::MouseDevice;
use self::steam_deck::SteamDeckDevice;
use self::touchpad::TouchpadDevice;
use self::touchscreen::TouchscreenDevice;

pub mod client;
pub mod command;
pub mod dbus;
pub mod debug;
pub mod dualsense;
pub mod horipad_steam;
pub mod keyboard;
pub mod mouse;
pub mod steam_deck;
pub mod steam_deck_uhid;
pub mod touchpad;
pub mod touchscreen;
pub mod unified_gamepad;
pub mod xpad;

/// Possible errors for a target device client
#[derive(Error, Debug)]
pub enum InputError {
    #[error("InputError occurred running target device: {0}")]
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

impl From<io::Error> for InputError {
    fn from(value: io::Error) -> Self {
        InputError::DeviceError(value.to_string())
    }
}

impl From<ClientError> for InputError {
    fn from(value: ClientError) -> Self {
        InputError::DeviceError(value.to_string())
    }
}

/// Possible errors for a target device client
#[derive(Error, Debug)]
pub enum OutputError {
    #[allow(dead_code)]
    #[error("Output behavior is not implemented")]
    NotImplemented,
    #[error("OutputError occurred running target device: {0}")]
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

impl From<io::Error> for OutputError {
    fn from(value: io::Error) -> Self {
        OutputError::DeviceError(value.to_string())
    }
}

/// TargetDeviceTypeId is a string representation of a supported TargetDevice.
/// When a new target device is added, an entry should be added to the list of
/// supported types.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct TargetDeviceTypeId {
    id: &'static str,
    name: &'static str,
    device_class: TargetDeviceClass,
}

impl TargetDeviceTypeId {
    /// Returns a list of all supported target device types
    pub fn supported_types() -> Vec<TargetDeviceTypeId> {
        vec![
            TargetDeviceTypeId {
                id: "null",
                name: "Null Device",
                device_class: TargetDeviceClass::Null,
            },
            TargetDeviceTypeId {
                id: "dbus",
                name: "DBus Device",
                device_class: TargetDeviceClass::DBus,
            },
            TargetDeviceTypeId {
                id: "deck",
                name: "Valve Steam Deck Controller",
                device_class: TargetDeviceClass::Gamepad,
            },
            TargetDeviceTypeId {
                id: "deck-uhid",
                name: "Valve Steam Deck Controller",
                device_class: TargetDeviceClass::Gamepad,
            },
            TargetDeviceTypeId {
                id: "ds5",
                name: "Sony Interactive Entertainment DualSense Wireless Controller",
                device_class: TargetDeviceClass::Gamepad,
            },
            TargetDeviceTypeId {
                id: "ds5-edge",
                name: "Sony Interactive Entertainment DualSense Edge Wireless Controller",
                device_class: TargetDeviceClass::Gamepad,
            },
            TargetDeviceTypeId {
                id: "hori-steam",
                name: "HORI CO.,LTD. HORIPAD STEAM",
                device_class: TargetDeviceClass::Gamepad,
            },
            TargetDeviceTypeId {
                id: "keyboard",
                name: "InputPlumber Keyboard",
                device_class: TargetDeviceClass::Keyboard,
            },
            TargetDeviceTypeId {
                id: "mouse",
                name: "InputPlumber Mouse",
                device_class: TargetDeviceClass::Mouse,
            },
            TargetDeviceTypeId {
                id: "gamepad",
                name: "InputPlumber Gamepad",
                device_class: TargetDeviceClass::Gamepad,
            },
            TargetDeviceTypeId {
                id: "touchpad",
                name: "InputPlumber Touchpad",
                device_class: TargetDeviceClass::Touchpad,
            },
            TargetDeviceTypeId {
                id: "touchscreen",
                name: "InputPlumber Touchscreen",
                device_class: TargetDeviceClass::Touchscreen,
            },
            TargetDeviceTypeId {
                id: "xb360",
                name: "Microsoft X-Box 360 pad",
                device_class: TargetDeviceClass::Gamepad,
            },
            TargetDeviceTypeId {
                id: "xbox-elite",
                name: "Microsoft X-Box One Elite pad",
                device_class: TargetDeviceClass::Gamepad,
            },
            TargetDeviceTypeId {
                id: "xbox-series",
                name: "Microsoft Xbox Series S|X Controller",
                device_class: TargetDeviceClass::Gamepad,
            },
            TargetDeviceTypeId {
                id: "unified-gamepad",
                name: "InputPlumber Unified Gamepad",
                device_class: TargetDeviceClass::Gamepad,
            },
            TargetDeviceTypeId {
                id: "debug",
                name: "Debug Device",
                device_class: TargetDeviceClass::Debug,
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

    /// Returns true if the target type is a gamepad
    pub fn is_gamepad(&self) -> bool {
        !matches!(
            self.id,
            "dbus" | "null" | "touchscreen" | "touchpad" | "mouse" | "keyboard" | "debug"
        )
    }

    /// Returns device class used to get the base name that should be used for this kind
    /// of device. E.g. a gamepad will return "gamepad" so it can be named
    /// "gamepad0", "gamepad1", etc. when requesting a DBus path.
    pub fn device_class(&self) -> TargetDeviceClass {
        self.device_class
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

/// The device class describes what kind of device a target device is.
/// E.g. a gamepad will return "gamepad" so it can be named "gamepad0",
/// "gamepad1", etc. when requesting a DBus path.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum TargetDeviceClass {
    Null,
    DBus,
    Debug,
    Gamepad,
    Keyboard,
    Mouse,
    Touchscreen,
    Touchpad,
}

impl Display for TargetDeviceClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let str = match self {
            TargetDeviceClass::Null => "null",
            TargetDeviceClass::DBus => "dbus",
            TargetDeviceClass::Debug => "debug",
            TargetDeviceClass::Gamepad => "gamepad",
            TargetDeviceClass::Keyboard => "keyboard",
            TargetDeviceClass::Mouse => "mouse",
            TargetDeviceClass::Touchscreen => "touchscreen",
            TargetDeviceClass::Touchpad => "touchpad",
        };
        write!(f, "{str}")
    }
}

/// A [TargetInputDevice] is a device implementation that is capable of emitting
/// input events. Input events originate from source devices, are processed by
/// a composite device, and are sent to a target device to be emitted.
pub trait TargetInputDevice {
    /// Start the DBus interface(s) for this target device
    fn start_dbus_interface(
        &mut self,
        dbus: &mut DBusInterfaceManager,
        client: TargetDeviceClient,
        type_id: TargetDeviceTypeId,
    ) {
        log::trace!("Using device client: {client:?}");
        let iface = TargetGamepadInterface::new(type_id.name().to_owned());
        dbus.register(iface);
    }

    /// Write the given input event to the virtual device
    fn write_event(&mut self, event: NativeEvent) -> Result<(), InputError> {
        log::trace!("Discarding event: {event:?}");
        Ok(())
    }

    /// Returns the target device input capabilities that the device can handle
    fn get_capabilities(&self) -> Result<Vec<Capability>, InputError> {
        Ok(vec![])
    }

    /// Returns scheduled events that should be written later. This function will
    /// be called every poll iteration by the [TargetDriver] and schedule the
    /// events to be written at the specified time.
    fn scheduled_events(&mut self) -> Option<Vec<ScheduledNativeEvent>> {
        None
    }

    /// Clear any local state on the target device. This is typically called
    /// whenever the composite device has entered intercept mode to indicate
    /// that the target device should stop sending input.
    fn clear_state(&mut self) {
        log::debug!("Generic clear state called. Do nothing.");
    }

    /// Called when the target device has been attached to a composite device.
    fn on_composite_device_attached(
        &mut self,
        _device: CompositeDeviceClient,
    ) -> Result<(), InputError> {
        Ok(())
    }

    /// Called when the input capabilities of the source device(s) change.
    fn on_capabilities_changed(
        &mut self,
        _capabilities: HashSet<Capability>,
    ) -> Result<(), InputError> {
        Ok(())
    }

    /// Stop the target device
    fn stop(&mut self) -> Result<(), InputError> {
        Ok(())
    }
}

/// A [TargetOutputDevice] is a device implementation that is capable of emitting
/// output events such as force feedback, etc. These output events will be routed
/// to physical source devices that can handle them.
pub trait TargetOutputDevice {
    /// Poll the given device for output events. This method will be called by
    /// the target driver every polling iteration. Any output events returned by
    /// this method will be sent to the composite device to be processed. Optionally,
    /// output events can be sent directly using the provided composite device.
    fn poll(
        &mut self,
        _composite_device: &Option<CompositeDeviceClient>,
    ) -> Result<Vec<OutputEvent>, OutputError> {
        //log::trace!("Polling with composite device: {composite_device:?}");
        Ok(vec![])
    }

    /// Returns the possible output events this device is capable of emitting
    #[allow(dead_code)]
    fn get_output_capabilities(&self) -> Result<Vec<OutputCapability>, OutputError> {
        Ok(vec![])
    }

    /// Called when the output capabilities of the source device(s) change.
    fn on_output_capabilities_changed(
        &mut self,
        _capabilities: HashSet<OutputCapability>,
    ) -> Result<(), OutputError> {
        Ok(())
    }
}

/// Options for running a target device
#[derive(Debug)]
pub struct TargetDriverOptions {
    pub poll_rate: Duration,
    pub buffer_size: usize,
}

impl Default for TargetDriverOptions {
    fn default() -> Self {
        Self {
            poll_rate: Duration::from_millis(8),
            buffer_size: 2048,
        }
    }
}

/// A [TargetDriver] is any virtual input device that can emit input events
#[derive(Debug)]
pub struct TargetDriver<T: TargetInputDevice + TargetOutputDevice> {
    type_id: TargetDeviceTypeId,
    options: TargetDriverOptions,
    dbus: DBusInterfaceManager,
    implementation: Arc<Mutex<T>>,
    composite_device: Option<CompositeDeviceClient>,
    scheduled_events: Vec<ScheduledNativeEvent>,
    tx: mpsc::Sender<TargetCommand>,
    rx: mpsc::Receiver<TargetCommand>,
}

impl<T: TargetInputDevice + TargetOutputDevice + Send + 'static> TargetDriver<T> {
    /// Create a new target device with the given implementation
    pub fn new(type_id: TargetDeviceTypeId, device: T, dbus: DBusInterfaceManager) -> Self {
        let options = TargetDriverOptions::default();
        TargetDriver::new_with_options(type_id, device, dbus, options)
    }

    /// Create a new target device with the given implementation and options
    pub fn new_with_options(
        type_id: TargetDeviceTypeId,
        device: T,
        dbus: DBusInterfaceManager,
        options: TargetDriverOptions,
    ) -> Self {
        let (tx, rx) = mpsc::channel(options.buffer_size);
        Self {
            type_id,
            options,
            dbus,
            implementation: Arc::new(Mutex::new(device)),
            composite_device: None,
            scheduled_events: Vec::new(),
            rx,
            tx,
        }
    }

    /// Returns a transmitter channel that can be used to send events to this device
    pub fn client(&self) -> TargetDeviceClient {
        self.tx.clone().into()
    }

    /// Run the target device, consuming the device.
    pub async fn run(mut self) -> Result<(), Box<dyn Error>> {
        log::debug!("Started running target device: {}", self.dbus.path());
        let metrics_enabled = match env::var("ENABLE_METRICS") {
            Ok(value) => value.as_str() == "1",
            Err(_) => false,
        };

        // Spawn a blocking task to run the target device. The '?' operator should
        // be avoided in this task so cleanup tasks can run to remove the DBus
        // interface and stop the device if an error occurs.
        let client = self.client();
        let task: JoinHandle<Result<(), Box<dyn Error + Send + Sync>>> = tokio::task::spawn(
            async move {
                let mut composite_device = self.composite_device.take();
                let rx = &mut self.rx;

                // Start the DBus interface for the device
                let target_iface = TargetInterface::new(&self.type_id);
                self.dbus.register(target_iface);
                if let Ok(mut implementation) = self.implementation.lock() {
                    implementation.start_dbus_interface(&mut self.dbus, client, self.type_id);
                }

                // Start the performance metrics DBus interface for the device
                // if metrics are enabled.
                let metrics_tx = if metrics_enabled {
                    Some(Self::start_metrics_interface(&mut self.dbus))
                } else {
                    None
                };

                log::debug!("Target device running: {}", self.dbus.path());
                let mut interval = tokio::time::interval(self.options.poll_rate);
                loop {
                    // Find any scheduled events that are ready to be sent
                    let mut ready_events = vec![];
                    let mut i = 0;
                    while i < self.scheduled_events.len() {
                        if self.scheduled_events[i].is_ready() {
                            let event = self.scheduled_events.remove(i);
                            ready_events.push(event);
                            continue;
                        }
                        i += 1;
                    }
                    for event in ready_events.drain(..) {
                        let mut implementation = self.implementation.lock().unwrap();
                        if let Err(e) = implementation.write_event(event.into()) {
                            log::error!("Error writing event: {e:?}");
                            break;
                        }
                    }

                    // Receive commands/input events
                    {
                        let mut implementation = self.implementation.lock().unwrap();
                        if let Err(e) = TargetDriver::receive_commands(
                            &self.type_id,
                            &mut composite_device,
                            rx,
                            &metrics_tx,
                            &mut implementation,
                        ) {
                            log::debug!("Error receiving commands: {e:?}");
                            break;
                        }
                    }

                    // Poll the implementation for scheduled input events
                    {
                        let mut implementation = self.implementation.lock().unwrap();
                        if let Some(mut scheduled_events) = implementation.scheduled_events() {
                            self.scheduled_events.append(&mut scheduled_events);
                        }
                    }

                    // Poll the implementation for output events
                    let events = {
                        let mut implementation = self.implementation.lock().unwrap();
                        let events = match implementation.poll(&composite_device) {
                            Ok(events) => events,
                            Err(e) => {
                                log::error!("Error polling target device: {e:?}");
                                break;
                            }
                        };
                        events
                    };
                    for event in events.into_iter() {
                        let Some(ref client) = composite_device else {
                            break;
                        };

                        // Send the output event to source devices
                        let result = client.process_output_event(event).await;
                        if let Err(e) = result {
                            return Err(e.to_string().into());
                        }
                    }

                    // Sleep for the configured duration or until a command is sent
                    tokio::select! {
                        _ = interval.tick() => (),
                        Some(cmd) = rx.recv() => {
                            let mut implementation = self.implementation.lock().unwrap();
                            let result = Self::process_command(&self.type_id, &mut composite_device, &metrics_tx, &mut implementation, cmd);
                            if let Err(e) = result {
                                log::debug!("Error processing received command: {e}");
                                break;
                            }
                        }
                    }
                }

                // Stop the device
                log::debug!("Target device stopping: {}", self.dbus.path());
                self.implementation.lock().unwrap().stop()?;
                log::debug!("Target device stopped: {}", self.dbus.path());

                Ok(())
            },
        );

        // Wait for the device to finish running.
        if let Err(e) = task.await? {
            return Err(e.to_string().into());
        }

        Ok(())
    }

    /// Start the performance metrics dbus interface
    fn start_metrics_interface(
        dbus: &mut DBusInterfaceManager,
    ) -> mpsc::Sender<(Capability, EventContext)> {
        // Create a channel to emit event metrics
        let (tx, mut rx) = mpsc::channel(2048);

        // Register the metrics dbus interface
        let metrics_interface = PerformanceInterface::new();
        dbus.register(metrics_interface);

        // Spawn a task to read event metrics over the channel and emit them
        // over dbus.
        let conn = dbus.connection().clone();
        let dbus_path = dbus.path().to_string();
        tokio::task::spawn(async move {
            while let Some((cap, ctx)) = rx.recv().await {
                let result =
                    PerformanceInterface::emit_metrics(&conn, dbus_path.as_str(), cap, &ctx).await;
                if let Err(e) = result {
                    log::debug!("Error emitting metrics: {e}");
                }
            }
        });

        tx
    }

    /// Read commands sent to this device from the channel until it is
    /// empty.
    fn receive_commands(
        type_id: &TargetDeviceTypeId,
        composite_device: &mut Option<CompositeDeviceClient>,
        rx: &mut mpsc::Receiver<TargetCommand>,
        metrics_tx: &Option<mpsc::Sender<(Capability, EventContext)>>,
        implementation: &mut MutexGuard<'_, T>,
    ) -> Result<(), Box<dyn Error>> {
        const MAX_COMMANDS: u8 = 64;
        let mut commands_processed = 0;
        loop {
            match rx.try_recv() {
                Ok(cmd) => Self::process_command(
                    type_id,
                    composite_device,
                    metrics_tx,
                    implementation,
                    cmd,
                )?,
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

    /// Process the given [TargetCommand] received over a channel
    fn process_command(
        type_id: &TargetDeviceTypeId,
        composite_device: &mut Option<CompositeDeviceClient>,
        metrics_tx: &Option<mpsc::Sender<(Capability, EventContext)>>,
        implementation: &mut MutexGuard<'_, T>,
        cmd: TargetCommand,
    ) -> Result<(), Box<dyn Error>> {
        match cmd {
            TargetCommand::WriteEvent(event) => {
                Self::write_event(metrics_tx, implementation, event)?;
            }
            TargetCommand::SetCompositeDevice(device) => {
                *composite_device = Some(device.clone());
                implementation.on_composite_device_attached(device)?;
            }
            TargetCommand::GetCapabilities(sender) => {
                let capabilities = implementation.get_capabilities().unwrap_or_default();
                tokio::spawn(async move {
                    if let Err(e) = sender.send(capabilities).await {
                        log::warn!("Failed to send capabilities response: {e}");
                    }
                });
            }
            TargetCommand::NotifyCapabilitiesChanged(capabilities) => {
                implementation.on_capabilities_changed(capabilities)?;
            }
            TargetCommand::NotifyOutputCapabilitiesChanged(capabilities) => {
                implementation.on_output_capabilities_changed(capabilities)?;
            }
            TargetCommand::GetType(sender) => {
                let type_id = *type_id;
                tokio::spawn(async move {
                    if let Err(e) = sender.send(type_id).await {
                        log::warn!("Failed to send type response: {e}");
                    }
                });
            }
            TargetCommand::ClearState => {
                implementation.clear_state();
            }
            TargetCommand::Stop => {
                implementation.stop()?;
                return Err("Target device stopped".into());
            }
        }

        Ok(())
    }

    /// Write the event to the underlying target device implementation
    fn write_event(
        metrics_tx: &Option<mpsc::Sender<(Capability, EventContext)>>,
        implementation: &mut MutexGuard<'_, T>,
        event: NativeEvent,
    ) -> Result<(), Box<dyn Error>> {
        let Some(mut context) = event.get_context().cloned() else {
            implementation.write_event(event)?;
            return Ok(());
        };
        let Some(metrics_tx) = metrics_tx else {
            implementation.write_event(event)?;
            return Ok(());
        };
        let cap = event.as_capability();

        // Finish recording time for "target_send"
        if let Some(target_send_span) = context.metrics_mut().get_mut("target_send") {
            target_send_span.finish();
        }

        // Create a span to record how long writing to the target device takes
        let write_span = context
            .metrics_mut()
            .create_child_span("root", "target_write");
        write_span.start();
        implementation.write_event(event)?;
        write_span.finish();

        // Finish recording the root span with the timing for the entire event
        if let Some(root_span) = context.metrics_mut().get_mut("root") {
            root_span.finish();
        }

        // Send the metrics so they can be emitted over dbus
        if let Err(e) = metrics_tx.try_send((cap, context)) {
            match e {
                mpsc::error::TrySendError::Closed(_) => (),
                _ => {
                    log::debug!("Failed to send event metrics: {e}");
                }
            }
        }

        Ok(())
    }
}

/// A [TargetDevice] is any virtual input device that emits input events
#[derive(Debug)]
pub enum TargetDevice {
    Null,
    DBus(TargetDriver<DBusDevice>),
    Debug(TargetDriver<DebugDevice>),
    DualSense(TargetDriver<DualSenseDevice>),
    HoripadSteam(TargetDriver<HoripadSteamDevice>),
    Keyboard(TargetDriver<KeyboardDevice>),
    Mouse(TargetDriver<MouseDevice>),
    SteamDeck(TargetDriver<SteamDeckDevice>),
    SteamDeckUhid(TargetDriver<SteamDeckUhidDevice>),
    Touchpad(TargetDriver<TouchpadDevice>),
    Touchscreen(TargetDriver<TouchscreenDevice>),
    XBoxController(TargetDriver<XBoxController>),
    UnifiedGamepad(TargetDriver<UnifiedGamepadDevice>),
}

impl TargetDevice {
    /// Create a new target device from the given target device type id
    pub fn from_type_id(
        id: TargetDeviceTypeId,
        dbus: DBusInterfaceManager,
    ) -> Result<Self, Box<dyn Error>> {
        match id.as_str() {
            "dbus" => {
                let device = DBusDevice::default();
                let driver = TargetDriver::new(id, device, dbus);
                Ok(Self::DBus(driver))
            }
            "debug" => {
                let device = DebugDevice::new(dbus.connection().clone());
                let driver = TargetDriver::new(id, device, dbus);
                Ok(Self::Debug(driver))
            }
            "deck" => {
                let device = SteamDeckDevice::new()?;
                let options = TargetDriverOptions {
                    poll_rate: Duration::from_millis(4),
                    buffer_size: 2048,
                };
                let driver = TargetDriver::new_with_options(id, device, dbus, options);
                Ok(Self::SteamDeck(driver))
            }
            "deck-uhid" => {
                let device = SteamDeckUhidDevice::new()?;
                let options = TargetDriverOptions {
                    poll_rate: Duration::from_millis(4),
                    buffer_size: 2048,
                };
                let driver = TargetDriver::new_with_options(id, device, dbus, options);
                Ok(Self::SteamDeckUhid(driver))
            }
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
                let device = DualSenseDevice::new(hw)?;
                let options = TargetDriverOptions {
                    poll_rate: Duration::from_millis(4),
                    buffer_size: 2048,
                };
                let driver = TargetDriver::new_with_options(id, device, dbus, options);
                Ok(Self::DualSense(driver))
            }
            "hori-steam" => {
                let device = HoripadSteamDevice::new()?;
                let options = TargetDriverOptions {
                    poll_rate: Duration::from_millis(1),
                    buffer_size: 2048,
                };
                let driver = TargetDriver::new_with_options(id, device, dbus, options);
                Ok(Self::HoripadSteam(driver))
            }
            "keyboard" => {
                let device = KeyboardDevice::new()?;
                let driver = TargetDriver::new(id, device, dbus);
                Ok(Self::Keyboard(driver))
            }
            "mouse" => {
                let device = MouseDevice::new()?;
                let options = TargetDriverOptions {
                    poll_rate: Duration::from_millis(16),
                    buffer_size: 2048,
                };
                let driver = TargetDriver::new_with_options(id, device, dbus, options);
                Ok(Self::Mouse(driver))
            }
            "touchpad" => {
                let device = TouchpadDevice::new()?;
                let options = TargetDriverOptions {
                    poll_rate: Duration::from_millis(3),
                    buffer_size: 2048,
                };
                let driver = TargetDriver::new_with_options(id, device, dbus, options);
                Ok(Self::Touchpad(driver))
            }
            "touchscreen" => {
                let device = TouchscreenDevice::new()?;
                let options = TargetDriverOptions {
                    poll_rate: Duration::from_micros(13605),
                    buffer_size: 2048,
                };
                let driver = TargetDriver::new_with_options(id, device, dbus, options);
                Ok(Self::Touchscreen(driver))
            }
            "xb360" | "gamepad" | "xbox-elite" | "xbox-series" => {
                let device = XBoxController::new(id.as_str())?;
                let driver = TargetDriver::new(id, device, dbus);
                Ok(Self::XBoxController(driver))
            }
            "unified-gamepad" => {
                let device = UnifiedGamepadDevice::new()?;
                let driver = TargetDriver::new(id, device, dbus);
                Ok(Self::UnifiedGamepad(driver))
            }
            "null" => Ok(Self::Null),
            _ => Ok(Self::Null),
        }
    }

    /// Returns string identifiers of the target device. This string is used
    /// in some interfaces that want to specify a type of input device to use
    /// such as an input profile. E.g. "xb360", "xbox-elite", "ds5-edge"
    pub fn _type_identifiers(&self) -> Vec<TargetDeviceTypeId> {
        match self {
            TargetDevice::Null => vec!["null".try_into().unwrap()],
            TargetDevice::DBus(_) => vec!["dbus".try_into().unwrap()],
            TargetDevice::Debug(_) => vec!["debug".try_into().unwrap()],
            TargetDevice::DualSense(_) => vec![
                "ds5".try_into().unwrap(),
                "ds5-usb".try_into().unwrap(),
                "ds5-bt".try_into().unwrap(),
                "ds5-edge".try_into().unwrap(),
                "ds5-edge-usb".try_into().unwrap(),
                "ds5-edge-bt".try_into().unwrap(),
            ],
            TargetDevice::HoripadSteam(_) => vec!["hori-steam".try_into().unwrap()],
            TargetDevice::Keyboard(_) => vec!["keyboard".try_into().unwrap()],
            TargetDevice::Mouse(_) => vec!["mouse".try_into().unwrap()],
            TargetDevice::SteamDeck(_) => vec!["deck".try_into().unwrap()],
            TargetDevice::SteamDeckUhid(_) => vec!["deck-uhid".try_into().unwrap()],
            TargetDevice::Touchpad(_) => vec!["touchpad".try_into().unwrap()],
            TargetDevice::Touchscreen(_) => vec!["touchscreen".try_into().unwrap()],
            TargetDevice::XBoxController(_) => {
                vec![
                    "xb360".try_into().unwrap(),
                    "gamepad".try_into().unwrap(),
                    "xbox-elite".try_into().unwrap(),
                    "xbox-series".try_into().unwrap(),
                ]
            }
            TargetDevice::UnifiedGamepad(_) => vec!["unified-gamepad".try_into().unwrap()],
        }
    }

    /// Returns a client channel that can be used to send events to this device
    pub fn client(&self) -> Option<TargetDeviceClient> {
        match self {
            TargetDevice::Null => None,
            TargetDevice::DBus(device) => Some(device.client()),
            TargetDevice::Debug(device) => Some(device.client()),
            TargetDevice::DualSense(device) => Some(device.client()),
            TargetDevice::HoripadSteam(device) => Some(device.client()),
            TargetDevice::Keyboard(device) => Some(device.client()),
            TargetDevice::Mouse(device) => Some(device.client()),
            TargetDevice::SteamDeck(device) => Some(device.client()),
            TargetDevice::SteamDeckUhid(device) => Some(device.client()),
            TargetDevice::Touchpad(device) => Some(device.client()),
            TargetDevice::Touchscreen(device) => Some(device.client()),
            TargetDevice::XBoxController(device) => Some(device.client()),
            TargetDevice::UnifiedGamepad(device) => Some(device.client()),
        }
    }

    /// Run the target device
    pub async fn run(self) -> Result<(), Box<dyn Error>> {
        match self {
            TargetDevice::Null => Ok(()),
            TargetDevice::DBus(device) => device.run().await,
            TargetDevice::Debug(device) => device.run().await,
            TargetDevice::DualSense(device) => device.run().await,
            TargetDevice::HoripadSteam(device) => device.run().await,
            TargetDevice::Keyboard(device) => device.run().await,
            TargetDevice::Mouse(device) => device.run().await,
            TargetDevice::SteamDeck(device) => device.run().await,
            TargetDevice::SteamDeckUhid(device) => device.run().await,
            TargetDevice::Touchpad(device) => device.run().await,
            TargetDevice::Touchscreen(device) => device.run().await,
            TargetDevice::XBoxController(device) => device.run().await,
            TargetDevice::UnifiedGamepad(device) => device.run().await,
        }
    }
}
