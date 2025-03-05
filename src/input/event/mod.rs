pub mod context;
pub mod dbus;
pub mod evdev;
pub mod native;
#[cfg(feature = "networking")]
pub mod ucis;
pub mod value;

#[cfg(test)]
pub mod value_test;

/// Events are events that flow from source devices to target devices
/// TODO: Remove this enum in favor of directly using NativeEvent
#[derive(Debug, Clone)]
pub enum Event {
    Native(native::NativeEvent),
}
