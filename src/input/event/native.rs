use std::time::{Duration, Instant};

use evdev::AbsoluteAxisCode;

use crate::input::capability::{Capability, Gamepad, GamepadButton};

use super::{context::EventContext, evdev::EvdevEvent, value::InputValue};

/// A native event represents an InputPlumber event
#[derive(Debug, Clone)]
pub struct NativeEvent {
    /// The capability of the input event. Target input devices that implement
    /// this capability will be able to emit this event.
    capability: Capability,
    /// Optional source capability of the input event if this event was translated
    /// from one type to another. This can allow downstream target input devices
    /// to have different behavior for events that have been translated from one
    /// type to another.
    source_capability: Option<Capability>,
    /// The value of the input event.
    value: InputValue,
    /// The event context contains additional information related to the event, such
    /// as performance metrics and metadata. `Box<>` is used to allocate the
    /// context on the heap to reduce the size of [NativeEvent].
    context: Option<Box<EventContext>>,
}

impl NativeEvent {
    /// Returns a new [NativeEvent] with the given capability and value
    pub fn new(capability: Capability, value: InputValue) -> NativeEvent {
        NativeEvent {
            capability,
            value,
            source_capability: None,
            context: None,
        }
    }

    /// Returns a new [NativeEvent] with the original un-translated source
    /// capability, the translated capability, and value.
    pub fn new_translated(
        source_capability: Capability,
        capability: Capability,
        value: InputValue,
    ) -> NativeEvent {
        NativeEvent {
            capability,
            source_capability: Some(source_capability),
            value,
            context: None,
        }
    }

    /// Returns the capability that this event implements
    pub fn as_capability(&self) -> Capability {
        self.capability.clone()
    }

    /// Returns the value of this event
    pub fn get_value(&self) -> InputValue {
        self.value.clone()
    }

    /// Returns true if this event is a translated event and has a source
    /// capability defined.
    pub fn is_translated(&self) -> bool {
        self.source_capability.is_some()
    }

    /// Returns the current context of the event
    pub fn get_context(&self) -> Option<&EventContext> {
        self.context.as_deref()
    }

    /// Returns the current context of the event
    pub fn get_context_mut(&mut self) -> Option<&mut EventContext> {
        self.context.as_deref_mut()
    }

    /// Add the given event context to the event. Calling this method will
    /// allocate the given [EventContext] on the heap.
    pub fn set_context(&mut self, context: EventContext) {
        self.context = Some(Box::new(context));
    }

    /// Set the source capability of the event if this is a translated event
    #[allow(dead_code)]
    pub fn set_source_capability(&mut self, cap: Capability) {
        self.source_capability = Some(cap);
    }

    /// Returns the source capability that this event was translated from
    pub fn get_source_capability(&self) -> Option<Capability> {
        self.source_capability.clone()
    }

    /// Returns whether or not the event is "pressed"
    pub fn pressed(&self) -> bool {
        self.value.pressed()
    }

    pub fn from_evdev_raw(event: EvdevEvent, hat_state: Option<i32>) -> NativeEvent {
        // If this is a Dpad input, figure out with button this event is for
        let capability = if let Some(old_state) = hat_state {
            let axis = AbsoluteAxisCode(event.as_input_event().code());
            let value = event.as_input_event().value();

            match axis {
                AbsoluteAxisCode::ABS_HAT0X => match value {
                    -1 => Capability::Gamepad(Gamepad::Button(GamepadButton::DPadLeft)),
                    1 => Capability::Gamepad(Gamepad::Button(GamepadButton::DPadRight)),
                    0 => match old_state {
                        -1 => Capability::Gamepad(Gamepad::Button(GamepadButton::DPadLeft)),
                        1 => Capability::Gamepad(Gamepad::Button(GamepadButton::DPadRight)),
                        _ => Capability::NotImplemented,
                    },
                    _ => Capability::NotImplemented,
                },
                AbsoluteAxisCode::ABS_HAT0Y => match value {
                    -1 => Capability::Gamepad(Gamepad::Button(GamepadButton::DPadUp)),
                    1 => Capability::Gamepad(Gamepad::Button(GamepadButton::DPadDown)),
                    0 => match old_state {
                        -1 => Capability::Gamepad(Gamepad::Button(GamepadButton::DPadUp)),
                        1 => Capability::Gamepad(Gamepad::Button(GamepadButton::DPadDown)),
                        _ => Capability::NotImplemented,
                    },
                    _ => Capability::NotImplemented,
                },

                _ => Capability::NotImplemented,
            }
        } else {
            event.as_capability()
        };

        let value = event.get_value();

        NativeEvent {
            capability,
            value,
            source_capability: None,
            context: None,
        }
    }
}

impl From<EvdevEvent> for NativeEvent {
    /// Convert the [EvdevEvent] into a [NativeEvent]
    fn from(item: EvdevEvent) -> Self {
        let capability = item.as_capability();
        let value = item.get_value();
        NativeEvent {
            capability,
            value,
            source_capability: None,
            context: None,
        }
    }
}

impl From<ScheduledNativeEvent> for NativeEvent {
    fn from(value: ScheduledNativeEvent) -> Self {
        value.event
    }
}

/// A scheduled event represents an input event that should be sent sometime in
/// the future.
#[derive(Debug, Clone)]
pub struct ScheduledNativeEvent {
    event: NativeEvent,
    scheduled_time: Instant,
    wait_time: Duration,
}

impl ScheduledNativeEvent {
    /// Create a new scheduled event with the given time to wait before being
    /// emitted.
    pub fn new(event: NativeEvent, wait_time: Duration) -> Self {
        Self {
            event,
            scheduled_time: Instant::now(),
            wait_time,
        }
    }

    /// Create a new scheduled event with the given timestamp and wait time before
    /// being emitted
    pub fn new_with_time(event: NativeEvent, timestamp: Instant, wait_time: Duration) -> Self {
        Self {
            event,
            scheduled_time: timestamp,
            wait_time,
        }
    }

    /// Returns true when the scheduled event is ready to be emitted
    pub fn is_ready(&self) -> bool {
        self.scheduled_time.elapsed() > self.wait_time
    }
}
