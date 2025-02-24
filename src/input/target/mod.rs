use std::{
    error::Error,
    io,
    sync::{Arc, Mutex, MutexGuard},
    thread,
    time::Duration,
};

use horipad_steam::HoripadSteamDevice;
use thiserror::Error;
use tokio::sync::mpsc::{self, error::TryRecvError};
use unified_gamepad::UnifiedGamepadDevice;

use crate::{
    dbus::interface::target::{
        gamepad::TargetGamepadInterface, udev::TargetUdevDeviceInterface, TargetInterface,
    },
    udev::device::UdevDevice,
};

use super::{
    capability::Capability,
    composite_device::client::{ClientError, CompositeDeviceClient},
    event::native::{NativeEvent, ScheduledNativeEvent},
    output_capability::OutputCapability,
    output_event::OutputEvent,
};

use std::convert::TryFrom;
use std::fmt::Display;

use zbus::Connection;

use self::client::TargetDeviceClient;
use self::command::TargetCommand;
use self::dbus::DBusDevice;
use self::dualsense::{DualSenseDevice, DualSenseHardware};
use self::keyboard::KeyboardDevice;
use self::mouse::MouseDevice;
use self::steam_deck::SteamDeckDevice;
use self::touchpad::TouchpadDevice;
use self::touchscreen::TouchscreenDevice;
use self::xb360::XBox360Controller;
use self::xbox_elite::XboxEliteController;
use self::xbox_series::XboxSeriesController;

pub mod client;
pub mod command;
pub mod dbus;
pub mod dualsense;
pub mod horipad_steam;
pub mod keyboard;
pub mod mouse;
pub mod steam_deck;
pub mod touchpad;
pub mod touchscreen;
pub mod unified_gamepad;
pub mod xb360;
pub mod xbox_elite;
pub mod xbox_series;

/// Possible errors for a target device client
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
            TargetDeviceTypeId {
                id: "hori-steam",
                name: "HORI CO.,LTD. HORIPAD STEAM",
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
                id: "touchpad",
                name: "InputPlumber Touchpad",
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
                id: "unified-gamepad",
                name: "InputPlumber Unified Gamepad",
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
            "dbus" | "null" | "touchscreen" | "touchpad" | "mouse" | "keyboard"
        )
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

/// A [TargetInputDevice] is a device implementation that is capable of emitting
/// input events. Input events originate from source devices, are processed by
/// a composite device, and are sent to a target device to be emitted.
pub trait TargetInputDevice {
    /// Returns whether or not the device is ready to start its dbus interfaces
    /// and start receiving input events.
    fn is_ready(&self) -> bool {
        true
    }

