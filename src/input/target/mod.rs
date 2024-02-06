pub mod keyboard;
pub mod mouse;

/// A [TargetDevice] is any virtual input device that emits input events
#[derive(Debug)]
pub enum TargetDevice {
    Keyboard(keyboard::KeyboardDevice),
    Mouse(mouse::MouseDevice),
}
