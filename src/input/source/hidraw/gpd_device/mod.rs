use crate::{
    drivers::gpd_device::event,
    input::{
        capability::{
            Capability, Gamepad, GamepadButton, GamepadTrigger, Touch, TouchButton, Touchpad,
        },
        event::value::InputValue,
        source::EventCapability,
    },
};

pub mod macro_keyboard;
pub mod touchpad_2023;
pub mod touchpad_2024;

/// All GPD sources share the driver events, so the capability mapping is
/// shared. It covers the union of all three devices' capabilities.
impl EventCapability for event::Event {
    fn to_value(&self) -> Option<InputValue> {
        match self {
            event::Event::GamepadButton(button) => Some(InputValue::Bool(button.pressed())),
            event::Event::TouchButton(button) => Some(InputValue::Bool(button.pressed())),
            // The touchpad axis and force trigger use per-model max values,
            // so the touchpad sources normalize them in translate_event
            event::Event::TouchAxis(_) | event::Event::Trigger(_) => None,
        }
    }

    fn to_capability(&self) -> Capability {
        match self {
            event::Event::GamepadButton(button) => match button {
                event::GamepadButtonEvent::L4(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftPaddle1))
                }
                event::GamepadButtonEvent::R4(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::RightPaddle1))
                }
            },
            event::Event::TouchAxis(_) => Capability::Touchpad(Touchpad::RightPad(Touch::Motion)),
            event::Event::TouchButton(button) => match button {
                event::TouchButtonEvent::Left(_) => {
                    Capability::Touchpad(Touchpad::RightPad(Touch::Button(TouchButton::Press)))
                }
            },
            event::Event::Trigger(_) => {
                Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::RightTouchpadForce))
            }
        }
    }

    fn from_capability(capability: &Capability) -> Option<Self> {
        match capability {
            Capability::Gamepad(Gamepad::Button(button)) => {
                let event = match button {
                    GamepadButton::LeftPaddle1 => event::GamepadButtonEvent::L4(Default::default()),
                    GamepadButton::RightPaddle1 => {
                        event::GamepadButtonEvent::R4(Default::default())
                    }
                    _ => return None,
                };
                Some(event::Event::GamepadButton(event))
            }
            Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::RightTouchpadForce)) => Some(
                event::Event::Trigger(event::TriggerEvent::PadForce(Default::default())),
            ),
            Capability::Touchpad(Touchpad::RightPad(Touch::Motion)) => {
                Some(event::Event::TouchAxis(Default::default()))
            }
            Capability::Touchpad(Touchpad::RightPad(Touch::Button(TouchButton::Press))) => Some(
                event::Event::TouchButton(event::TouchButtonEvent::Left(Default::default())),
            ),
            _ => None,
        }
    }
}
