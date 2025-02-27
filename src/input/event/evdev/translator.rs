use std::{
    collections::{HashMap, HashSet},
    error::Error,
    time::Duration,
};

use evdev::{AbsInfo, AbsoluteAxisCode, EventType, InputEvent};

use crate::{
    config::capability_map::{CapabilityMapConfig, CapabilityMappingV2},
    input::{
        capability::Capability,
        composite_device::command::CompositeCommand,
        event::{native::NativeEvent, value::InputValue},
    },
};

/// Event type + event code pair
//type EvdevCapability = (EventType, u16);
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
struct EvdevCapability {
    event_type: EventType,
    event_code: u16,
}

impl From<&InputEvent> for EvdevCapability {
    fn from(value: &InputEvent) -> Self {
        Self {
            event_type: value.event_type(),
            event_code: value.code(),
        }
    }
}

/// Used to translate evdev events into native inputplumber events using a
/// capability map.
pub struct EventTranslator {
    capability_map: EvdevCapabilityMapping,
    axes_info: HashMap<AbsoluteAxisCode, AbsInfo>,
    hat_state: HashMap<AbsoluteAxisCode, i32>,
    /// List of input capabilities that can be translated by the capability map
    translatable_capabilities: HashSet<EvdevCapability>,
    /// List of currently "pressed" actions used to translate multiple input
    /// sequences into a single input event.
    translatable_active_inputs: HashSet<EvdevCapability>,
    /// List of translated events that were emitted less than 8ms ago. This
    /// is required to support "on release" style buttons on some devices where
    /// a button "up" event will fire immediately after a "down" event upon
    /// physical release of the button.
    translated_recent_events: HashSet<EvdevCapability>,
}

impl EventTranslator {
    pub fn translate(&mut self, event: InputEvent) -> Option<NativeEvent> {
        // Add or remove the event from translatable_active_inputs
        let event_capability = (&event).into();
        if event.value() > 0 {
            //
        }
        if self
            .translatable_active_inputs
            .get(&event_capability)
            .is_some()
        {
            //
        }

        None
    }
}

/// Evdev-specific capability mapping
pub struct EvdevCapabilityMap {}

impl From<&CapabilityMapConfig> for EvdevCapabilityMap {
    fn from(value: &CapabilityMapConfig) -> Self {
        todo!()
    }
}

pub struct EvdevCapabilityMapping {
    pub event_type: EventType,
    pub target_capability: Capability,
    // EventSummary
}

impl From<&CapabilityMappingV2> for EvdevCapabilityMapping {
    fn from(value: &CapabilityMappingV2) -> Self {
        let capability = value.target_event.into();

        Self {
            event_type,
            target_capability: capability,
        }
    }
}

impl EvdevCapabilityMapping {
    pub fn matches(&self, event: InputEvent) -> bool {
        match event.destructure() {
            evdev::EventSummary::Synchronization(
                synchronization_event,
                synchronization_code,
                _,
            ) => todo!(),
            evdev::EventSummary::Key(key_event, key_code, _) => todo!(),
            evdev::EventSummary::RelativeAxis(relative_axis_event, relative_axis_code, _) => {
                todo!()
            }
            evdev::EventSummary::AbsoluteAxis(absolute_axis_event, absolute_axis_code, _) => {
                todo!()
            }
            evdev::EventSummary::Misc(misc_event, misc_code, _) => todo!(),
            evdev::EventSummary::Switch(switch_event, switch_code, _) => todo!(),
            evdev::EventSummary::Led(led_event, led_code, _) => todo!(),
            evdev::EventSummary::Sound(sound_event, sound_code, _) => todo!(),
            evdev::EventSummary::Repeat(repeat_event, repeat_code, _) => todo!(),
            evdev::EventSummary::ForceFeedback(ffevent, ffeffect_code, _) => todo!(),
            evdev::EventSummary::Power(power_event, power_code, _) => todo!(),
            evdev::EventSummary::ForceFeedbackStatus(ffstatus_event, ffstatus_code, _) => todo!(),
            evdev::EventSummary::UInput(uinput_event, uinput_code, _) => todo!(),
            evdev::EventSummary::Other(other_event, other_code, _) => todo!(),
        }
    }

