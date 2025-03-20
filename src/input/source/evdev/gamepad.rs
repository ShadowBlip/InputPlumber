use std::fmt::Debug;
use std::{collections::HashMap, error::Error, os::fd::AsRawFd};

use evdev::{
    AbsInfo, AbsoluteAxisCode, Device, EventType, FFEffect, FFEffectData, FFEffectKind, FFReplay,
    FFTrigger, InputEvent,
};
use nix::fcntl::{FcntlArg, OFlag};
use packed_struct::types::SizedInteger;
use packed_struct::PrimitiveEnum;

use crate::config::capability_map::CapabilityMapConfigV2;
use crate::drivers::steam_deck::hid_report::{
    CommandType, PackedHapticReport, PackedRumbleReport, PadSide,
};
use crate::input::event::evdev::translator::EventTranslator;
use crate::{
    drivers::dualsense::hid_report::SetStatePackedOutputData,
    input::{
        capability::{Capability, Gamepad, GamepadAxis, GamepadButton},
        event::{evdev::EvdevEvent, native::NativeEvent},
        output_event::OutputEvent,
        source::{InputError, OutputError, SourceInputDevice, SourceOutputDevice},
    },
    udev::device::UdevDevice,
};

/// Source device implementation for evdev gamepads
pub struct GamepadEventDevice {
    device: Device,
    axes_info: HashMap<AbsoluteAxisCode, AbsInfo>,
    translator: Option<EventTranslator>,
    ff_effects: HashMap<i16, FFEffect>,
    ff_effects_dualsense: Option<i16>,
    ff_effects_deck: Option<i16>,
    hat_state: HashMap<AbsoluteAxisCode, i32>,
}

impl GamepadEventDevice {
    /// Create a new [Gamepad] source device from the given udev info
    pub fn new(
        device_info: UdevDevice,
        capability_map: Option<CapabilityMapConfigV2>,
    ) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let path = device_info.devnode();
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

        // Create an event translator if a capability map was given
        let translator = capability_map.map(|map| EventTranslator::new(&map, axes_info.clone()));

        Ok(Self {
            device,
            axes_info,
            translator,
            ff_effects: HashMap::new(),
            ff_effects_dualsense: None,
            ff_effects_deck: None,
            hat_state: HashMap::new(),
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

        // If this is an ABS event, get the min/max info for this type of
        // event so we can normalize the value.
        let abs_info = if event.event_type() == EventType::ABSOLUTE {
            self.axes_info.get(&AbsoluteAxisCode(event.code()))
        } else {
            None
        };

        let state = if event.event_type() == EventType::ABSOLUTE {
            let axis = AbsoluteAxisCode(event.code());

            let state = match axis {
                AbsoluteAxisCode::ABS_HAT0X | AbsoluteAxisCode::ABS_HAT0Y => {
                    let value = event.value();
                    let last_value = *self.hat_state.get(&axis).unwrap_or(&0);
                    self.hat_state
                        .entry(axis)
                        .and_modify(|v| *v = value)
                        .or_insert(value);
                    Some(last_value)
                }
                _ => None,
            };
            state
        } else {
            None
        };

        // Convert the event into an [EvdevEvent] and optionally include
        // the axis information with min/max values
        let mut evdev_event: EvdevEvent = event.into();
        if let Some(info) = abs_info {
            evdev_event.set_abs_info(*info);
        }

        // Convert the event into a [NativeEvent]
        let native_event: NativeEvent = NativeEvent::from_evdev_raw(evdev_event, state);

        Some(native_event)
    }

    /// Process dualsense force feedback output reports
    fn process_dualsense_ff(
        &mut self,
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
            log::trace!("Uploading FF effect data");
            let effect = self.device.upload_ff_effect(effect_data)?;
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

    // Process Steam Deck FFB events.
    fn process_deck_ff(&mut self, report: PackedRumbleReport) -> Result<(), Box<dyn Error>> {
        // If no effect was uploaded to handle Steam Deck force feedback, upload one.
        if self.ff_effects_deck.is_none() {
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
                    strong_magnitude: 0,
                    weak_magnitude: 0,
                },
            };
            log::trace!("Uploading FF effect data");
            let effect = self.device.upload_ff_effect(effect_data)?;
            let id = effect.id() as i16;
            self.ff_effects.insert(id, effect);
            self.ff_effects_deck = Some(id);
        }

        let effect_id = self.ff_effects_deck.unwrap();
        let effect = self.ff_effects.get_mut(&effect_id).unwrap();

        let left_speed = report.left_speed.to_primitive();
        let right_speed = report.right_speed.to_primitive();

        log::trace!("Got FF event data, Left Speed: {left_speed}, Right Speed: {right_speed}");

        // Stop playing the effect if values are set to zero
        if left_speed == 0 && right_speed == 0 {
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
                strong_magnitude: left_speed,
                weak_magnitude: right_speed,
            },
        };
        log::trace!("Updating effect data");
        effect.update(effect_data)?;
        log::trace!("Playing effect with data: {:?}", effect_data);
        effect.play(1)?;

