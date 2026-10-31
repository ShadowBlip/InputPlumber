use std::{
    collections::{HashMap, HashSet},
    f64::consts::PI,
    time::{Duration, Instant},
};

use evdev::{AbsInfo, AbsoluteAxisCode, EventType, InputEvent};

use crate::{
    config::capability_map::evdev::{AxisDirection, ValueType},
    input::{
        capability::Capability,
        event::{
            evdev::{
                maps::{MappedTranslation, MappingKind},
                normalize_signed_value, normalize_unsigned_value, EvdevEvent, EventSignature,
            },
            native::{NativeEvent, ScheduledNativeEvent},
            value::InputValue,
        },
    },
};

/// Minimum press-to-release interval; a release arriving within it is held
/// until the interval has elapsed.
pub const MIN_EVENT_TIME: Duration = Duration::from_millis(34);

/// Grace window after a deferrable chord completes; a strict superset chord
/// completing within it cancels the press, otherwise the chord fires.
const CHORD_DEFER_DELAY: Duration = Duration::from_millis(20);

/// Accelerometer counts per g, used when the kernel leaves resolution unset.
const ACCELEROMETER_FALLBACK_RES: f64 = 8192.0;

/// Gyroscope counts per degree/second, used when the kernel leaves resolution unset.
const GYROSCOPE_FALLBACK_RES: f64 = 1024.0;

/// Standard gravity in m/s^2.
const GRAVITY: f64 = 9.80665;

/// Translates raw evdev events through the loaded capability mappings.
#[derive(Debug)]
pub struct EventTranslator {
    /// Information (i.e. min/max values) about absolute axis events so events
    /// can be normalized.
    abs_info: HashMap<AbsoluteAxisCode, AbsInfo>,
    /// List of currently "pressed" events used to translate multiple input
    /// sequences into a single input event.
    active: HashSet<EventSignature>,
    /// Lookup table for looking up chord mappings for a given event signature.
    chords: HashMap<EventSignature, Vec<usize>>,
    /// Mappings that have been emitted by a chord that have not been released
    held: HashSet<usize>,
    mappings: Vec<MappedTranslation>,
    /// Lookup table for looking up multi source mappings for a given event signature.
    multi_source: HashMap<EventSignature, Vec<usize>>,
    /// Deferred chord presses: (mapping index, deadline).
    pending: Vec<(usize, Instant)>,
    /// Last emission per capability, for the debounce window.
    recent: HashMap<Capability, Instant>,
    /// Events to emit during polling
    scheduled: HashMap<Capability, ScheduledNativeEvent>,
    /// Lookup table for looking up capability mappings for a given event signature.
    translatable: HashSet<EventSignature>,
}

impl EventTranslator {
    pub fn new(
        mappings: &[MappedTranslation],
        abs_info: HashMap<AbsoluteAxisCode, AbsInfo>,
    ) -> Self {
        let mappings = mappings.to_vec();
        let mut chords: HashMap<EventSignature, Vec<usize>> = HashMap::new();
        let mut multi_source: HashMap<EventSignature, Vec<usize>> = HashMap::new();
        let mut translatable: HashSet<EventSignature> = HashSet::new();

        for (index, mapping) in mappings.iter().enumerate() {
            for (signatures, _) in mapping.source_events.iter() {
                for signature in signatures.iter() {
                    translatable.insert(*signature);
                    match mapping.kind {
                        MappingKind::Chord => {
                            chords.entry(*signature).or_default().push(index);
                        }
                        MappingKind::MultiSource => {
                            multi_source.entry(*signature).or_default().push(index);
                        }
                    }
                }
            }
        }

        Self {
            abs_info,
            active: HashSet::new(),
            chords,
            held: HashSet::new(),
            mappings,
            multi_source,
            pending: Vec::new(),
            recent: HashMap::new(),
            scheduled: HashMap::new(),
            translatable,
        }
    }

    /// Translate one raw evdev event through the debounce. None when no
    /// mapping consumes the event's signature.
    pub fn translate(&mut self, event: &InputEvent) -> Option<Vec<NativeEvent>> {
        let signature = EvdevEvent::from(*event).signature();
        let translatable = self.translatable.contains(&signature);
        log::trace!(
            "translate: event_type={:?} code={} value={} translatable={}",
            event.event_type(),
            event.code(),
            event.value(),
            translatable
        );
        if !translatable {
            return None;
        }

        let mut out = Vec::new();
        if event.value() == 0 {
            self.release(signature, event, &mut out);
        } else {
            self.press(signature, event, &mut out);
        }
        Some(out)
    }

