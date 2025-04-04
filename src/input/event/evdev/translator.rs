use std::{
    collections::{hash_map::Entry, HashMap, HashSet},
    time::Duration,
};

use evdev::{AbsInfo, InputEvent};

use crate::{
    config::capability_map::{
        evdev::{AxisDirection, ValueType},
        CapabilityMapConfigV2, CapabilityMapping, EvdevMappingType,
    },
    input::{
        capability::Capability,
        event::{
            native::{NativeEvent, ScheduledNativeEvent},
            value::InputValue,
        },
    },
};

use super::{normalize_signed_value, normalize_unsigned_value};

type EventType = u16;
type EventCode = u16;
type EventSignature = (EventType, EventCode);

/// Used to translate evdev events into native inputplumber events using a
/// capability map.
#[derive(Debug)]
pub struct EventTranslator {
    /// Information (i.e. min/max values) about absolute axis events so events
    /// can be normalized.
    abs_info: HashMap<evdev::AbsoluteAxisCode, AbsInfo>,
    /// Lookup table for looking up chord mappings for a given event signature.
    chord_mappings: HashMap<EventSignature, Vec<CapabilityMapping>>,
    /// Lookup table for looking up delayed chord mappings for a given event signature.
    delayed_chord_mappings: HashMap<EventSignature, CapabilityMapping>,
    /// Lookup table for looking up multi source mappings for a given event signature.
    multi_source_mappings: HashMap<EventSignature, Vec<CapabilityMapping>>,
    /// Lookup table for looking up capability mappings for a given event signature.
    translatable_events: HashSet<EventSignature>,
    /// List of currently "pressed" events used to translate multiple input
    /// sequences into a single input event.
    translatable_active_inputs: HashSet<EventSignature>,
    /// List of used "pressed" events used to translate multiple input sequences
    /// into a single input event.
    translatable_active_inputs_used: HashSet<EventSignature>,
    /// Mappings that have been emitted by a chord that have not been released
    emitted_mappings: Vec<CapabilityMapping>,
    /// Events to emit during polling
    scheduled_events: Vec<ScheduledNativeEvent>,
}

impl EventTranslator {
    /// Creates a new translator for the given capability map.
    pub fn new(
        capability_map: &CapabilityMapConfigV2,
        abs_info: HashMap<evdev::AbsoluteAxisCode, AbsInfo>,
    ) -> Self {
        // Build lookup tables to look up mappings based on the event signature.
        let mut translatable_events = HashSet::with_capacity(capability_map.mapping.len());
        let mut chord_mappings: HashMap<EventSignature, Vec<CapabilityMapping>> = HashMap::new();
        let mut delayed_chord_mappings: HashMap<EventSignature, CapabilityMapping> = HashMap::new();
        let mut multi_source_mappings: HashMap<EventSignature, Vec<CapabilityMapping>> =
            HashMap::new();

        // Sort events to translate.
        for mapping in capability_map.mapping.iter() {
            // Get the type of mapping. Defaults to single source mapping.
            let mapping_type = mapping
                .mapping_type
                .clone()
                .and_then(|t| t.evdev)
                .unwrap_or_default();

            // Only consider evdev mappings
            for event in mapping.source_events.iter() {
                let Some(evdev_event) = event.evdev.as_ref() else {
                    log::warn!("Non-evdev source mapping not supported on evdev device: {event:?}");
                    continue;
                };
                let kind = evdev_event.event_type.as_u16();
                let code = evdev_event.event_code.as_u16();
                let event_signature = (kind, code);

                translatable_events.insert(event_signature);

                match mapping_type {
                    EvdevMappingType::Chord => {
                        chord_mappings
                            .entry(event_signature)
                            .and_modify(|m| m.push(mapping.clone()))
                            .or_insert(vec![mapping.clone()]);
                    }
                    EvdevMappingType::DelayedChord => {
                        if let Entry::Vacant(e) = delayed_chord_mappings.entry(event_signature) {
                            e.insert(mapping.clone());
                        } else {
                            log::warn!(
                                "A delayed mapping already exists for event ({:?} {:?}). Skipping.",
                                evdev_event.event_type,
                                evdev_event.event_code
                            );
                        }
                    }
                    EvdevMappingType::MultiSource => {
                        // If multiple source events exist, split into multiple
                        // mappings with one source event.
                        for source_event in mapping.source_events.iter() {
                            let Some(evdev_event) = source_event.evdev.as_ref() else {
                                log::warn!("Non-evdev source mapping not supported on evdev device: {event:?}");
                                continue;
                            };
                            let kind = evdev_event.event_type.as_u16();
                            let code = evdev_event.event_code.as_u16();
                            let mapping_signature = (kind, code);

                            // Skip source events that don't have matching signatures
                            // to prevent duplicate entries
                            if event_signature != mapping_signature {
                                continue;
                            }

                            let mut source_event_mapping = mapping.clone();
                            source_event_mapping.source_events = vec![source_event.clone()];
                            multi_source_mappings
                                .entry(mapping_signature)
                                .and_modify(|m| m.push(source_event_mapping.clone()))
                                .or_insert(vec![source_event_mapping]);
                        }
                    }
                }
            }
        }

        Self {
            abs_info,
            chord_mappings,
            emitted_mappings: Vec::new(),
            delayed_chord_mappings,
            multi_source_mappings,
            translatable_events,
            translatable_active_inputs: HashSet::new(),
            translatable_active_inputs_used: HashSet::new(),
            scheduled_events: Vec::new(),
        }
    }

