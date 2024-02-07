pub mod keyboard;
pub mod mouse;
pub mod xb360;

/// A [TargetDevice] is any virtual input device that emits input events
#[derive(Debug)]
pub enum TargetDevice {
    Keyboard(keyboard::KeyboardDevice),
    Mouse(mouse::MouseDevice),
    XBox360(xb360::XBox360Controller),
}
