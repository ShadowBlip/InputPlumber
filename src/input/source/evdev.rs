use std::{
    any::Any,
    collections::HashMap,
    error::Error,
    os::fd::AsRawFd,
    sync::{Arc, Mutex},
    time::Duration,
};

use evdev::{
    AbsInfo, AbsoluteAxisCode, Device, EventType, FFEffect, FFEffectData, FFEffectKind, FFReplay,
    FFTrigger, InputEvent,
};
use nix::fcntl::{FcntlArg, OFlag};
use tokio::{
    sync::{
        broadcast,
        mpsc::{self, error::TryRecvError},
    },
    time::sleep,
};
use zbus::{fdo, Connection};
use zbus_macros::dbus_interface;

use crate::{
    constants::BUS_PREFIX,
    drivers::dualsense::hid_report::SetStatePackedOutputData,
    input::{
        capability::Capability,
        composite_device::Command,
        event::{evdev::EvdevEvent, Event},
        output_event::OutputEvent,
    },
    procfs,
};

use super::SourceCommand;

/// Size of the [SourceCommand] buffer for receiving output events
const BUFFER_SIZE: usize = 2048;
/// How long to sleep before polling for events.
const POLL_RATE: Duration = Duration::from_micros(1666);

/// The [DBusInterface] provides a DBus interface that can be exposed for managing
/// a [Manager]. It works by sending command messages to a channel that the
/// [Manager] is listening on.
pub struct DBusInterface {
    handler: String,
    info: procfs::device::Device,
}

impl DBusInterface {
    pub fn new(handler: String, info: procfs::device::Device) -> DBusInterface {
        DBusInterface { info, handler }
    }

    /// Creates a new instance of the source evdev interface on DBus. Returns
    /// a structure with information about the source device.
    pub async fn listen_on_dbus(
        conn: Connection,
        handler: String,
        info: procfs::device::Device,
    ) -> Result<(), Box<dyn Error>> {
        let path = get_dbus_path(handler.clone());
        let iface = DBusInterface::new(handler.clone(), info);
        conn.object_server().at(path, iface).await?;
        Ok(())
    }
}

#[dbus_interface(name = "org.shadowblip.Input.Source.EventDevice")]
impl DBusInterface {
    #[dbus_interface(property)]
    async fn name(&self) -> fdo::Result<String> {
        Ok(self.info.name.clone())
    }

    #[dbus_interface(property)]
    async fn handlers(&self) -> fdo::Result<Vec<String>> {
        Ok(self.info.handlers.clone())
    }

    #[dbus_interface(property)]
    async fn phys_path(&self) -> fdo::Result<String> {
        Ok(self.info.phys_path.clone())
    }

    #[dbus_interface(property)]
    async fn sysfs_path(&self) -> fdo::Result<String> {
        Ok(self.info.sysfs_path.clone())
    }

    #[dbus_interface(property)]
    async fn unique_id(&self) -> fdo::Result<String> {
        Ok(self.info.unique_id.clone())
    }

    /// Returns the full path to the device handler (e.g. /dev/input/event3)
    #[dbus_interface(property)]
    pub fn device_path(&self) -> fdo::Result<String> {
        let handlers = &self.info.handlers;
        for handler in handlers {
            if !handler.starts_with("event") {
                continue;
            }
            return Ok(format!("/dev/input/{}", handler.clone()));
        }
        Ok("".into())
    }
}

/// [EventDevice] represents an input device using the input subsystem.
#[derive(Debug)]
pub struct EventDevice {
    info: procfs::device::Device,
    composite_tx: mpsc::Sender<Command>,
    tx: mpsc::Sender<SourceCommand>,
    rx: mpsc::Receiver<SourceCommand>,
    ff_effects: HashMap<i16, FFEffect>,
    ff_effects_dualsense: Option<i16>,
}

impl EventDevice {
    pub fn new(info: procfs::device::Device, composite_tx: mpsc::Sender<Command>) -> Self {
        let (tx, rx) = mpsc::channel(BUFFER_SIZE);
        Self {
            info,
            composite_tx,
            tx,
            rx,
            ff_effects: HashMap::new(),
            ff_effects_dualsense: None,
        }
    }

    /// Returns a transmitter channel that can be used to send events to this device
    pub fn transmitter(&self) -> mpsc::Sender<SourceCommand> {
        self.tx.clone()
    }

