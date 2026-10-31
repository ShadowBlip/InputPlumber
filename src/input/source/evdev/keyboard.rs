use std::fmt::Debug;
use std::os::fd::AsFd;
use std::{
    collections::{HashMap, HashSet},
    error::Error,
};

use evdev::{Device, EventType, InputEvent};
use nix::fcntl::{FcntlArg, OFlag};

use super::supported_signatures;
use crate::config::capability_map::CapabilityMapConfig;
use crate::{
    config::SourceDevice,
    input::{
        capability::Capability,
        event::{
            evdev::{maps, translator::EventTranslator, EvdevEvent},
            native::NativeEvent,
        },
        source::{EventCapability, InputError, SourceInputDevice, SourceOutputDevice},
    },
    udev::device::UdevDevice,
};

/// Source device implementation for evdev gamepads
pub struct KeyboardEventDevice {
    device: Device,
    translator: Option<EventTranslator>,
    event_filter: HashSet<Capability>,
}

impl KeyboardEventDevice {
    /// Create a new [KeyboardEventDevice] source device from the given udev info
    pub fn new(
        device_info: UdevDevice,
        config: &Option<SourceDevice>,
        capability_map: Option<CapabilityMapConfig>,
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
        let fd = device.as_fd();
        nix::fcntl::fcntl(fd, FcntlArg::F_SETFL(OFlag::O_NONBLOCK))?;

        // Create an event translator if a capability map was given
        let translator = capability_map.map(|map| {
            let mappings = maps::from_maps(None, Some(&map), &supported_signatures(&device));
            EventTranslator::new(&mappings, HashMap::new())
        });

        Ok(Self {
            device,
            translator,
            event_filter: HashSet::new(),
        })
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

    /// Emit a native event unless the on-demand filter excludes it.
    fn emit(&mut self, events: &mut Vec<NativeEvent>, event: NativeEvent) {
        if !self.event_filter.is_empty() && !self.event_filter.contains(&event.as_capability()) {
            return;
        }
        events.push(event);
    }
}

impl SourceInputDevice for KeyboardEventDevice {
    /// Poll the given input device for input events
    fn poll(&mut self) -> Result<Vec<NativeEvent>, InputError> {
        let mut native_events = vec![];

        // Emit any translator held events that are ready
        if let Some(translator) = self.translator.as_mut() {
            for event in translator.drain_deferrals() {
                self.emit(&mut native_events, event);
            }
        }

        // Read events from the device
        let events = {
            let result = self.device.fetch_events();
            let events = match result {
                Ok(events) => events,
                Err(err) => match err.kind() {
                    // Do nothing if this would block
                    std::io::ErrorKind::WouldBlock => return Ok(native_events),
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

        for event in events {
            let translated = self.translator.as_mut().and_then(|t| t.translate(&event));
            if let Some(translated) = translated {
                for event in translated {
                    self.emit(&mut native_events, event);
                }
                continue;
            }
            if let Some(native_event) = self.translate(event) {
                self.emit(&mut native_events, native_event);
            }
        }

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
                        let cap = evdev_event.to_capability();
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

        // Report post-translation capabilities when a translator is active
        if let Some(translator) = &self.translator {
            let raw_kept: HashSet<Capability> = capabilities.iter().cloned().collect();
            capabilities = translator
                .exposed_capabilities(&raw_kept)
                .into_iter()
                .collect();
        }

        Ok(capabilities)
    }

    fn update_event_filter(&mut self, events: HashSet<Capability>) -> Result<(), InputError> {
        self.event_filter = events;
        Ok(())
    }
}

impl SourceOutputDevice for KeyboardEventDevice {}

impl Debug for KeyboardEventDevice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KeyboardEventDevice").finish()
    }
}