    /// Returns true if the capability map associated with the translator has
    /// a translation defined for the given event.
    pub fn has_translation(&self, event: &InputEvent) -> bool {
        let event_signature = (event.event_type().0, event.code());
        self.translatable_events.contains(&event_signature)
    }

    /// Poll for scheduled events
    pub fn poll(&mut self) -> Vec<NativeEvent> {
        let mut ready_events = vec![];
        let mut not_ready_events = vec![];
        for event in self.scheduled_events.drain(..) {
            log::debug!("Found scheduled event: {event:?}");
            if event.is_ready() {
                log::debug!("Event is ready!");
                ready_events.push(event.into());
                continue;
            }
            not_ready_events.push(event);
        }
        self.scheduled_events = not_ready_events;

        if !ready_events.is_empty() {
            log::debug!("Ready events: {ready_events:?}");
        }

        ready_events
    }

    /// Translate the given event into a native inputplumber event according to
    /// the capability map.
    pub fn translate(&mut self, event: &InputEvent) -> Vec<NativeEvent> {
        // Get the event signature for the event
        let event_signature = (event.event_type().0, event.code());
        if !self.translatable_events.contains(&event_signature) {
            return vec![];
        }

        // Check if this event is part of a chord mapping
        if self.delayed_chord_mappings.contains_key(&event_signature)
            && !event.is_pressed()
            && self.translatable_active_inputs.contains(&event_signature)
        {
            // Fall through so we can potentially match against normal chord
            // mappings if no translation if found.
            if let Some(native_event) = self.translate_delayed_chord(event) {
                return vec![native_event];
            }
        }
        if self.chord_mappings.contains_key(&event_signature) {
            return self
                .translate_chord(event)
                .map(|e| vec![e])
                .unwrap_or_default();
        }

        // Check if this event is part of a multi_source mapping
        if self.multi_source_mappings.contains_key(&event_signature) {
            return self.translate_multi_source(event);
        }

        vec![]
    }

    /// Translate the given evdev event into a single native inputplumber event
    fn translate_multi_source(&mut self, event: &InputEvent) -> Vec<NativeEvent> {
        let event_signature = (event.event_type().0, event.code());
        let Some(mappings) = self.multi_source_mappings.get(&event_signature) else {
            return vec![];
        };

        let mut events = vec![];
        for mapping in mappings.iter() {
            let cap: Capability = mapping.target_event.clone().into();
            if cap == Capability::NotImplemented {
                continue;
            }
            let Some(source_event) = mapping.source_events.first() else {
                continue;
            };
            let Some(evdev_config) = source_event.evdev.as_ref() else {
                continue;
            };

            // If an axis direction is defined, only translate if the value
            // matches the axis direction.
            if let Some(direction) = evdev_config.axis_direction.as_ref() {
                let raw_value = event.value();
                match direction {
                    AxisDirection::None => (),
                    AxisDirection::Positive => {
                        if raw_value < 0 {
                            continue;
                        }
                    }
                    AxisDirection::Negative => {
                        if raw_value > 0 {
                            continue;
                        }
                    }
                }
            }

            // Normalize the input value
            let value = self.get_input_value(event, &evdev_config.value_type);
            log::trace!(
                "Translated value {:?} {} to {:?}",
                event.code(),
                event.value(),
                value
            );

            let event = NativeEvent::new(cap, value);
            events.push(event);
        }

        events
    }