    /// Run the source device handler
    pub async fn run(&mut self) -> Result<(), Box<dyn Error>> {
        let path = self.get_device_path();
        log::debug!("Opening device at: {}", path);
        let mut device = Device::open(path.clone())?;
        device.grab()?;

        // Set the device to do non-blocking reads
        // TODO: use epoll to wake up when data is available
        // https://github.com/emberian/evdev/blob/main/examples/evtest_nonblocking.rs
        let raw_fd = device.as_raw_fd();
        nix::fcntl::fcntl(raw_fd, FcntlArg::F_SETFL(OFlag::O_NONBLOCK))?;

        // Query information about the device to get the absolute ranges
        let mut axes_info = HashMap::new();
        for (axis, info) in device.get_absinfo()? {
            log::trace!("Found axis: {:?}", axis);
            log::trace!("Found info: {:?}", info);
            axes_info.insert(axis, info);
        }

        // Loop to read events from the device and commands over the channel
        log::debug!("Reading events from {}", path);
        let mut interval = tokio::time::interval(POLL_RATE);
        loop {
            // Sleep for the given polling interval
            interval.tick().await;

            // Receive commands/output events
            if let Err(e) = self.receive_commands(&mut device) {
                log::debug!("Error receiving commands: {:?}", e);
                break;
            }

            // Read events from the device
            let events = match self.poll(&mut device) {
                Ok(events) => events,
                Err(err) => {
                    log::error!("Failed to poll event device: {:?}", err);
                    break;
                }
            };

            // Process events from the device
            if let Err(err) = self.process_events(events, &axes_info).await {
                log::error!("Failed to process events: {:?}", err);
                break;
            }
        }

        log::debug!("Stopped reading device events");

        Ok(())
    }

    /// Polls the evdev device for input events
    fn poll(&self, device: &mut Device) -> Result<Vec<InputEvent>, Box<dyn Error>> {
        let result = device.fetch_events();
        match result {
            Ok(events) => Ok(events.collect()),
            Err(err) => match err.kind() {
                // Do nothing if this would block
                std::io::ErrorKind::WouldBlock => Ok(vec![]),
                _ => {
                    log::trace!("Failed to fetch events: {:?}", err);
                    let msg = format!("Failed to fetch events: {:?}", err);
                    Err(msg.into())
                }
            },
        }
    }

    /// Process incoming events and send them to the composite device.
    async fn process_events(
        &self,
        events: Vec<InputEvent>,
        axes_info: &HashMap<AbsoluteAxisCode, AbsInfo>,
    ) -> Result<(), Box<dyn Error>> {
        for event in events {
            log::trace!("Received event: {:?}", event);
            // If this is an ABS event, get the min/max info for this type of
            // event so we can normalize the value.
            let abs_info = if event.event_type() == EventType::ABSOLUTE {
                axes_info.get(&AbsoluteAxisCode(event.code()))
            } else {
                None
            };

            // Convert the event into an [EvdevEvent] and optionally include
            // the axis information with min/max values
            let mut evdev_event: EvdevEvent = event.into();
            if let Some(info) = abs_info {
                evdev_event.set_abs_info(*info);
            }

            // Send the event to the composite device
            let event = Event::Evdev(evdev_event);
            self.composite_tx
                .send(Command::ProcessEvent(self.get_id(), event))
                .await?;
        }

        Ok(())
    }