    /// Collect the events that are ready to emit: deferred chord presses
    /// past their grace window and debounced releases past their hold.
    /// They are emitted as-is; no translation or debounce is applied again.
    pub fn drain_deferrals(&mut self) -> Vec<NativeEvent> {
        let mut out = Vec::new();

        if !self.pending.is_empty() || !self.scheduled.is_empty() {
            log::trace!(
                "drain_deferrals: pending={} scheduled={}",
                self.pending.len(),
                self.scheduled.len()
            );
        }

        let now = Instant::now();
        let mut ready = Vec::new();
        self.pending.retain(|(index, deadline)| {
            if *deadline > now {
                return true;
            }
            ready.push(*index);
            false
        });

        for index in ready {
            if !self.is_complete(index) {
                log::trace!("drain_deferrals: pending index {index} not complete, dropped");
                continue;
            }
            log::trace!(
                "drain_deferrals: pending index {index} target {:?} press emitted",
                self.mappings[index].target
            );
            self.emit(
                &mut out,
                NativeEvent::new(self.mappings[index].target.clone(), InputValue::Bool(true)),
            );
            self.held.insert(index);
        }

        let mut ready = Vec::new();
        self.scheduled.retain(|capability, event| {
            if event.is_ready() {
                log::trace!("drain_deferrals: scheduled {capability:?} ready, emitted");
                ready.push(event.clone().into());
                return false;
            }
            log::trace!("drain_deferrals: scheduled {capability:?} not ready yet");
            true
        });
        out.extend(ready);

        out
    }

    /// All source capabilities consumed by any mapping.
    pub fn translatable_capabilities(&self) -> HashSet<Capability> {
        self.mappings
            .iter()
            .flat_map(|mapping| {
                mapping
                    .source_events
                    .iter()
                    .map(|(_, capability)| capability)
            })
            .filter(|capability| **capability != Capability::NotImplemented)
            .cloned()
            .collect()
    }

    /// List of capabilities after accounting for applied translations.
    pub fn exposed_capabilities(&self, raw_kept: &HashSet<Capability>) -> HashSet<Capability> {
        let mut capabilities: HashSet<Capability> = raw_kept
            .difference(&self.translatable_capabilities())
            .cloned()
            .collect();
        for mapping in self.mappings.iter() {
            let complete = mapping
                .source_events
                .iter()
                .all(|(_, capability)| raw_kept.contains(capability));
            if complete {
                capabilities.insert(mapping.target.clone());
            }
        }
        capabilities
    }

    fn press(&mut self, signature: EventSignature, event: &InputEvent, out: &mut Vec<NativeEvent>) {
        if self.chords.contains_key(&signature) {
            log::trace!("press: {signature:?} chord branch");
            self.press_chord(signature, out);
            return;
        }
        if let Some(indices) = self.multi_source.get(&signature).cloned() {
            log::trace!("press: {signature:?} multi_source branch, indices={indices:?}");
            self.translate_multi_source(&indices, event, out);
            return;
        }
        log::trace!("press: {signature:?} no branch matched");
    }

    fn release(
        &mut self,
        signature: EventSignature,
        event: &InputEvent,
        out: &mut Vec<NativeEvent>,
    ) {
        self.release_chord(signature, out);
        if let Some(indices) = self.multi_source.get(&signature).cloned() {
            log::trace!("release: {signature:?} multi_source branch, indices={indices:?}");
            self.translate_multi_source(&indices, event, out);
        }
    }

    fn press_chord(&mut self, signature: EventSignature, out: &mut Vec<NativeEvent>) {
        // Repeated presses of an already active source are ignored.
        if !self.active.insert(signature) {
            log::trace!("press_chord: {signature:?} repeat, ignored");
            return;
        }
        let Some(indices) = self.chords.get(&signature).cloned() else {
            log::trace!("press_chord: {signature:?} active but no chord indices");
            return;
        };
        log::trace!("press_chord: {signature:?} active, indices={indices:?}");

        for index in indices {
            if self.held.contains(&index) {
                log::trace!("press_chord: index {index} already held, skipped");
                continue;
            }
            if self.pending.iter().any(|(pending, _)| *pending == index) {
                log::trace!("press_chord: index {index} already pending, skipped");
                continue;
            }
            if !self.is_complete(index) {
                log::trace!("press_chord: index {index} incomplete, skipped");
                continue;
            }

            if self.mappings[index].deferrable {
                // Defer until a larger chord completes or the delay expires.
                log::trace!(
                    "press_chord: index {index} target {:?} deferred",
                    self.mappings[index].target
                );
                self.pending
                    .push((index, Instant::now() + CHORD_DEFER_DELAY));
            } else {
                log::trace!(
                    "press_chord: index {index} target {:?} press emitted",
                    self.mappings[index].target
                );
                self.emit(
                    out,
                    NativeEvent::new(self.mappings[index].target.clone(), InputValue::Bool(true)),
                );
                self.held.insert(index);
            }
        }

        // A pending press loses to any pending or held chord that contains
        // all of its sources.
        let pending: Vec<usize> = self.pending.iter().map(|(index, _)| *index).collect();
        let held: Vec<usize> = self.held.iter().copied().collect();
        self.pending.retain(|(index, _)| {
            !pending.iter().chain(held.iter()).any(|other| {
                index != other
                    && translation_is_subset(&self.mappings[*index], &self.mappings[*other])
            })
        });
    }

