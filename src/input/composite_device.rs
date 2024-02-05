/// A [CompositeDevice] represents any number source input devices that
/// can translate input to any target devices
#[derive(Debug, Clone)]
struct CompositeDevice {
    source_devices: Vec<String>,
}
