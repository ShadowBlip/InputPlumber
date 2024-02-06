use evdev::InputEvent;

use super::MappableEvent;

#[derive(Debug, Clone)]
pub struct EvdevEvent {
    kind: u32,
    code: u32,
    value: i32,
}

impl EvdevEvent {
    pub fn new() -> EvdevEvent {
        EvdevEvent {
            code: 0,
            value: 0,
            kind: 0,
        }
    }
}

impl From<InputEvent> for EvdevEvent {
    fn from(item: InputEvent) -> Self {
        EvdevEvent {
            kind: item.event_type().0 as u32,
            code: item.code() as u32,
            value: item.value(),
        }
    }
}

impl MappableEvent for EvdevEvent {
    fn matches<T>(&self, event: T) -> bool
    where
        T: MappableEvent,
    {
        match event.kind() {
            super::Event::Evdev(event) => self.code == event.code && self.kind == event.kind,
            _ => false,
        }
    }

    fn set_value(&mut self, value: f64) {
        self.value = value.round() as i32;
    }

    fn get_value(&self) -> f64 {
        self.value as f64
    }

    fn get_signature(&self) -> String {
        todo!()
    }

    fn kind(&self) -> super::Event {
        super::Event::Evdev(self.clone())
    }
}
