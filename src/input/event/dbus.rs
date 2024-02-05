use super::MappableEvent;

/// A native event represents an InputPlumber event
#[derive(Debug, Clone)]
pub struct DBusEvent {
    kind: u32,
    code: u32,
    value: u32,
}

impl DBusEvent {
    pub fn new() -> DBusEvent {
        DBusEvent {
            code: 0,
            value: 0,
            kind: 0,
        }
    }
}

impl MappableEvent for DBusEvent {
    fn matches<T>(&self, event: T) -> bool
    where
        T: MappableEvent,
    {
        match event.kind() {
            super::Event::DBus(event) => self.code == event.code && self.kind == event.kind,
            _ => false,
        }
    }

    fn set_value(&mut self, value: f64) {
        self.value = value.round() as u32;
    }

    fn get_value(&self) -> f64 {
        self.value as f64
    }

    fn get_signature(&self) -> String {
        todo!()
    }

    fn kind(&self) -> super::Event {
        super::Event::DBus(self.clone())
    }
}