    /// Read commands sent to this device from the channel until it is
    /// empty.
    fn receive_commands(&mut self, device: &mut Device) -> Result<(), Box<dyn Error>> {
        const MAX_COMMANDS: u8 = 64;
        let mut commands_processed = 0;
        loop {
            match self.rx.try_recv() {
                Ok(cmd) => match cmd {
                    SourceCommand::UploadEffect(data, composite_dev) => {
                        self.upload_ff_effect(device, data, composite_dev);
                    }
                    SourceCommand::UpdateEffect(effect_id, data) => {
                        self.update_ff_effect(effect_id, data);
                    }
                    SourceCommand::EraseEffect(id, composite_dev) => {
                        self.erase_ff_effect(id, composite_dev);
                    }
                    SourceCommand::WriteEvent(event) => {
                        log::trace!("Received output event: {:?}", event);

                        // Only process output events if FF is supported
                        let force_feedback = device.supported_ff();
                        if force_feedback.is_none() {
                            log::trace!("Device does not support FF events");
                            continue;
                        }
                        if let Some(ff) = force_feedback {
                            if ff.iter().count() == 0 {
                                log::trace!("Device has no FF support");
                                continue;
                            }
                        }

                        match event {
                            OutputEvent::Evdev(input_event) => {
                                if let Err(e) = device.send_events(&[input_event]) {
                                    log::error!("Failed to write output event: {:?}", e);
                                }
                            }
                            OutputEvent::DualSense(report) => {
                                log::debug!("Received DualSense output report");
                                if report.use_rumble_not_haptics
                                    || report.enable_improved_rumble_emulation
                                {
                                    if let Err(e) = self.process_dualsense_ff(device, report) {
                                        log::error!(
                                            "Failed to process dualsense output report: {:?}",
                                            e
                                        );
                                    }
                                }
                            }
                            OutputEvent::Uinput(_) => (),
                        }
                    }
                    SourceCommand::Stop => return Err("Device stopped".into()),
                },
                Err(e) => match e {
                    TryRecvError::Empty => return Ok(()),
                    TryRecvError::Disconnected => {
                        log::debug!("Receive channel disconnected");
                        return Err("Receive channel disconnected".into());
                    }
                },
            };

            commands_processed += 1;
            if commands_processed >= MAX_COMMANDS {
                return Ok(());
            }
        }
    }

    /// Upload the given effect data to the device and send the result to
    /// the composite device.
    fn upload_ff_effect(
        &mut self,
        device: &mut Device,
        data: evdev::FFEffectData,
        composite_dev: std::sync::mpsc::Sender<Result<i16, Box<dyn Error + Send + Sync>>>,
    ) {
        log::debug!("Uploading FF effect data");
        let res = match device.upload_ff_effect(data) {
            Ok(effect) => {
                let id = effect.id() as i16;
                self.ff_effects.insert(id, effect);
                composite_dev.send(Ok(id))
            }
            Err(e) => {
                let err = format!("Failed to upload effect: {:?}", e);
                composite_dev.send(Err(err.into()))
            }
        };
        if let Err(err) = res {
            log::error!("Failed to send upload result: {:?}", err);
        }
    }

    /// Update the given effect with the given effect data.
    fn update_ff_effect(&mut self, effect_id: i16, data: FFEffectData) {
        log::debug!("Update FF effect {effect_id}");
        let Some(effect) = self.ff_effects.get_mut(&effect_id) else {
            log::warn!("Unable to find existing FF effect with id {effect_id}");
            return;
        };

        if let Err(e) = effect.update(data) {
            log::warn!("Failed to update effect with id {effect_id}: {:?}", e);
        }
    }

    /// Erase the effect from the device with the given effect id and send the
    /// result to the composite device.
    fn erase_ff_effect(
        &mut self,
        id: i16,
        composite_dev: std::sync::mpsc::Sender<Result<(), Box<dyn Error + Send + Sync>>>,
    ) {
        log::debug!("Erasing FF effect data");
        self.ff_effects.remove(&id);
        if let Err(err) = composite_dev.send(Ok(())) {
            log::error!("Failed to send erase result: {:?}", err);
        }
    }

