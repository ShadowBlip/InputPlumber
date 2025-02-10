use std::error::Error;
use std::fmt::Debug;
use std::os::fd::AsRawFd;

use evdev::{Device, EventType, InputEvent};
use nix::fcntl::{FcntlArg, OFlag};

use crate::{
    config::SourceDevice,
    input::{
        capability::Capability,
        event::{evdev::EvdevEvent, native::NativeEvent},
        source::{InputError, SourceInputDevice, SourceOutputDevice},
    },
    udev::device::UdevDevice,
};

/// Source device implementation for evdev gamepads
pub struct KeyboardEventDevice {
    device: Device,
}

impl KeyboardEventDevice {
    /// Create a new [KeyboardEventDevice] source device from the given udev info
    pub fn new(
        device_info: UdevDevice,
        config: &Option<SourceDevice>,
    ) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let path = device_info.devnode();
        log::debug!("Opening device at: {}", path);
        let mut device = Device::open(path.clone())?;

        // Grab exclusive access to the device
        let should_passthru = config.as_ref().and_then(|c| c.passthrough).unwrap_or(false);
        if !should_passthru {
            device.grab()?;
        }

        // Set the device to do non-blocking reads
        // TODO: use epoll to wake up when data is available
        // https://github.com/emberian/evdev/blob/main/examples/evtest_nonblocking.rs
        let raw_fd = device.as_raw_fd();
        nix::fcntl::fcntl(raw_fd, FcntlArg::F_SETFL(OFlag::O_NONBLOCK))?;

        Ok(Self { device })
    }

    /// Translate the given evdev event into a native event
    fn translate(&mut self, event: InputEvent) -> Option<NativeEvent> {
        log::trace!("Received event: {:?}", event);

        // Block Sync events, we create these at the target anyway and they waste processing.
        if event.event_type() == EventType::SYNCHRONIZATION {
            log::trace!("Holding Sync event from propagating through the processing stack.");
            return None;
        }

        // Convert the event into a [NativeEvent]
        let native_event = NativeEvent::from_evdev_raw(event.into(), None);

        Some(native_event)
    }
}

impl SourceInputDevice for KeyboardEventDevice {
    /// Poll the given input device for input events
    fn poll(&mut self) -> Result<Vec<NativeEvent>, InputError> {
        // Read events from the device
        let events = {
            let result = self.device.fetch_events();
            let events = match result {
                Ok(events) => events,
                Err(err) => match err.kind() {
                    // Do nothing if this would block
                    std::io::ErrorKind::WouldBlock => return Ok(vec![]),
                    _ => {
                        log::trace!("Failed to fetch events: {:?}", err);
                        let msg = format!("Failed to fetch events: {:?}", err);
                        return Err(msg.into());
                    }
                },
            };

            let events: Vec<InputEvent> = events.into_iter().collect();
            events
        };

        // Convert the events into native events
        let native_events = events
            .into_iter()
            .filter_map(|e| self.translate(e))
            .collect();

        Ok(native_events)
    }

    /// Returns the possible input events this device is capable of emitting
    fn get_capabilities(&self) -> Result<Vec<Capability>, InputError> {
        let mut capabilities = vec![];

        // Loop through all support events
        let events = self.device.supported_events();
        for event in events.iter() {
            match event {
                EventType::SYNCHRONIZATION => {
                    capabilities.push(Capability::Sync);
                }
                EventType::KEY => {
                    let Some(keys) = self.device.supported_keys() else {
                        continue;
                    };
                    for key in keys.iter() {
                        let input_event = InputEvent::new(event.0, key.0, 0);
                        let evdev_event = EvdevEvent::from(input_event);
                        let cap = evdev_event.as_capability();
                        capabilities.push(cap);
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

impl SourceOutputDevice for KeyboardEventDevice {}

impl Debug for KeyboardEventDevice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KeyboardEventDevice").finish()
    }
}