        Ok(())
    }

    // Process Steam Deck Haptic events.
    fn process_haptic_ff(&mut self, report: PackedHapticReport) -> Result<(), Box<dyn Error>> {
        // If no effect was uploaded to handle Steam Deck force feedback, upload one.
        if self.ff_effects_deck.is_none() {
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
                    strong_magnitude: 0,
                    weak_magnitude: 0,
                },
            };
            log::trace!("Uploading FF effect data");
            let effect = self.device.upload_ff_effect(effect_data)?;
            let id = effect.id() as i16;
            self.ff_effects.insert(id, effect);
            self.ff_effects_deck = Some(id);
        }

        let effect_id = self.ff_effects_deck.unwrap();
        let effect = self.ff_effects.get_mut(&effect_id).unwrap();

        let intensity = report.intensity.to_primitive() + 1;
        let scaled_gain = (report.gain + 24) as u8 * intensity;
        let normalized_gain = normalize_unsigned_value(scaled_gain as f64, 150.0);
        let new_gain = normalized_gain * u16::MAX as f64;
        let new_gain = new_gain as u16;

        let left_speed = match report.side {
            PadSide::Left => new_gain,
            PadSide::Right => 0,
            PadSide::Both => new_gain,
        };

        let left_speed = match report.cmd_type {
            CommandType::Off => 0,
            CommandType::Tick => left_speed,
            CommandType::Click => left_speed,
        };

        let right_speed = match report.side {
            PadSide::Left => 0,
            PadSide::Right => new_gain,
            PadSide::Both => new_gain,
        };

        let right_speed = match report.cmd_type {
            CommandType::Off => 0,
            CommandType::Tick => right_speed,
            CommandType::Click => right_speed,
        };

        let length = match report.cmd_type {
            CommandType::Off => 0,
            CommandType::Tick => 50,
            CommandType::Click => 150,
        };

        log::trace!("Got FF event data, Left Speed: {left_speed}, Right Speed: {right_speed}, Length: {length}");

        // Stop playing the effect if values are set to zero
        if left_speed == 0 && right_speed == 0 {
            log::trace!("Stopping FF effect");
            effect.stop()?;
            return Ok(());
        }

        // Set the values of the effect and play it
        let effect_data = FFEffectData {
            direction: 0,
            trigger: FFTrigger {
                button: 0,
                interval: 25,
            },
            replay: FFReplay { length, delay: 0 },
            kind: FFEffectKind::Rumble {
                strong_magnitude: left_speed,
                weak_magnitude: right_speed,
            },
        };
        log::trace!("Updating effect data");
        effect.update(effect_data)?;
        log::trace!("Playing effect with data: {:?}", effect_data);
        effect.play(1)?;

        Ok(())
    }
}

