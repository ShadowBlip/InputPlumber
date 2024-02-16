use super::event::native::NativeEvent;

pub mod dbus;
pub mod gamepad;
pub mod keyboard;
pub mod mouse;
pub mod xb360;

/// A [TargetDevice] is any virtual input device that emits input events
#[derive(Debug)]
pub enum TargetDevice {
    Null,
    DBus(dbus::DBusDevice),
    Keyboard(keyboard::KeyboardDevice),
    Mouse(mouse::MouseDevice),
    GenericGamepad(gamepad::GenericGamepad),
    XBox360(xb360::XBox360Controller),
}

/// A [TargetCommand] is a message that can be sent to a [TargetDevice] over
/// a channel.
#[derive(Debug, Clone)]
pub enum TargetCommand {
    WriteEvent(NativeEvent),
    Stop,
}
