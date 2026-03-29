use std::collections::HashSet;
use std::fmt::Debug;
use std::os::fd::AsFd;
use std::time::{Duration, Instant};
use std::{collections::HashMap, error::Error};

use evdev::{
    AbsInfo, AbsoluteAxisCode, Device, EventSummary, InputEvent, KeyCode, MiscCode,
    SynchronizationCode,
};
use nix::fcntl::{FcntlArg, OFlag};

use crate::config::capability_map::CapabilityMapConfigV2;
use crate::config::TouchscreenConfig;
use crate::input::capability::{GestureArea, GestureType, Touch};
use crate::input::event::evdev::translator::EventTranslator;
use crate::input::event::value::InputValue;
use crate::{
    input::{
        capability::Capability,
        event::native::NativeEvent,
        source::{InputError, SourceInputDevice, SourceOutputDevice},
    },
    udev::device::UdevDevice,
};

/// Edge zone for gesture detection: finger must start within this fraction of the edge.
const GESTURE_START: f64 = 0.03;
/// Edge zone for pre-suppression in grab mode: narrower than GESTURE_START so that
/// taps and scrolls near (but not at) the edge still pass through freely.
const GESTURE_SUPPRESS_START: f64 = 0.01;
/// Minimum travel distance (as a fraction of screen) required to confirm a gesture.
const GESTURE_MIN_TRAVEL: f64 = 0.12;
/// Maximum duration from first touch to gesture recognition
const GESTURE_TIME: Duration = Duration::from_millis(400);
/// Y coordinate ratio separating the top and bottom gesture areas for left/right swipes
const GESTURE_TOP_RATIO: f64 = 0.33;

/// Orientation of the touchscreen used to translate touch
#[derive(Debug, Clone, Copy, Default)]
enum Orientation {
    #[default]
    Normal,
    RotateLeft,
    RotateRight,
    UpsideDown,
}

impl From<&str> for Orientation {
    fn from(value: &str) -> Self {
        match value {
            "normal" => Self::Normal,
            "left" => Self::RotateLeft,
            "right" => Self::RotateRight,
            "upsidedown" => Self::UpsideDown,
            other => {
                log::warn!("Unknown touchscreen orientation '{other}', defaulting to normal");
                Self::Normal
            }
        }
    }
}

/// Lifecycle of a single-finger edge-swipe gesture within one touch sequence
#[derive(Debug, Default)]
enum GesturePhase {
    /// No touch in progress.
    #[default]
    Idle,
    /// Touch started; watching for a gesture.
    /// In grab mode, `suppressing` indicates whether slot-0 events are being
    /// held back because the touch started in an edge zone.
    Tracking { suppressing: bool },
    /// A gesture was recognized; suppress remaining touch events until lift.
    Triggered,
    /// A second finger arrived; gesture detection disabled for this touch.
    Invalidated,
}

/// Tracks a single-finger swipe gesture in progress
#[derive(Debug, Default)]
struct GestureState {
    start_x: f64,
    /// None until the first Y-axis event arrives after touch-down
    start_y: Option<f64>,
    last_x: f64,
    last_y: f64,
    start_time: Option<Instant>,
    phase: GesturePhase,
}

impl GestureState {
    /// Returns true if no touch is currently being tracked
    fn is_idle(&self) -> bool {
        matches!(self.phase, GesturePhase::Idle)
    }

    /// Returns true if slot-0 touch events should not be forwarded
    fn is_suppressing(&self) -> bool {
        matches!(
            self.phase,
            GesturePhase::Tracking { suppressing: true } | GesturePhase::Triggered
        )
    }

    /// Multi-finger touch detected; disable gesture for this touch sequence
    fn invalidate(&mut self) {
        self.phase = GesturePhase::Invalidated;
    }

    /// Gesture recognized; suppress remaining touch and block re-initialization
    fn mark_triggered(&mut self) {
        self.phase = GesturePhase::Triggered;
    }

    /// Finger released; reset all state
    fn reset(&mut self) {
        *self = GestureState::default();
    }

    /// Returns true if a gesture is still being evaluated (within time limit)
    fn is_active(&self) -> bool {
        let GesturePhase::Tracking { .. } = self.phase else {
            return false;
        };
        self.start_time
            .map(|t| t.elapsed() <= GESTURE_TIME)
            .unwrap_or(false)
    }
}

