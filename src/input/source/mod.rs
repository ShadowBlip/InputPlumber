pub mod evdev;

/// A [SourceDevice] is any physical input device that emits input events
#[derive(Debug)]
pub enum SourceDevice {
    EventDevice(evdev::EventDevice),
}