    fn translate_chord(&mut self, event: &InputEvent) -> Option<NativeEvent> {
        let event_signature = (event.event_type().0, event.code());

        // Add or remove the event from translatable_active_inputs to keep track
        // of the state of translatable inputs.
        let is_pressed = event.is_pressed();
        let is_active_event = self.translatable_active_inputs.contains(&event_signature);
        if is_pressed && is_active_event {
            return None;
        }
        self.update_active_input_state(event_signature, is_pressed, is_active_event);

        // If the event is "pressed", check the mapping to see if ALL source
        // events have been "pressed". If they are, then the event combo
        // matches and should be translated.
        if is_pressed {
            // Loop over each mapping for this event and perform translation logic.
            let mappings = self.chord_mappings.get(&event_signature)?;
            for mapping in mappings.iter() {
                // Check to see if all source events in the mapping have been
                // "pressed".
                let mut is_missing_source_event = false;
                let mut source_event_signatures = Vec::with_capacity(mapping.source_events.len());
                for source_event in mapping.source_events.iter() {
                    // Only `Evdev` -> `Capability` mapping is supported here
                    let Some(evdev_config) = source_event.evdev.as_ref() else {
                        continue;
                    };
                    let kind = evdev_config.event_type.as_u16();
                    let code = evdev_config.event_code.as_u16();
                    let mapping_signature = (kind, code);
                    source_event_signatures.push(mapping_signature);

                    if !self.translatable_active_inputs.contains(&mapping_signature) {
                        is_missing_source_event = true;
                        break;
                    }
                }

                // If the mapping is missing part of the combo, do nothing.
                if is_missing_source_event {
                    continue;
                }

                // If all source events in the mapping have been pressed, then
                // return the translated event.
                let cap = mapping.target_event.clone().into();
                if cap == Capability::NotImplemented {
                    continue;
                }

                // Move all the events in the mapping from active inputs
                // to used active inputs.
                for source_event in source_event_signatures {
                    self.translatable_active_inputs.remove(&source_event);
                    self.translatable_active_inputs_used.insert(source_event);
                }

                self.emitted_mappings.push(mapping.clone());

                // NOTE: assuming buttons only here
                let event = NativeEvent::new(cap, InputValue::Bool(true));
                return Some(event);
            }

            return None;
        }

        // Remove the event from used active inputs on release
        self.translatable_active_inputs_used
            .remove(&event_signature);

        // Loop over all emitted mappings and see if a release event needs
        // to be emitted.
        'outer: for mapping in self.emitted_mappings.clone().iter() {
            for source_event in mapping.source_events.iter() {
                // Only `Evdev` -> `Capability` mapping is supported here
                let Some(evdev_config) = source_event.evdev.as_ref() else {
                    continue 'outer;
                };

                let kind = evdev_config.event_type.as_u16();
                let code = evdev_config.event_code.as_u16();
                let mapping_signature = (kind, code);

                // If this mapping still has used active inputs, then this
                // mapping is still active and should not send a release event.
                if self
                    .translatable_active_inputs_used
                    .contains(&mapping_signature)
                {
                    continue 'outer;
                }
            }

            // Remove the mapping from emitted mappings
            self.emitted_mappings = self
                .emitted_mappings
                .drain(..)
                .filter(|m| m != mapping)
                .collect();

            // If nothing matched, we matched
            let cap = mapping.target_event.clone().into();
            let event = NativeEvent::new(cap, InputValue::Bool(false));
            return Some(event);
        }

