pub mod context;
pub mod dbus;
pub mod evdev;
pub mod native;
pub mod value;

#[cfg(test)]
pub mod value_test;

use crate::input::capability::Capability;

/// Maps a driver's device events to and from native capabilities
pub trait EventCapability {
    /// The native capability this event represents
    fn to_capability(&self) -> Capability;

    /// The normalized value this event carries, if the event produces a
    /// native event
    fn to_value(&self) -> Option<value::InputValue>;

    /// A prototype event for the given capability, if the device has one
    fn from_capability(capability: &Capability) -> Option<Self>
    where
        Self: Sized;
}

/// Events are events that flow from source devices to target devices
/// TODO: Remove this enum in favor of directly using NativeEvent
#[derive(Debug, Clone)]
pub enum Event {
    Native(native::NativeEvent),
}
