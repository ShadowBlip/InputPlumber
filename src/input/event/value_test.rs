use crate::config::capability_map::CapabilityConfig;
use crate::input::capability::{
    Capability, Gamepad, GamepadButton, Keyboard, Mouse, MouseButton, Touch, TouchButton, Touchpad,
};
use crate::input::event::value::InputValue;

#[test]
fn test_touchpad_button_translations() {
    let source_cap = Capability::Touchpad(Touchpad::RightPad(Touch::Button(TouchButton::Press)));
    let source_config = CapabilityConfig::default();
    let target_config = CapabilityConfig::default();

    let input_val = InputValue::Bool(true);

    let target_cap = Capability::Mouse(Mouse::Button(MouseButton::Left));
    let res = input_val
        .translate(&source_cap, &source_config, &target_cap, &target_config)
        .expect("should translate touchpad button to mouse button");
    assert!(res.pressed());

    let target_cap = Capability::Gamepad(Gamepad::Button(GamepadButton::South));
    let res = input_val
        .translate(&source_cap, &source_config, &target_cap, &target_config)
        .expect("should translate touchpad button to gamepad button");
    assert!(res.pressed());

    let target_cap = Capability::Keyboard(Keyboard::KeyA);
    let res = input_val
        .translate(&source_cap, &source_config, &target_cap, &target_config)
        .expect("should translate touchpad button to keyboard key");
    assert!(res.pressed());

    let target_cap =
        Capability::Touchpad(Touchpad::CenterPad(Touch::Button(TouchButton::Touch)));
    let res = input_val
        .translate(&source_cap, &source_config, &target_cap, &target_config)
        .expect("should translate touchpad button to touchpad button");
    assert!(res.pressed());

    let source_cap = Capability::Touchscreen(Touch::Button(TouchButton::Press));
    let target_cap = Capability::Mouse(Mouse::Button(MouseButton::Right));
    let res = input_val
        .translate(&source_cap, &source_config, &target_cap, &target_config)
        .expect("should translate touchscreen button to mouse button");
    assert!(res.pressed());
}