    /// Translates the given event into a different event based on the given
    /// [CapabilityMap].
    async fn translate_capability(&mut self, event: &NativeEvent) -> Result<(), Box<dyn Error>> {
        // Get the capability map to translate input events
        let Some(map) = self.capability_map.as_ref() else {
            return Err("Cannot translate device capability without capability map!".into());
        };

        // Add or remove the event from translatable_active_inputs.
        let event_capability = event.as_capability();
        let capability_idx = self
            .translatable_active_inputs
            .iter()
            .position(|c| c == &event_capability);
        if event.pressed() {
            if capability_idx.is_none() {
                log::trace!("Adding capability to active inputs: {:?}", event_capability);
                self.translatable_active_inputs.push(event_capability);
                log::trace!(
                    "Active translatable inputs: {:?}",
                    self.translatable_active_inputs
                );
            } else {
                return Ok(());
            }
        } else if capability_idx.is_some() {
            log::trace!(
                "Removing capability from active inputs: {:?}",
                event_capability
            );
            let idx = capability_idx.unwrap();
            self.translatable_active_inputs.remove(idx);
            log::trace!(
                "Active translatable inputs: {:?}",
                self.translatable_active_inputs
            );
        } else {
            return Ok(());
        }

        // Keep a list of events to emit. The reason for this is some mapped
        // capabilities may use one or more of the same source capability and
        // they would release at the same time.
        let mut emit_queue = Vec::new();

        // Loop over each mapping and try to match source events
        for mapping in map.mapping.iter() {
            // If the event was not pressed and it exists in the emitted_mappings array,
            // then we need to check to see if ALL of its events no longer exist in
            // translatable_active_inputs.
            if !event.pressed() && self.emitted_mappings.contains_key(&mapping.name) {
                let mut has_source_event_pressed = false;

                // Loop through each source capability in the mapping
                for source_event in mapping.source_events.iter() {
                    let cap = source_event.clone().into();
                    if cap == Capability::NotImplemented {
                        continue;
                    }
                    if self.translatable_active_inputs.contains(&cap) {
                        has_source_event_pressed = true;
                        break;
                    }
                }

                // If no more inputs are being pressed, send a release event.
                if !has_source_event_pressed {
                    let cap = mapping.target_event.clone().into();
                    if cap == Capability::NotImplemented {
                        continue;
                    }
                    let event = NativeEvent::new(cap, InputValue::Bool(false));
                    log::trace!("Adding event to emit queue: {:?}", event);
                    emit_queue.push(event);
                    self.emitted_mappings.remove(&mapping.name);
                }
            }

            // If the event is pressed, check for any matches to send a 'press' event
            if event.pressed() {
                let mut is_missing_source_event = false;
                for source_event in mapping.source_events.iter() {
                    let cap = source_event.clone().into();
                    if cap == Capability::NotImplemented {
                        continue;
                    }
                    if !self.translatable_active_inputs.contains(&cap) {
                        is_missing_source_event = true;
                        break;
                    }
                }

                if !is_missing_source_event {
                    let cap = mapping.target_event.clone().into();
                    if cap == Capability::NotImplemented {
                        continue;
                    }
                    let event = NativeEvent::new(cap, InputValue::Bool(true));
                    log::trace!("Adding event to emit queue: {:?}", event);
                    emit_queue.push(event);
                    self.emitted_mappings
                        .insert(mapping.name.clone(), mapping.clone());
                }
            }
        }

        // Emit the translated events. If this translated event has been emitted
        // very recently, delay sending subsequent events of the same type.
        let sleep_time = Duration::from_millis(4);
        for event in emit_queue {
            // Check to see if the event is in recently translated.
            // If it is, spawn a task to delay emit the event.
            let cap = event.as_capability();
            if self.translated_recent_events.contains(&cap) {
                log::debug!("Event emitted too quickly. Delaying emission.");
                let tx = self.tx.clone();
                tokio::task::spawn(async move {
                    tokio::time::sleep(sleep_time).await;
                    if let Err(e) = tx.send(CompositeCommand::HandleEvent(event)).await {
                        log::error!("Failed to send delayed event command: {:?}", e);
                    }
                });

                continue;
            }

            // Add the event to our list of recently device translated events
            self.translated_recent_events.insert(event.as_capability());

            // Spawn a task to remove the event from recent translated
            let tx = self.tx.clone();
            tokio::task::spawn(async move {
                tokio::time::sleep(sleep_time).await;
                if let Err(e) = tx.send(CompositeCommand::RemoveRecentEvent(cap)).await {
                    log::error!("Failed to send remove recent event command: {:?}", e);
                }
            });

            log::trace!("Emitting event: {:?}", event);
            self.handle_event(event).await?;
        }

        Ok(())
    }
}