    /// Process dualsense force feedback output reports
    fn process_dualsense_ff(
        &mut self,
        device: &mut Device,
        report: SetStatePackedOutputData,
    ) -> Result<(), Box<dyn Error>> {
        // If no effect was uploaded to handle DualSense force feedback, upload one.
        if self.ff_effects_dualsense.is_none() {
            let effect_data = FFEffectData {
                direction: 0,
                trigger: FFTrigger {
                    button: 0,
                    interval: 0,
                },
                replay: FFReplay {
                    length: 50,
                    delay: 0,
                },
                kind: FFEffectKind::Rumble {
                    strong_magnitude: 32768,
                    weak_magnitude: 0,
                },
            };
            log::debug!("Uploading FF effect data");
            let effect = device.upload_ff_effect(effect_data)?;
            let id = effect.id() as i16;
            self.ff_effects.insert(id, effect);
            self.ff_effects_dualsense = Some(id);
        }

        let effect_id = self.ff_effects_dualsense.unwrap();
        let effect = self.ff_effects.get_mut(&effect_id).unwrap();

        // Stop playing the effect if values are set to zero
        if report.rumble_emulation_left == 0 && report.rumble_emulation_right == 0 {
            log::trace!("Stopping FF effect");
            effect.stop()?;
            return Ok(());
        }

        // Set the values of the effect and play it
        let effect_data = FFEffectData {
            direction: 0,
            trigger: FFTrigger {
                button: 0,
                interval: 0,
            },
            replay: FFReplay {
                length: 60000,
                delay: 0,
            },
            kind: FFEffectKind::Rumble {
                // DualSense values are u8, so scale them to be from u16::MIN-u16::MAX
                strong_magnitude: report.rumble_emulation_left as u16 * 256,
                weak_magnitude: report.rumble_emulation_right as u16 * 256,
            },
        };
        log::trace!("Updating effect data");
        effect.update(effect_data)?;
        log::trace!("Playing effect with data: {:?}", effect_data);
        effect.play(1)?;

        Ok(())
    }

    /// Returns a unique identifier for the source device.
    pub fn get_id(&self) -> String {
        format!("evdev://{}", self.get_event_handler())
    }

    /// Returns the name of the event handler (e.g. event3)
    pub fn get_event_handler(&self) -> String {
        let handlers = &self.info.handlers;
        for handler in handlers {
            if !handler.starts_with("event") {
                continue;
            }
            return handler.clone();
        }
        "".into()
    }

    /// Returns the full path to the device handler (e.g. /dev/input/event3)
    pub fn get_device_path(&self) -> String {
        let handler = self.get_event_handler();
        format!("/dev/input/{}", handler)
    }

    /// Returns the capabilities that this source device can fulfill.
    pub fn get_capabilities(&self) -> Result<Vec<Capability>, Box<dyn Error>> {
        let mut capabilities = vec![];

        // Open the device to get the evdev capabilities
        let path = self.get_device_path();
        log::debug!("Opening device at: {}", path);
        let device = Device::open(path.clone())?;

        // Loop through all support events
        let events = device.supported_events();
        for event in events.iter() {
            match event {
                EventType::SYNCHRONIZATION => {
                    capabilities.push(Capability::Sync);
                }
                EventType::KEY => {
                    let Some(keys) = device.supported_keys() else {
                        continue;
                    };
                    for key in keys.iter() {
                        let input_event = InputEvent::new(event.0, key.0, 0);
                        let evdev_event = EvdevEvent::from(input_event);
                        let cap = evdev_event.as_capability();
                        capabilities.push(cap);
                    }
                }
                EventType::RELATIVE => {
                    let Some(rel) = device.supported_relative_axes() else {
                        continue;
                    };
                    for axis in rel.iter() {
                        let input_event = InputEvent::new(event.0, axis.0, 0);
                        let evdev_event = EvdevEvent::from(input_event);
                        let cap = evdev_event.as_capability();
                        capabilities.push(cap);
                    }
                }
                EventType::ABSOLUTE => {
                    let Some(abs) = device.supported_absolute_axes() else {
                        continue;
                    };
                    for axis in abs.iter() {
                        let input_event = InputEvent::new(event.0, axis.0, 0);
                        let evdev_event = EvdevEvent::from(input_event);
                        let cap = evdev_event.as_capability();
                        capabilities.push(cap);
                    }
                }
                EventType::MISC => (),
                EventType::SWITCH => (),
                EventType::LED => (),
                EventType::SOUND => (),
                EventType::REPEAT => (),
                EventType::FORCEFEEDBACK => (),
                EventType::POWER => (),
                EventType::FORCEFEEDBACKSTATUS => (),
                EventType::UINPUT => (),
                _ => (),
            }
        }

        Ok(capabilities)
    }

    /// Returns the output capabilities (such as Force Feedback) that this source
    /// device can fulfill.
    pub fn get_output_capabilities(&self) -> Result<Vec<Capability>, Box<dyn Error>> {
        let capabilities = vec![];

        Ok(capabilities)
    }
}

/// Returns the DBus object path for evdev devices
pub fn get_dbus_path(handler: String) -> String {
    format!("{}/devices/source/{}", BUS_PREFIX, handler.clone())
}
