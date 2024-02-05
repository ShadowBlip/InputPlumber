pub mod dbus;
pub mod evdev;
pub mod native;

#[derive(Debug, Clone)]
pub enum Event {
    Evdev(evdev::EvdevEvent),
    HIDRaw,
    Native(native::NativeEvent),
    DBus(dbus::DBusEvent),
}

/// Trait to define events that can be mapped to other events
pub trait MappableEvent {
    /// Returns true if the given event matches
    fn matches<T>(&self, event: T) -> bool
    where
        T: MappableEvent;
    /// Returns the kind of event this is
    fn kind(&self) -> Event;
    /// Set the given value on the event
    fn set_value(&mut self, value: f64);
    /// Return the underlying value of the event.
    fn get_value(&self) -> f64;
    /// Returns a signature of the event to aid with faster matching. This signature
    /// should return a unique string based on the kind of event but not the value.
    fn get_signature(&self) -> String;
    /// Returns whether or not the given event only uses binary values (e.g. pressed
    /// or not pressed). Defaults to true.
    fn is_binary_event(&self) -> bool {
        true
    }
}
