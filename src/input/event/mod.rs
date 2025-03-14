pub mod dbus;
pub mod evdev;
pub mod native;
pub mod ucis;
pub mod value;

/// Events are events that flow from source devices to target devices
#[derive(Debug, Clone)]
pub enum Event {
    Evdev(evdev::EvdevEvent),
    HIDRaw,
    Native(native::NativeEvent),
    DBus(dbus::DBusEvent),
}