impl SourceInputDevice for GamepadEventDevice {
    /// Poll the given input device for input events
    fn poll(&mut self) -> Result<Vec<NativeEvent>, InputError> {
        let mut native_events = vec![];

        // Poll the translator for any scheduled events
        if let Some(translator) = self.translator.as_mut() {
            native_events.extend(translator.poll());
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

        // Convert the events into native events if no translator exists
        if self.translator.is_none() {
            let translated_events: Vec<NativeEvent> = events
                .into_iter()
                .filter_map(|e| self.translate(e))
                .collect();
            native_events.extend(translated_events);
            return Ok(native_events);
        }

        // Create a list of events that the translator can't translate
        let mut untranslated_events = vec![];

        // Convert the events into native events with the translator
        {
            let Some(translator) = self.translator.as_mut() else {
                return Ok(native_events);
            };

            for event in events {
                if translator.has_translation(&event) {
                    native_events.extend(translator.translate(&event));
                } else {
                    untranslated_events.push(event);
                }
            }
        }

        // Translate any untranslated events using the legacy method
        let translated_events: Vec<NativeEvent> = untranslated_events
            .into_iter()
            .filter_map(|e| self.translate(e))
            .collect();
        native_events.extend(translated_events);

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
                EventType::RELATIVE => {
                    let Some(rel) = self.device.supported_relative_axes() else {
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
                    let Some(abs) = self.device.supported_absolute_axes() else {
                        continue;
                    };
                    for axis in abs.iter() {
                        let input_event = InputEvent::new(event.0, axis.0, 0);
                        let evdev_event = EvdevEvent::from(input_event);
                        let cap = evdev_event.as_capability();
                        if cap == Capability::Gamepad(Gamepad::Axis(GamepadAxis::Hat0)) {
                            capabilities
                                .push(Capability::Gamepad(Gamepad::Button(GamepadButton::DPadUp)));
                            capabilities.push(Capability::Gamepad(Gamepad::Button(
                                GamepadButton::DPadDown,
                            )));
                            capabilities.push(Capability::Gamepad(Gamepad::Button(
                                GamepadButton::DPadLeft,
                            )));
                            capabilities.push(Capability::Gamepad(Gamepad::Button(
                                GamepadButton::DPadRight,
                            )));
                            continue;
                        }
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
}

impl SourceOutputDevice for GamepadEventDevice {
    /// Write the given output event to the source device. Output events are
    /// events that flow from an application (like a game) to the physical
    /// input device, such as force feedback events.
    fn write_event(&mut self, event: OutputEvent) -> Result<(), OutputError> {
        log::trace!("Received output event: {:?}", event);

        // Only process output events if FF is supported
        let force_feedback = self.device.supported_ff();
        if force_feedback.is_none() {
            log::trace!("Device does not support FF events");
            return Ok(());
        }
        if let Some(ff) = force_feedback {
            if ff.iter().count() == 0 {
                log::trace!("Device has no FF support");
                return Ok(());
            }
        }

        match event {
            OutputEvent::Evdev(input_event) => {
                if let Err(e) = self.device.send_events(&[input_event]) {
                    log::error!("Failed to write output event: {:?}", e);
                }
                Ok(())
            }
            OutputEvent::DualSense(report) => {
                log::trace!("Received DualSense output report");
                if report.use_rumble_not_haptics || report.enable_improved_rumble_emulation {
                    if let Err(e) = self.process_dualsense_ff(report) {
                        log::error!("Failed to process dualsense output report: {e:?}");
                    }
                }
                Ok(())
            }
            OutputEvent::Uinput(_) => Ok(()),
            OutputEvent::SteamDeckHaptics(report) => {
                log::trace!("Received Steam Deck Haptic Output Report");
                if let Err(e) = self.process_haptic_ff(report) {
                    log::error!("Failed to process Steam Deck Haptic Output Report: {e:?}")
                }
                Ok(())
            }
            OutputEvent::SteamDeckRumble(report) => {
                log::trace!("Received Steam Deck Force Feedback Report");
                if let Err(e) = self.process_deck_ff(report) {
                    log::error!("Failed to process Steam Deck Force Feedback Report: {e:?}")
                }
                Ok(())
            }
        }
    }

    /// Upload the given force feedback effect data to the source device. Returns
    /// a device-specific id of the uploaded effect if it is successful.
    fn upload_effect(&mut self, effect: FFEffectData) -> Result<i16, OutputError> {
        log::trace!("Uploading FF effect data");
        if self.device.supported_ff().is_none() {
            log::debug!("Device does not support FF effects");
            return Ok(-1);
        }
        match self.device.upload_ff_effect(effect) {
            Ok(effect) => {
                let id = effect.id() as i16;
                self.ff_effects.insert(id, effect);
                Ok(id)
            }
            Err(e) => Err(OutputError::DeviceError(e.to_string())),
        }
    }

    /// Update the effect with the given id using the given effect data.
    fn update_effect(&mut self, effect_id: i16, effect: FFEffectData) -> Result<(), OutputError> {
        log::trace!("Update FF effect {effect_id}");
        if self.device.supported_ff().is_none() {
            log::debug!("Device does not support FF effects");
            return Ok(());
        }
        let Some(current_effect) = self.ff_effects.get_mut(&effect_id) else {
            log::warn!("Unable to find existing FF effect with id {effect_id}");
            return Ok(());
        };

        if let Err(e) = current_effect.update(effect) {
            log::warn!("Failed to update effect with id {effect_id}: {:?}", e);
        }

        Ok(())
    }

    /// Erase the effect with the given id from the source device.
    fn erase_effect(&mut self, effect_id: i16) -> Result<(), OutputError> {
        log::trace!("Erasing FF effect data");
        if self.device.supported_ff().is_none() {
            log::debug!("Device does not support FF effects");
            return Ok(());
        }
        self.ff_effects.remove(&effect_id);
        Ok(())
    }
}

impl Debug for GamepadEventDevice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GamepadEventDevice")
            .field("axes_info", &self.axes_info)
            .field("ff_effects", &self.ff_effects)
            .field("ff_effects_dualsense", &self.ff_effects_dualsense)
            .field("hat_state", &self.hat_state)
            .finish()
    }
}

// Returns a value between 0.0 and 1.0 based on the given value with its
// maximum.
fn normalize_unsigned_value(raw_value: f64, max: f64) -> f64 {
    raw_value / max
}