/// TouchState represents the state of a single touch
#[derive(Debug, Clone)]
struct TouchState {
    is_touching: bool,
    pressure: f64,
    x: f64,
    y: f64,
}

impl Default for TouchState {
    fn default() -> Self {
        Self {
            is_touching: Default::default(),
            pressure: 1.0,
            x: Default::default(),
            y: Default::default(),
        }
    }
}

impl TouchState {
    /// Rotates the touch input to the given orientation
    fn rotate(&self, orientation: Orientation) -> Self {
        let mut value = self.clone();
        let (x, y) = match orientation {
            Orientation::Normal => (self.x, self.y),
            Orientation::UpsideDown => (1.0 - self.x, 1.0 - self.y),
            Orientation::RotateLeft => (1.0 - self.y, self.x),
            Orientation::RotateRight => (self.y, 1.0 - self.x),
        };
        value.x = x;
        value.y = y;

        value
    }

    /// Convert the touch into an InputPlumber value with the given touch index
    fn to_value(&self, idx: u8) -> InputValue {
        InputValue::Touch {
            index: idx,
            is_touching: self.is_touching,
            pressure: Some(self.pressure),
            x: Some(self.x),
            y: Some(self.y),
        }
    }

    /// Convert the touch into an InputPlumber event with the given touch index
    fn to_native_event(&self, idx: u8) -> NativeEvent {
        NativeEvent::new(Capability::Touchscreen(Touch::Motion), self.to_value(idx))
    }
}

/// Source device implementation for evdev touchscreens
/// https://www.kernel.org/doc/Documentation/input/multi-touch-protocol.txt
pub struct TouchscreenEventDevice {
    device: Device,
    translator: Option<EventTranslator>,
    orientation: Orientation,
    axes_info: HashMap<AbsoluteAxisCode, AbsInfo>,
    touch_state: [TouchState; 10], // NOTE: Max of 10 touch inputs
    dirty_states: HashSet<usize>,
    last_touch_idx: usize,
    gesture_state: GestureState,
    /// When true, the device is grabbed exclusively and touch events in the
    /// edge zone are suppressed until a gesture is confirmed or ruled out.
    grab: bool,
}

impl TouchscreenEventDevice {
    /// Create a new Touchscreen source device from the given udev info
    pub fn new(
        device_info: UdevDevice,
        config: Option<TouchscreenConfig>,
        capability_map: Option<CapabilityMapConfigV2>,
    ) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let path = device_info.devnode();
        log::debug!("Opening device at: {}", path);
        let mut device = Device::open(path.clone())?;

        let grab = if config.as_ref().and_then(|c| c.grab).unwrap_or(false) {
            match device.grab() {
                Ok(_) => true,
                Err(e) => {
                    log::warn!(
                        "Failed to grab touchscreen, falling back to pass-through mode: {e}"
                    );
                    false
                }
            }
        } else {
            false
        };

        // Set the device to do non-blocking reads
        // TODO: use epoll to wake up when data is available
        // https://github.com/emberian/evdev/blob/main/examples/evtest_nonblocking.rs
        let fd = device.as_fd();
        nix::fcntl::fcntl(fd, FcntlArg::F_SETFL(OFlag::O_NONBLOCK))?;

        // Check to see if the user wants to override the screen width/height.
        let override_size = {
            let override_opt = config.as_ref().and_then(|c| c.override_source_size);
            override_opt.unwrap_or_default()
        };

        // Query information about the device to get the absolute ranges
        let mut axes_info = HashMap::new();
        for (axis, info) in device.get_absinfo()? {
            log::trace!("Found axis: {:?}", axis);
            log::trace!("Found info: {:?}", info);

            // If the user isn't overriding the size or this axis doesn't contain
            // size information, then use the original limits advertised by the device.
            let is_size_info = axis == AbsoluteAxisCode::ABS_MT_POSITION_X
                || axis == AbsoluteAxisCode::ABS_MT_POSITION_Y;
            if !override_size || !is_size_info {
                axes_info.insert(axis, info);
                continue;
            }

            // Override the axis maximum if the user has that defined.
            let mut maximum = info.maximum();

            let width = config.as_ref().and_then(|c| c.width);
            if axis == AbsoluteAxisCode::ABS_MT_POSITION_X && width.is_some() {
                maximum = width.unwrap_or_default() as i32;
            }

            let height = config.as_ref().and_then(|c| c.height);
            if axis == AbsoluteAxisCode::ABS_MT_POSITION_Y && height.is_some() {
                maximum = height.unwrap_or_default() as i32;
            }

            let modified_info = AbsInfo::new(
                info.value(),
                info.minimum(),
                maximum,
                info.fuzz(),
                info.flat(),
                info.resolution(),
            );
            axes_info.insert(axis, modified_info);
        }

