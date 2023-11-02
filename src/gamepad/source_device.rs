use evdev::KeyCode;

use crate::input;

/// A [SourceDevice] is a container that holds an input device and related
/// information about that source device.
pub struct SourceDevice {
    pub device: evdev::Device,
    pub info: Option<input::device::Device>,
    pub path: String,
}

impl std::fmt::Debug for SourceDevice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "SourceDevice {{ path: {}, info: {:?} }}",
            self.path, self.info
        )
    }
}

impl SourceDevice {
    /// Returns a new instance of [SourceDevice]
    pub fn new(path: String, device: evdev::Device) -> SourceDevice {
        let mut info = None;
        let filename = path.split("/").last().unwrap();
        let input_devices = input::device::get_all();

        // Try to look for the device information for this device
        if input_devices.is_ok() {
            let input_devices = input_devices.unwrap();
            for dev in input_devices {
                for handler in &dev.handlers {
                    if filename == handler {
                        info = Some(dev);
                        break;
                    }
                }
            }
        }

        SourceDevice { device, info, path }
    }

    /// Returns true if the given evdev device is a gamepad
    pub fn is_gamepad(&self) -> bool {
        let supported = self
            .device
            .supported_keys()
            .map_or(false, |keys| keys.contains(KeyCode::BTN_MODE));
        return supported;
    }

    /// Returns true if the device is detected as virtual
    pub fn is_virtual(&self) -> bool {
        if self
            .info
            .clone()
            .unwrap()
            .sysfs_path
            .contains("/devices/virtual")
        {
            return true;
        }
        return false;
    }
}
