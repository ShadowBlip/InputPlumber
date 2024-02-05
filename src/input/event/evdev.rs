use super::MappableEvent;

#[derive(Debug, Clone)]
pub struct EvdevEvent {
    kind: u32,
    code: u32,
    value: u32,
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
        self.value = value.round() as u32;
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
