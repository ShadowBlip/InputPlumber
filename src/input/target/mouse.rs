use std::error::Error;

use evdev::{uinput::VirtualDeviceBuilder, AttributeSet, RelativeAxisCode};

#[derive(Debug)]
pub struct MouseDevice {}

impl MouseDevice {
    pub fn new() -> Self {
        Self {}
    }

    pub fn run() -> Result<(), Box<dyn Error>> {
        let mut device = VirtualDeviceBuilder::new()?
            .name("InputPlumber Mouse")
            .with_relative_axes(&AttributeSet::from_iter([
                RelativeAxisCode::REL_X,
                RelativeAxisCode::REL_Y,
                RelativeAxisCode::REL_WHEEL,
                RelativeAxisCode::REL_HWHEEL,
            ]))?
            .build()?;

        Ok(())
    }
}
