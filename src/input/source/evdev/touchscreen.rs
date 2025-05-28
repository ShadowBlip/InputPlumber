use std::collections::HashSet;
use std::fmt::Debug;
use std::time::Duration;
use std::{collections::HashMap, error::Error, os::fd::AsRawFd};

use evdev::{
    AbsInfo, AbsoluteAxisCode, Device, EventSummary, InputEvent, KeyCode, MiscCode,
    SynchronizationCode,
};
use nix::fcntl::{FcntlArg, OFlag};
use tokio::time::{interval, Interval};

use crate::config::capability_map::CapabilityMapConfigV2;
use crate::config::TouchscreenConfig;
use crate::input::capability::Touch;
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
            _ => Self::Normal,
        }
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
    interval: Interval,
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
        device.grab()?;

        // Set the device to do non-blocking reads
        // TODO: use epoll to wake up when data is available
        // https://github.com/emberian/evdev/blob/main/examples/evtest_nonblocking.rs
        let raw_fd = device.as_raw_fd();
        nix::fcntl::fcntl(raw_fd, FcntlArg::F_SETFL(OFlag::O_NONBLOCK))?;

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

        // Set polling interval
        let interval = interval(Duration::from_millis(10));

        Ok(Self {
            device,
            orientation,
            translator,
            axes_info,
            touch_state: Default::default(),
            dirty_states: HashSet::with_capacity(10),
            last_touch_idx: 0,
            interval,
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

                    // Rotate values based on config
                    let rotated_touch = touch.rotate(self.orientation);
                    let event = rotated_touch.to_native_event(idx as u8);
                    events.push(event);
                }

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
                // Select the current slot to update
                let slot = value as usize;
                self.last_touch_idx = slot;
                self.dirty_states.insert(slot);
            }
            // Whenever a touch is lifted, an ABS_MT_TRACKING_ID event with a value of
            // -1 event will occur.
            EventSummary::AbsoluteAxis(_, AbsoluteAxisCode::ABS_MT_TRACKING_ID, -1) => {
                if let Some(touch) = self.touch_state.get_mut(self.last_touch_idx) {
                    touch.is_touching = false;
                    self.dirty_states.insert(self.last_touch_idx);
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
}

impl SourceInputDevice for TouchscreenEventDevice {
    /// Poll the given input device for input events
    async fn poll(&mut self) -> Result<Vec<NativeEvent>, InputError> {
        self.interval.tick().await;
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
        log::trace!("Read events from device: {events:?}");

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

        log::trace!("Sending events: {native_events:?}");

        Ok(native_events)
    }

    /// Returns the possible input events this device is capable of emitting
    fn get_capabilities(&self) -> Result<Vec<Capability>, InputError> {
        Ok(vec![Capability::Touchscreen(Touch::Motion)])
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
    raw_value as f64 / max as f64
}