    /// Start the DBus interface for this target device. Defaults to starting
    /// the `Gamepad` interface.
    fn start_dbus_interface(
        &mut self,
        dbus: Connection,
        path: String,
        client: TargetDeviceClient,
        type_id: TargetDeviceTypeId,
    ) {
        log::debug!("Starting dbus interface: {path}");
        log::trace!("Using device client: {client:?}");
        tokio::task::spawn(async move {
            let iface = TargetGamepadInterface::new(type_id.name().to_owned());

            let object_server = dbus.object_server();
            if let Err(e) = object_server.at(path.clone(), iface).await {
                log::debug!("Failed to start dbus interface for {path}: {e}");
                return;
            }
            log::debug!("Started dbus interface: {path}");
        });
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

    /// Returns the device information for the target device. This information
    /// is required to start the dbus interfaces. If the device information
    /// is not available yet, then this can return `None`, but it must return
    /// valid device information at some point in the future.
    fn get_device_info(&self) -> Result<Option<UdevDevice>, InputError>;

    /// Returns scheduled events that should be written later. This function will
    /// be called every poll iteration by the [TargetDriver] and schedule the
    /// events to be written at the specified time.
    fn scheduled_events(&mut self) -> Option<Vec<ScheduledNativeEvent>> {
        None
    }

    /// Stop the DBus interface for this target device. Defaults to stopping the
    /// `Gamepad` interface.
    fn stop_dbus_interface(&mut self, dbus: Connection, path: String) {
        log::debug!("Stopping dbus interface for {path}");
        tokio::task::spawn(async move {
            let object_server = dbus.object_server();
            if let Err(e) = object_server
                .remove::<TargetGamepadInterface, String>(path.clone())
                .await
            {
                log::debug!("Failed to stop dbus interface {path}: {e}");
                return;
            }
            log::debug!("Stopped dbus interface for {path}");
        });
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
    dbus: Connection,
    implementation: Arc<Mutex<T>>,
    composite_device: Option<CompositeDeviceClient>,
    scheduled_events: Vec<ScheduledNativeEvent>,
    tx: mpsc::Sender<TargetCommand>,
    rx: mpsc::Receiver<TargetCommand>,
}

impl<T: TargetInputDevice + TargetOutputDevice + Send + 'static> TargetDriver<T> {
    /// Create a new target device with the given implementation
    pub fn new(type_id: TargetDeviceTypeId, device: T, dbus: Connection) -> Self {
        let options = TargetDriverOptions::default();
        TargetDriver::new_with_options(type_id, device, dbus, options)
    }

    /// Create a new target device with the given implementation and options
    pub fn new_with_options(
        type_id: TargetDeviceTypeId,
        device: T,
        dbus: Connection,
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
    pub async fn run(mut self, dbus_path: String) -> Result<(), Box<dyn Error>> {
        log::debug!("Started running target device: {dbus_path}");

        // Spawn a blocking task to run the target device. The '?' operator should
        // be avoided in this task so cleanup tasks can run to remove the DBus
        // interface and stop the device if an error occurs.
        let client = self.client();
        let task =
            tokio::task::spawn_blocking(move || -> Result<(), Box<dyn Error + Send + Sync>> {
                let mut composite_device = self.composite_device;
                let mut rx = self.rx;
                let mut implementation = self.implementation.lock().unwrap();
                let mut has_started = false;

                log::debug!("Target device running: {dbus_path}");
                loop {
                    // Start the dbus interfaces for the device if they haven't
                    // started yet.
                    if !has_started {
                        let device_info = implementation.get_device_info()?;
                        if let Some(info) = device_info {
                            // Start the DBus interfaces for the device
                            Self::start_dbus_interface(
                                info,
                                self.dbus.clone(),
                                dbus_path.clone(),
                                client.clone(),
                                self.type_id,
                            );
                            implementation.start_dbus_interface(
                                self.dbus.clone(),
                                dbus_path.clone(),
                                client.clone(),
                                self.type_id,
                            );

                            has_started = true;
                        }
                    }

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
                        if let Err(e) = implementation.write_event(event.into()) {
                            log::error!("Error writing event: {e:?}");
                            break;
                        }
                    }

                    // Receive commands/input events
                    if let Err(e) = TargetDriver::receive_commands(
                        self.type_id.as_str(),
                        &mut composite_device,
                        &mut rx,
                        &mut implementation,
                    ) {
                        log::debug!("Error receiving commands: {e:?}");
                        break;
                    }

                    // Poll the implementation for scheduled input events
                    if let Some(mut scheduled_events) = implementation.scheduled_events() {
                        self.scheduled_events.append(&mut scheduled_events);
                    }

                    // Poll the implementation for output events
                    let events = match implementation.poll(&composite_device) {
                        Ok(events) => events,
                        Err(e) => {
                            log::error!("Error polling target device: {e:?}");
                            break;
                        }
                    };
                    for event in events.into_iter() {
                        let Some(ref client) = composite_device else {
                            break;
                        };

                        // Send the output event to source devices
                        let result = client.blocking_process_output_event(event);
                        if let Err(e) = result {
                            return Err(e.to_string().into());
                        }
                    }

                    // Sleep for the configured duration
                    thread::sleep(self.options.poll_rate);
                }

                // Stop the device
                log::debug!("Target device stopping: {dbus_path}");
                Self::stop_dbus_interface(self.dbus.clone(), dbus_path.clone());
                implementation.stop_dbus_interface(self.dbus, dbus_path.clone());
                implementation.stop()?;
                log::debug!("Target device stopped: {dbus_path}");

                Ok(())
            });

        // Wait for the device to finish running.
        if let Err(e) = task.await? {
            return Err(e.to_string().into());
        }

        Ok(())
    }

    /// Start the DBus interfaces that all target devices implement.
    fn start_dbus_interface(
        device_info: UdevDevice,
        dbus: Connection,
        path: String,
        client: TargetDeviceClient,
        type_id: TargetDeviceTypeId,
    ) {
        log::debug!("Starting dbus interface: {path}");
        log::trace!("Using device client: {client:?}");
        tokio::task::spawn(async move {
            let generic_interface = TargetInterface::new(&type_id);
            let udev_interface = TargetUdevDeviceInterface::new(device_info);

            let object_server = dbus.object_server();
            let (gen_result, result) = tokio::join!(
                object_server.at(path.clone(), generic_interface),
                object_server.at(path.clone(), udev_interface)
            );

            if gen_result.is_err() || result.is_err() {
                log::debug!("Failed to start dbus interface: {path} generic: {gen_result:?} type-specific: {result:?}");
            } else {
                log::debug!("Started dbus interface: {path}");
            }
        });
    }

    /// Stop the DBus interfaces that all target devices implement for this target.
    fn stop_dbus_interface(dbus: Connection, path: String) {
        log::debug!("Stopping dbus interface for {path}");
        tokio::task::spawn(async move {
            let object_server = dbus.object_server();
            let (target, generic) = tokio::join!(
                object_server.remove::<TargetUdevDeviceInterface, String>(path.clone()),
                object_server.remove::<TargetInterface, String>(path.clone())
            );
            if generic.is_err() || target.is_err() {
                log::debug!("Failed to stop dbus interface: {path} generic: {generic:?} type-specific: {target:?}");
            } else {
                log::debug!("Stopped dbus interface for {path}");
            }
        });
    }

    /// Read commands sent to this device from the channel until it is
    /// empty.
    fn receive_commands(
        type_id: &str,
        composite_device: &mut Option<CompositeDeviceClient>,
        rx: &mut mpsc::Receiver<TargetCommand>,
        implementation: &mut MutexGuard<'_, T>,
    ) -> Result<(), Box<dyn Error>> {
        const MAX_COMMANDS: u8 = 64;
        let mut commands_processed = 0;
        loop {
            match rx.try_recv() {
                Ok(cmd) => match cmd {
                    TargetCommand::WriteEvent(event) => {
                        implementation.write_event(event)?;
                    }
                    TargetCommand::SetCompositeDevice(device) => {
                        *composite_device = Some(device.clone());
                        implementation.on_composite_device_attached(device)?;
                    }
                    TargetCommand::GetCapabilities(sender) => {
                        let capabilities = implementation.get_capabilities().unwrap_or_default();
                        sender.blocking_send(capabilities)?;
                    }
                    TargetCommand::GetType(sender) => {
                        sender.blocking_send(type_id.to_string())?;
                    }
                    TargetCommand::ClearState => {
                        implementation.clear_state();
                    }
                    TargetCommand::Stop => {
                        implementation.stop()?;
                        return Err("Target device stopped".into());
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

/// A [TargetDevice] is any virtual input device that emits input events
#[derive(Debug)]
pub enum TargetDevice {
    Null,
    DBus(TargetDriver<DBusDevice>),
    DualSense(TargetDriver<DualSenseDevice>),
    HoripadSteam(TargetDriver<HoripadSteamDevice>),
    Keyboard(TargetDriver<KeyboardDevice>),
    Mouse(TargetDriver<MouseDevice>),
    SteamDeck(TargetDriver<SteamDeckDevice>),
    Touchpad(TargetDriver<TouchpadDevice>),
    Touchscreen(TargetDriver<TouchscreenDevice>),
    XBox360(TargetDriver<XBox360Controller>),
    XBoxElite(TargetDriver<XboxEliteController>),
    XBoxSeries(TargetDriver<XboxSeriesController>),
    UnifiedGamepad(TargetDriver<UnifiedGamepadDevice>),
}

impl TargetDevice {
    /// Create a new target device from the given target device type id
    pub fn from_type_id(id: TargetDeviceTypeId, dbus: Connection) -> Result<Self, Box<dyn Error>> {
        match id.as_str() {
            "dbus" => {
                let device = DBusDevice::new(dbus.clone());
                let driver = TargetDriver::new(id, device, dbus);
                Ok(Self::DBus(driver))
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
                    poll_rate: Duration::from_millis(1),
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
                    poll_rate: Duration::from_micros(13605),
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
            "xb360" | "gamepad" => {
                let device = XBox360Controller::new()?;
                let driver = TargetDriver::new(id, device, dbus);
                Ok(Self::XBox360(driver))
            }
            "xbox-elite" => {
                let device = XboxEliteController::new()?;
                let driver = TargetDriver::new(id, device, dbus);
                Ok(Self::XBoxElite(driver))
            }
            "xbox-series" => {
                let device = XboxSeriesController::new()?;
                let driver = TargetDriver::new(id, device, dbus);
                Ok(Self::XBoxSeries(driver))
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
            TargetDevice::Touchpad(_) => vec!["touchpad".try_into().unwrap()],
            TargetDevice::Touchscreen(_) => vec!["touchscreen".try_into().unwrap()],
            TargetDevice::XBox360(_) => {
                vec!["xb360".try_into().unwrap(), "gamepad".try_into().unwrap()]
            }
            TargetDevice::XBoxElite(_) => vec!["xbox-elite".try_into().unwrap()],
            TargetDevice::XBoxSeries(_) => vec!["xbox-series".try_into().unwrap()],
            TargetDevice::UnifiedGamepad(_) => vec!["unified-gamepad".try_into().unwrap()],
        }
    }

    /// Returns a string of the base name that should be used for this kind
    /// of device. E.g. a gamepad will return "gamepad" so it can be named
    /// "gamepad0", "gamepad1", etc. when requesting a DBus path.
    pub fn dbus_device_class(&self) -> &str {
        match self {
            TargetDevice::Null => "null",
            TargetDevice::DBus(_) => "dbus",
            TargetDevice::DualSense(_) => "gamepad",
            TargetDevice::HoripadSteam(_) => "gamepad",
            TargetDevice::Keyboard(_) => "keyboard",
            TargetDevice::Mouse(_) => "mouse",
            TargetDevice::SteamDeck(_) => "gamepad",
            TargetDevice::Touchpad(_) => "touchpad",
            TargetDevice::Touchscreen(_) => "touchscreen",
            TargetDevice::XBox360(_) => "gamepad",
            TargetDevice::XBoxElite(_) => "gamepad",
            TargetDevice::XBoxSeries(_) => "gamepad",
            TargetDevice::UnifiedGamepad(_) => "gamepad",
        }
    }

    /// Returns a client channel that can be used to send events to this device
    pub fn client(&self) -> Option<TargetDeviceClient> {
        match self {
            TargetDevice::Null => None,
            TargetDevice::DBus(device) => Some(device.client()),
            TargetDevice::DualSense(device) => Some(device.client()),
            TargetDevice::HoripadSteam(device) => Some(device.client()),
            TargetDevice::Keyboard(device) => Some(device.client()),
            TargetDevice::Mouse(device) => Some(device.client()),
            TargetDevice::SteamDeck(device) => Some(device.client()),
            TargetDevice::Touchpad(device) => Some(device.client()),
            TargetDevice::Touchscreen(device) => Some(device.client()),
            TargetDevice::XBox360(device) => Some(device.client()),
            TargetDevice::XBoxElite(device) => Some(device.client()),
            TargetDevice::XBoxSeries(device) => Some(device.client()),
            TargetDevice::UnifiedGamepad(device) => Some(device.client()),
        }
    }

    /// Run the target device
    pub async fn run(self, dbus_path: String) -> Result<(), Box<dyn Error>> {
        match self {
            TargetDevice::Null => Ok(()),
            TargetDevice::DBus(device) => device.run(dbus_path).await,
            TargetDevice::DualSense(device) => device.run(dbus_path).await,
            TargetDevice::HoripadSteam(device) => device.run(dbus_path).await,
            TargetDevice::Keyboard(device) => device.run(dbus_path).await,
            TargetDevice::Mouse(device) => device.run(dbus_path).await,
            TargetDevice::SteamDeck(device) => device.run(dbus_path).await,
            TargetDevice::Touchpad(device) => device.run(dbus_path).await,
            TargetDevice::Touchscreen(device) => device.run(dbus_path).await,
            TargetDevice::XBox360(device) => device.run(dbus_path).await,
            TargetDevice::XBoxElite(device) => device.run(dbus_path).await,
            TargetDevice::XBoxSeries(device) => device.run(dbus_path).await,
            TargetDevice::UnifiedGamepad(device) => device.run(dbus_path).await,
        }
    }
}
