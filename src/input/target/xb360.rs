use std::error::Error;

use evdev::{
    uinput::VirtualDeviceBuilder, AbsInfo, AbsoluteAxisCode, AttributeSet, FFEffectCode, KeyCode,
    UinputAbsSetup,
};
use tokio::sync::broadcast;

use crate::input::composite_device;

#[derive(Debug)]
pub struct XBox360Controller {
    _composite_tx: Option<broadcast::Sender<composite_device::Command>>,
}

impl XBox360Controller {
    pub fn new() -> Self {
        Self {
            _composite_tx: None,
        }
    }

    pub fn run() -> Result<(), Box<dyn Error>> {
        // Setup Key inputs
        let mut keys = AttributeSet::<KeyCode>::new();
        keys.insert(KeyCode::BTN_SOUTH);
        keys.insert(KeyCode::BTN_EAST);
        keys.insert(KeyCode::BTN_NORTH);
        keys.insert(KeyCode::BTN_WEST);
        keys.insert(KeyCode::BTN_TL);
        keys.insert(KeyCode::BTN_TR);
        keys.insert(KeyCode::BTN_SELECT);
        keys.insert(KeyCode::BTN_START);
        keys.insert(KeyCode::BTN_MODE);
        keys.insert(KeyCode::BTN_THUMBL);
        keys.insert(KeyCode::BTN_THUMBR);
        keys.insert(KeyCode::BTN_TRIGGER_HAPPY1);
        keys.insert(KeyCode::BTN_TRIGGER_HAPPY2);
        keys.insert(KeyCode::BTN_TRIGGER_HAPPY3);
        keys.insert(KeyCode::BTN_TRIGGER_HAPPY4);

        // Setup ABS inputs
        let joystick_setup = AbsInfo::new(0, -32768, 32767, 16, 128, 1);
        let abs_x = UinputAbsSetup::new(AbsoluteAxisCode::ABS_X, joystick_setup);
        let abs_y = UinputAbsSetup::new(AbsoluteAxisCode::ABS_Y, joystick_setup);
        let abs_rx = UinputAbsSetup::new(AbsoluteAxisCode::ABS_RX, joystick_setup);
        let abs_ry = UinputAbsSetup::new(AbsoluteAxisCode::ABS_RY, joystick_setup);
        let triggers_setup = AbsInfo::new(0, 0, 255, 0, 0, 1);
        let abs_z = UinputAbsSetup::new(AbsoluteAxisCode::ABS_Z, triggers_setup);
        let abs_rz = UinputAbsSetup::new(AbsoluteAxisCode::ABS_RZ, triggers_setup);
        let dpad_setup = AbsInfo::new(0, -1, 1, 0, 0, 1);
        let abs_hat0x = UinputAbsSetup::new(AbsoluteAxisCode::ABS_HAT0X, dpad_setup);
        let abs_hat0y = UinputAbsSetup::new(AbsoluteAxisCode::ABS_HAT0Y, dpad_setup);

        // Setup Force Feedback
        let mut ff = AttributeSet::<FFEffectCode>::new();
        ff.insert(FFEffectCode::FF_RUMBLE);
        ff.insert(FFEffectCode::FF_PERIODIC);
        ff.insert(FFEffectCode::FF_SQUARE);
        ff.insert(FFEffectCode::FF_TRIANGLE);
        ff.insert(FFEffectCode::FF_SINE);
        ff.insert(FFEffectCode::FF_GAIN);

        // Build the device
        let _device = VirtualDeviceBuilder::new()?
            .name("Xbox 360 Wireless Receiver (XBOX)")
            .with_keys(&keys)?
            .with_absolute_axis(&abs_x)?
            .with_absolute_axis(&abs_y)?
            .with_absolute_axis(&abs_rx)?
            .with_absolute_axis(&abs_ry)?
            .with_absolute_axis(&abs_z)?
            .with_absolute_axis(&abs_rz)?
            .with_absolute_axis(&abs_hat0x)?
            .with_absolute_axis(&abs_hat0y)?
            .with_ff(&ff)?
            .build()?;

        Ok(())
    }
}
