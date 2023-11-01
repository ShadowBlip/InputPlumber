use std::{
    collections::HashMap,
    io,
    path::{Path, PathBuf},
};

// https://github.com/emberian/evdev/blob/main/examples/evtest_nonblocking.rs
use evdev::Device;

/// A ManagedGamepad is a physical/virtual gamepad pair for processing input
/// ManagedGamepad will convert physical gamepad input into virtual gamepad input.
pub struct ManagedGamepad {
    phys_devices: HashMap<PathBuf, Device>,
}

impl ManagedGamepad {
    /// Creates a new instance of a managed gamepad
    pub fn new() -> ManagedGamepad {
        ManagedGamepad {
            phys_devices: HashMap::new(),
        }
    }

    /// Opens the given physical devices and combines them into a single virtual
    /// device.
    pub fn open(&mut self, path: impl AsRef<Path>) -> io::Result<()> {
        let device = Device::open(path.as_ref())?;
        self.phys_devices
            .insert(path.as_ref().to_path_buf(), device);

        Ok(())
    }

    /// Sets the gamepad input mode
    pub fn set_mode(&mut self) {}

    /// Grab exclusive access over the physical device(s)
    pub fn grab(&mut self) {}

    /// Processes all physical and virtual inputs for this controller. This
    /// should be called in a tight loop to process input events.
    pub fn process_input(&mut self) {
        for device in self.phys_devices.values_mut() {}
    }
}
