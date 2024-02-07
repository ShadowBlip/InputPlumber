use std::error::Error;

use evdev::{uinput::VirtualDeviceBuilder, AttributeSet, KeyCode};

#[derive(Debug)]
pub struct KeyboardDevice {}

impl KeyboardDevice {
    pub fn new() -> Self {
        Self {}
    }

    pub fn run() -> Result<(), Box<dyn Error>> {
        let mut keys = AttributeSet::<KeyCode>::new();
        keys.insert(KeyCode::BTN_DPAD_UP);

        let mut device = VirtualDeviceBuilder::new()?
            .name("InputPlumber Keyboard")
            .with_keys(&keys)?
            .build()?;

        Ok(())
    }
}
