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
