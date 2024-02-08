use std::error::Error;

use evdev::{AttributeSet, Device, EventSummary, EventType, InputEvent, KeyCode, KeyEvent};
use tokio::sync::broadcast;
use zbus::{fdo, Connection};
use zbus_macros::dbus_interface;

use crate::{
    constants::BUS_PREFIX,
    input::{
        capability::{Capability, Gamepad, GamepadButton},
        composite_device::Command,
        event::Event,
    },
    procfs,
};

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
    composite_tx: broadcast::Sender<Command>,
}

impl EventDevice {
    pub fn new(info: procfs::device::Device, composite_tx: broadcast::Sender<Command>) -> Self {
        Self { info, composite_tx }
    }

    /// Run the source device handler
    pub async fn run(&self) -> Result<(), Box<dyn Error>> {
        let path = self.get_device_path();
        log::debug!("Opening device at: {}", path);
        let device = Device::open(path.clone())?;

        // Read events from the device and send them to the composite device
        log::debug!("Reading events from {}", path);
        let mut events = device.into_event_stream()?;
        while let Ok(event) = events.next_event().await {
            log::trace!("Received event: {:?}", event);
            let event = Event::Evdev(event.into());
            self.composite_tx.send(Command::ProcessEvent(event))?;
        }
        log::debug!("Stopped reading device events");

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
                EventType::SYNCHRONIZATION => (),
                EventType::KEY => {
                    let Some(keys) = device.supported_keys() else {
                        continue;
                    };
                    for key in keys.iter() {
                        let capability = match key {
                            KeyCode::KEY_LEFT => Capability::None,
                            KeyCode::KEY_UP => Capability::None,
                            KeyCode::BTN_SOUTH => {
                                Capability::Gamepad(Gamepad::Button(GamepadButton::South))
                            }
                            KeyCode::BTN_NORTH => {
                                Capability::Gamepad(Gamepad::Button(GamepadButton::North))
                            }
                            KeyCode::BTN_WEST => {
                                Capability::Gamepad(Gamepad::Button(GamepadButton::West))
                            }
                            KeyCode::BTN_EAST => {
                                Capability::Gamepad(Gamepad::Button(GamepadButton::East))
                            }
                            _ => Capability::None,
                        };
                        capabilities.push(capability);
                    }
                }
                EventType::RELATIVE => (),
                EventType::ABSOLUTE => (),
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
}

/// Returns the DBus object path for evdev devices
pub fn get_dbus_path(handler: String) -> String {
    format!("{}/devices/source/{}", BUS_PREFIX, handler.clone())
}