        // Configure the orientation of the touchscreen
        let orientation =
            if let Some(orientation) = config.as_ref().and_then(|c| c.orientation.as_ref()) {
                Orientation::from(orientation.as_str())
            } else {
                Orientation::default()
            };
        log::debug!("Configured touchscreen orientation: {orientation:?}");

        // Create an event translator if a capability map was given
        let translator = capability_map.map(|map| EventTranslator::new(&map, axes_info.clone()));

        Ok(Self {
            device,
            orientation,
            translator,
            axes_info,
            touch_state: Default::default(),
            dirty_states: HashSet::with_capacity(10),
            last_touch_idx: 0,
            gesture_state: GestureState::default(),
            grab,
        })
    }

    /// Translate the given evdev event into a native event
    fn translate(&mut self, event: InputEvent) -> Vec<NativeEvent> {
        log::trace!("Received event: {:?}", event);

        // Update internal touch state until a synchronization event occurs. Each
        // event that comes in will update the touch state and update 'dirty_states'
        // to indicate that events should be sent for that touch index when a
        // SYN_REPORT event occurs.
        match event.destructure() {
            // Synchronization events indicate that touch events can be emitted
            EventSummary::Synchronization(_, SynchronizationCode::SYN_REPORT, _) => {
                let mut events = Vec::with_capacity(self.dirty_states.len());

                // Send events for any dirty touch states
                for idx in self.dirty_states.drain() {
                    let Some(touch) = self.touch_state.get_mut(idx) else {
                        continue;
                    };

                    // Suppress slot-0 touch events while a potential gesture is being tracked
                    if idx == 0 && self.gesture_state.is_suppressing() {
                        continue;
                    }

                    // Rotate values based on config
                    let rotated_touch = touch.rotate(self.orientation);
                    let event = rotated_touch.to_native_event(idx as u8);
                    events.push(event);
                }

                // Detect edge-swipe gestures from the primary touch slot
                events.extend(self.detect_gesture());

                return events;
            }
            // The BTN_TOUCH event occurs whenever touches have started or stopped.
            // This can be used to reset the last touch index when no touches are
            // detected.
            EventSummary::Key(_, KeyCode::BTN_TOUCH, value) => {
                if value == 0 {
                    // Reset last touch index when all touches stop
                    self.last_touch_idx = 0;
                } else {
                    self.dirty_states.insert(0);
                }
            }
            // The ABS_MT_SLOT event defines the index of the touch. E.g. "0" would
            // be the first touch, "1", the second, etc. Upon receiving this event,
            // any following ABS_X/Y events are associated with this touch index.
            EventSummary::AbsoluteAxis(_, AbsoluteAxisCode::ABS_MT_SLOT, value) => {
                let slot = value as usize;
                self.last_touch_idx = slot;
                self.dirty_states.insert(slot);
                // A second finger arriving invalidates any in-progress gesture
                if slot > 0 {
                    self.gesture_state.invalidate();
                }
            }
            // Whenever a touch is lifted, an ABS_MT_TRACKING_ID event with a value of
            // -1 event will occur.
            EventSummary::AbsoluteAxis(_, AbsoluteAxisCode::ABS_MT_TRACKING_ID, -1) => {
                if let Some(touch) = self.touch_state.get_mut(self.last_touch_idx) {
                    touch.is_touching = false;

                    if self.last_touch_idx == 0 && self.gesture_state.is_suppressing() {
                        // Touch was pre-suppressed. If no gesture fired, the user
                        // made a tap or short swipe that didn't qualify as a gesture;
                        // replay it as a synthetic press→release so the system sees it.
                        if !matches!(self.gesture_state.phase, GesturePhase::Triggered) {
                            if let Some(start_y) = self.gesture_state.start_y {
                                let press = TouchState {
                                    is_touching: true,
                                    x: self.gesture_state.start_x,
                                    y: start_y,
                                    pressure: 1.0,
                                };
                                let release = TouchState {
                                    is_touching: false,
                                    ..press.clone()
                                };
                                self.gesture_state.reset();
                                return vec![
                                    press.rotate(self.orientation).to_native_event(0),
                                    release.rotate(self.orientation).to_native_event(0),
                                ];
                            }
                        }
                        // Gesture fired (or no Y data recorded): discard silently.
                    } else {
                        // Only emit release if this touch was not suppressed (i.e. the
                        // system never saw a press, so sending a release would confuse it)
                        self.dirty_states.insert(self.last_touch_idx);
                    }
                }
                // Primary finger lifted: reset gesture state
                if self.last_touch_idx == 0 {
                    self.gesture_state.reset();
                }
            }
            // Emitted whenever touch motion is detected for the X axis
            EventSummary::AbsoluteAxis(_, AbsoluteAxisCode::ABS_MT_POSITION_X, value) => {
                // Get the axis information so the value can be normalized
                let Some(info) = self.axes_info.get(&AbsoluteAxisCode::ABS_MT_POSITION_X) else {
                    return vec![];
                };
                let normal_value = normalize_unsigned_value(value, info.maximum());

                // Select the current slot to update
                if let Some(touch) = self.touch_state.get_mut(self.last_touch_idx) {
                    touch.is_touching = true;
                    touch.x = normal_value;
                    self.dirty_states.insert(self.last_touch_idx);
                }

                // Track gesture only for the primary slot
                if self.last_touch_idx == 0 {
                    if self.gesture_state.is_idle() {
                        // In grab mode, pre-suppress touches that start in the
                        // edge zone to avoid any leakage before gesture confirm.
                        let suppressing = self.grab
                            && !(GESTURE_SUPPRESS_START..=1.0 - GESTURE_SUPPRESS_START).contains(&normal_value);
                        self.gesture_state.start_x = normal_value;
                        self.gesture_state.start_time = Some(Instant::now());
                        self.gesture_state.phase = GesturePhase::Tracking { suppressing };
                    }
                    self.gesture_state.last_x = normal_value;
                }
            }
            // Emitted whenever touch motion is detected for the Y axis
            EventSummary::AbsoluteAxis(_, AbsoluteAxisCode::ABS_MT_POSITION_Y, value) => {
                // Get the axis information so the value can be normalized
                let Some(info) = self.axes_info.get(&AbsoluteAxisCode::ABS_MT_POSITION_Y) else {
                    return vec![];
                };
                let normal_value = normalize_unsigned_value(value, info.maximum());

                // Select the current slot to update
                if let Some(touch) = self.touch_state.get_mut(self.last_touch_idx) {
                    touch.is_touching = true;
                    touch.y = normal_value;
                    self.dirty_states.insert(self.last_touch_idx);
                }

                // Track gesture only for the primary slot
                if self.last_touch_idx == 0 {
                    if self.gesture_state.start_y.is_none()
                        && matches!(self.gesture_state.phase, GesturePhase::Tracking { .. })
                    {
                        self.gesture_state.start_y = Some(normal_value);
                        // In grab mode, also suppress touches starting at the
                        // top or bottom edge.
                        if self.grab
                            && !(GESTURE_SUPPRESS_START..=1.0 - GESTURE_SUPPRESS_START).contains(&normal_value)
                        {
                            self.gesture_state.phase = GesturePhase::Tracking { suppressing: true };
                        }
                    }
                    self.gesture_state.last_y = normal_value;
                }
            }
            // Some touchscreens support touch pressure and emit this event.
            EventSummary::AbsoluteAxis(_, AbsoluteAxisCode::ABS_PRESSURE, value) => {
                // Get the axis information so the value can be normalized
                let Some(info) = self.axes_info.get(&AbsoluteAxisCode::ABS_PRESSURE) else {
                    return vec![];
                };
                let normal_value = normalize_unsigned_value(value, info.maximum());

                // Select the current slot to update
                if let Some(touch) = self.touch_state.get_mut(self.last_touch_idx) {
                    touch.pressure = normal_value;
                    self.dirty_states.insert(self.last_touch_idx);
                }
            }
            EventSummary::Misc(_, MiscCode::MSC_TIMESTAMP, _) => (),
            _ => (),
        }

        vec![]
    }

    /// Attempt to recognize a completed edge-swipe gesture from the current
    /// gesture state. Returns gesture events if one is recognized, or an empty
    /// vec if the gesture has not yet been confirmed.
    fn detect_gesture(&mut self) -> Vec<NativeEvent> {
        if !self.gesture_state.is_active() {
            return vec![];
        }

        let Some(raw_start_y) = self.gesture_state.start_y else {
            return vec![];
        };

        // Rotate gesture coordinates to match display orientation before
        // evaluating edge zones and travel direction.
        let start = TouchState {
            is_touching: true,
            x: self.gesture_state.start_x,
            y: raw_start_y,
            pressure: 1.0,
        }
        .rotate(self.orientation);
        let last = TouchState {
            is_touching: true,
            x: self.gesture_state.last_x,
            y: self.gesture_state.last_y,
            pressure: 1.0,
        }
        .rotate(self.orientation);

        let (start_x, start_y) = (start.x, start.y);
        let (last_x, last_y) = (last.x, last.y);

        let gesture_type = if start_x < GESTURE_START && (last_x - start_x) > GESTURE_MIN_TRAVEL {
            // Swipe inward from the left edge
            let area = if start_y < GESTURE_TOP_RATIO {
                GestureArea::Top
            } else {
                GestureArea::Bottom
            };
            Some(GestureType::Right(area))
        } else if start_x > 1.0 - GESTURE_START && (start_x - last_x) > GESTURE_MIN_TRAVEL {
            // Swipe inward from the right edge
            let area = if start_y < GESTURE_TOP_RATIO {
                GestureArea::Top
            } else {
                GestureArea::Bottom
            };
            Some(GestureType::Left(area))
        } else if start_y > 1.0 - GESTURE_START && (start_y - last_y) > GESTURE_MIN_TRAVEL {
            // Swipe inward from the bottom edge
            Some(GestureType::Up)
        } else if start_y < GESTURE_START && (last_y - start_y) > GESTURE_MIN_TRAVEL {
            // Swipe inward from the top edge
            Some(GestureType::Down)
        } else {
            None
        };

        if let Some(gesture) = gesture_type {
            log::debug!("Gesture detected: {:?}", gesture);

            // Capture suppression state before transitioning: if the touch was
            // NOT pre-suppressed, the system already received press frames and
            // needs a matching synthetic release before the gesture fires.
            let needs_synthetic_release = self.grab && !self.gesture_state.is_suppressing();
            self.gesture_state.mark_triggered();

            let cap = Capability::Touchscreen(Touch::Gesture(gesture));
            let mut events = vec![
                NativeEvent::new(cap.clone(), InputValue::Bool(true)),
                NativeEvent::new(cap, InputValue::Bool(false)),
            ];

            if needs_synthetic_release {
                let mut release_state = self.touch_state[0].clone();
                release_state.is_touching = false;
                let release_event = release_state.rotate(self.orientation).to_native_event(0);
                events.insert(0, release_event);
            }

            events
        } else {
            vec![]
        }
    }
}