        None
    }

    fn translate_delayed_chord(&mut self, event: &InputEvent) -> Option<NativeEvent> {
        let event_signature = (event.event_type().0, event.code());
        let mapping = self.delayed_chord_mappings.get(&event_signature)?;

        // Emit the down event and queue the up event for next poll
        let cap: Capability = mapping.target_event.clone().into();
        if cap == Capability::NotImplemented {
            return None;
        }
        // NOTE: assuming buttons only here
        let event = NativeEvent::new(cap.clone(), InputValue::Bool(true));

        // Clear any event signatures in translatable_active_inputs that match
        // this mapping.
        for source_event in mapping.source_events.iter() {
            // Only `Evdev` -> `Capability` mapping is supported here
            let Some(evdev_config) = source_event.evdev.as_ref() else {
                continue;
            };

            let kind = evdev_config.event_type.as_u16();
            let code = evdev_config.event_code.as_u16();
            let mapping_signature = (kind, code);

            self.translatable_active_inputs.remove(&mapping_signature);
        }

        // Schedule the release event
        let release_event = NativeEvent::new(cap, InputValue::Bool(false));
        let release_event = ScheduledNativeEvent::new(release_event, Duration::from_millis(100));
        self.scheduled_events.push(release_event);

        Some(event)
    }

    // Add or remove the event from translatable_active_inputs to keep track
    // of the state of translatable inputs.
    fn update_active_input_state(
        &mut self,
        event_signature: EventSignature,
        is_pressed: bool,
        is_active_event: bool,
    ) {
        // Only keep track of key/button events
        if event_signature.0 != evdev::EventType::KEY.0 {
            return;
        }
        if is_pressed {
            if is_active_event {
                return;
            }
            log::trace!("Adding event to active inputs: {event_signature:?}");
            self.translatable_active_inputs.insert(event_signature);
            log::trace!(
                "Active translatable inputs: {:?}",
                self.translatable_active_inputs
            );
            return;
        }
        if is_active_event {
            log::trace!("Removing event from active inputs: {event_signature:?}");
            self.translatable_active_inputs.remove(&event_signature);
            log::trace!(
                "Active translatable inputs: {:?}",
                self.translatable_active_inputs
            );
        }
    }

    /// Returns the normalized value of the event expressed as an [InputValue].
    fn get_input_value(&self, event: &InputEvent, value_type: &ValueType) -> InputValue {
        let normal_value = self.get_normalized_value(event, value_type);

        match value_type {
            ValueType::Button => {
                if normal_value.abs() > 0.5 {
                    InputValue::Bool(true)
                } else {
                    InputValue::Bool(false)
                }
            }
            ValueType::Trigger => InputValue::Float(normal_value),
            ValueType::JoystickX => InputValue::Vector2 {
                x: Some(normal_value),
                y: None,
            },
            ValueType::JoystickY => InputValue::Vector2 {
                x: None,
                y: Some(normal_value),
            },
            ValueType::ImuX => InputValue::Vector3 {
                x: Some(normal_value),
                y: None,
                z: None,
            },
            ValueType::ImuY => InputValue::Vector3 {
                x: None,
                y: Some(normal_value),
                z: None,
            },
            ValueType::ImuZ => InputValue::Vector3 {
                x: None,
                y: None,
                z: Some(normal_value),
            },
        }
    }

    /// Returns the normalized value of the event. This will be a value that
    /// ranges from -1.0 to 1.0 or 0.0 to 1.0 based on the minimum and maximum values.
    pub fn get_normalized_value(&self, event: &InputEvent, value_type: &ValueType) -> f64 {
        let raw_value = event.value();
        if event.event_type() != evdev::EventType::ABSOLUTE {
            return raw_value as f64;
        }
        let code = evdev::AbsoluteAxisCode(event.code());

        // If this event has ABS info, normalize the value
        let Some(info) = self.abs_info.get(&code) else {
            log::trace!("Unable to find ABS info for event: {code:?}");
            return raw_value as f64;
        };

        // TODO: Find a better way to correctly scale gyro values
        const IMU_SCALE: f64 = 0.01;

        match value_type {
            ValueType::Button => normalize_unsigned_value(raw_value, info.maximum()),
            ValueType::Trigger => normalize_unsigned_value(raw_value, info.maximum()),
            ValueType::JoystickX | ValueType::JoystickY => {
                normalize_signed_value(raw_value, info.minimum(), info.maximum())
            }
            ValueType::ImuX | ValueType::ImuY | ValueType::ImuZ => (raw_value as f64) * IMU_SCALE,
        }
    }
}

trait Pressable {
    fn is_pressed(&self) -> bool;
}

impl Pressable for InputEvent {
    fn is_pressed(&self) -> bool {
        self.value() != 0
    }
}