    fn release_chord(&mut self, signature: EventSignature, out: &mut Vec<NativeEvent>) {
        if !self.active.remove(&signature) {
            log::trace!("release_chord: {signature:?} not active, ignored");
            return;
        }

        // A pending chord whose source released fires as a short press and
        // release.
        let taps: Vec<usize> = self
            .pending
            .iter()
            .map(|(index, _)| *index)
            .filter(|index| self.contains_source(*index, signature))
            .collect();
        self.pending.retain(|(index, _)| !taps.contains(index));
        for index in taps {
            let target = self.mappings[index].target.clone();
            log::trace!("release_chord: {signature:?} pending index {index} tap fires {target:?}");
            self.emit(
                out,
                NativeEvent::new(target.clone(), InputValue::Bool(true)),
            );
            self.emit(out, NativeEvent::new(target, InputValue::Bool(false)));
        }

        // A held chord releases on the release of any of its sources.
        let releases: Vec<usize> = self
            .held
            .iter()
            .copied()
            .filter(|index| self.contains_source(*index, signature))
            .collect();
        for index in releases {
            log::trace!(
                "release_chord: {signature:?} index {index} releases {:?}",
                self.mappings[index].target
            );
            self.emit(
                out,
                NativeEvent::new(self.mappings[index].target.clone(), InputValue::Bool(false)),
            );
            self.held.remove(&index);
        }
    }

    fn translate_multi_source(
        &mut self,
        indices: &[usize],
        event: &InputEvent,
        out: &mut Vec<NativeEvent>,
    ) {
        for &index in indices {
            let mapping = &self.mappings[index];
            let Some(value_type) = mapping.value_type.as_ref() else {
                continue;
            };

            // Only translate values matching the axis direction; zero
            // matches both.
            if let Some(direction) = mapping.axis_direction.as_ref() {
                let value = event.value();
                match direction {
                    AxisDirection::None => (),
                    AxisDirection::Positive => {
                        if value < 0 {
                            continue;
                        }
                    }
                    AxisDirection::Negative => {
                        if value > 0 {
                            continue;
                        }
                    }
                }
            }

            let value = input_value(event, value_type, &mapping.target, &self.abs_info);
            log::trace!(
                "Translated value {:?} {} to {:?}",
                event.code(),
                event.value(),
                value
            );
            self.emit(out, NativeEvent::new(mapping.target.clone(), value));
        }
    }

    /// Emit one translated event through the debounce; held events are
    /// parked for a later drain_deferrals() call.
    fn emit(&mut self, out: &mut Vec<NativeEvent>, event: NativeEvent) {
        let now = Instant::now();
        if try_emit(&mut self.recent, MIN_EVENT_TIME, &event, now) {
            // A new press cancels any release held for the same capability.
            if event.pressed() {
                self.scheduled.remove(&event.as_capability());
            }
            log::trace!(
                "emit: {:?} {} passed window",
                event.as_capability(),
                if event.pressed() { "press" } else { "release" }
            );
            out.push(event);
            return;
        }
        let capability = event.as_capability();
        // Hold the release until the full interval has elapsed since the
        // press, not for a further full interval.
        let wait = self
            .recent
            .get(&capability)
            .map(|pressed_at| *pressed_at + MIN_EVENT_TIME - now)
            .unwrap_or(MIN_EVENT_TIME);
        log::trace!("emit: {capability:?} inside window, parked for {wait:?}");
        self.scheduled
            .insert(capability, ScheduledNativeEvent::new(event, wait));
    }

    fn is_complete(&self, index: usize) -> bool {
        self.mappings[index]
            .source_events
            .iter()
            .all(|(signatures, _)| {
                signatures
                    .iter()
                    .any(|signature| self.active.contains(signature))
            })
    }

    fn contains_source(&self, index: usize, signature: EventSignature) -> bool {
        self.mappings[index]
            .source_events
            .iter()
            .any(|(signatures, _)| signatures.contains(&signature))
    }
}

