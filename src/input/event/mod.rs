pub mod context;
pub mod dbus;
pub mod evdev;
pub mod native;
pub mod value;

/// Events are events that flow from source devices to target devices
/// TODO: Remove this enum in favor of directly using NativeEvent
#[derive(Debug, Clone)]
pub enum Event {
    Native(native::NativeEvent),
}
