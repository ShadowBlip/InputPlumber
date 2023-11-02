use evdev::InputEvent;

/// Events that can be translated from one type to another
enum MappableEvent {}

pub struct EvdevEvent {
    event: InputEvent,
}

impl EvdevEvent {
    fn new(event: &InputEvent) -> EvdevEvent {
        EvdevEvent {
            event: event.clone(),
        }
    }
}

pub struct DBusEvent {}