/// Minimum press debounce decision. Returns false when a release arrives
/// within `window` of the capability's last emission.
pub fn try_emit(
    recent: &mut HashMap<Capability, Instant>,
    window: Duration,
    event: &NativeEvent,
    now: Instant,
) -> bool {
    let capability = event.as_capability();
    if event.pressed() {
        recent.insert(capability, now);
        return true;
    }
    if let Some(last) = recent.get(&capability) {
        if now.duration_since(*last) < window {
            return false;
        }
    }
    recent.insert(capability, now);
    true
}

/// True when every source of `small` is also a source of `large`, and
/// `large` has at least one more.
fn translation_is_subset(small: &MappedTranslation, large: &MappedTranslation) -> bool {
    if small.source_events.len() >= large.source_events.len() {
        return false;
    }
    let mut used = vec![false; large.source_events.len()];
    for (signatures, _) in small.source_events.iter() {
        let Some(index) =
            large
                .source_events
                .iter()
                .enumerate()
                .position(|(index, (large_signatures, _))| {
                    !used[index]
                        && large_signatures.len() == signatures.len()
                        && signatures.iter().all(|s| large_signatures.contains(s))
                })
        else {
            return false;
        };
        used[index] = true;
    }
    true
}

fn normalized_value(
    event: &InputEvent,
    value_type: &ValueType,
    target: &Capability,
    abs_info: &HashMap<AbsoluteAxisCode, AbsInfo>,
) -> f64 {
    let raw = event.value();
    match value_type {
        ValueType::ImuX | ValueType::ImuY | ValueType::ImuZ => {
            let code = AbsoluteAxisCode(event.code());
            imu_value(raw, target, abs_info.get(&code))
        }
        ValueType::Button | ValueType::Trigger | ValueType::JoystickX | ValueType::JoystickY => {
            axis_value(raw, event, value_type, abs_info)
        }
    }
}

fn axis_value(
    raw: i32,
    event: &InputEvent,
    value_type: &ValueType,
    abs_info: &HashMap<AbsoluteAxisCode, AbsInfo>,
) -> f64 {
    if event.event_type() != EventType::ABSOLUTE {
        return raw as f64;
    }
    let code = AbsoluteAxisCode(event.code());
    let Some(info) = abs_info.get(&code) else {
        log::trace!("Unable to find ABS info for event: {code:?}");
        return raw as f64;
    };
    match value_type {
        ValueType::Button if info.minimum() < 0 => {
            normalize_signed_value(raw, info.minimum(), info.maximum())
        }
        ValueType::Button | ValueType::Trigger => {
            normalize_unsigned_value(raw, info.minimum(), info.maximum())
        }
        ValueType::JoystickX | ValueType::JoystickY => {
            normalize_signed_value(raw, info.minimum(), info.maximum())
        }
        _ => raw as f64,
    }
}

// Kernel IMU nodes report calibrated counts and set the per axis resolution
// to counts per g (accelerometer) or per degree/second (gyroscope). Fall
// back to typical values when the driver leaves it unset.
fn imu_value(raw: i32, target: &Capability, info: Option<&AbsInfo>) -> f64 {
    let resolution = info.and_then(|info| {
        let res = info.resolution();
        (res > 0).then_some(res as f64)
    });
    match target {
        Capability::Accelerometer(_) => {
            let res = resolution.unwrap_or(ACCELEROMETER_FALLBACK_RES);
            raw as f64 / res * GRAVITY
        }
        Capability::Gyroscope(_) => {
            let res = resolution.unwrap_or(GYROSCOPE_FALLBACK_RES);
            raw as f64 / res * (PI / 180.0)
        }
        _ => raw as f64,
    }
}

fn input_value(
    event: &InputEvent,
    value_type: &ValueType,
    target: &Capability,
    abs_info: &HashMap<AbsoluteAxisCode, AbsInfo>,
) -> InputValue {
    let normal = normalized_value(event, value_type, target, abs_info);
    match value_type {
        ValueType::Button => InputValue::Bool(normal.abs() > 0.5),
        ValueType::Trigger => InputValue::Float(normal),
        ValueType::JoystickX => InputValue::Vector2 {
            x: Some(normal),
            y: None,
        },
        ValueType::JoystickY => InputValue::Vector2 {
            x: None,
            y: Some(normal),
        },
        ValueType::ImuX => InputValue::Vector3 {
            x: Some(normal),
            y: None,
            z: None,
        },
        ValueType::ImuY => InputValue::Vector3 {
            x: None,
            y: Some(normal),
            z: None,
        },
        ValueType::ImuZ => InputValue::Vector3 {
            x: None,
            y: None,
            z: Some(normal),
        },
    }
}
