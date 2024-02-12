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
