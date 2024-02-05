pub mod evdev;

/// A [SourceDevice] is any physical input device that emits input events
pub enum SourceDevice {
    EventDevice(evdev::EventDevice),
}