impl SourceInputDevice for TouchscreenEventDevice {
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
                .map(|e| self.translate(e))
                .filter(|events| !events.is_empty())
                .flatten()
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

        // Convert the events into native events
        let translated_events: Vec<NativeEvent> = untranslated_events
            .into_iter()
            .map(|e| self.translate(e))
            .filter(|events| !events.is_empty())
            .flatten()
            .collect();
        native_events.extend(translated_events);

        Ok(native_events)
    }

    /// Returns the possible input events this device is capable of emitting
    fn get_capabilities(&self) -> Result<Vec<Capability>, InputError> {
        Ok(vec![
            Capability::Touchscreen(Touch::Motion),
            Capability::Touchscreen(Touch::Gesture(GestureType::Right(GestureArea::Top))),
            Capability::Touchscreen(Touch::Gesture(GestureType::Right(GestureArea::Bottom))),
            Capability::Touchscreen(Touch::Gesture(GestureType::Left(GestureArea::Top))),
            Capability::Touchscreen(Touch::Gesture(GestureType::Left(GestureArea::Bottom))),
            Capability::Touchscreen(Touch::Gesture(GestureType::Up)),
            Capability::Touchscreen(Touch::Gesture(GestureType::Down)),
        ])
    }
}

impl SourceOutputDevice for TouchscreenEventDevice {}

impl Debug for TouchscreenEventDevice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TouchscreenEventDevice")
            .field("axes_info", &self.axes_info)
            .finish()
    }
}

// Returns a value between 0.0 and 1.0 based on the given value with its
// maximum.
fn normalize_unsigned_value(raw_value: i32, max: i32) -> f64 {
    if max <= 0 {
        return 0.0;
    }
    raw_value as f64 / max as f64
}
