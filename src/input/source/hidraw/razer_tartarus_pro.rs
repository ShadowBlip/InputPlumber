use crate::config::SourceDevice;
use crate::drivers::razer_tartarus_pro::{self, driver::Driver};
use crate::input::capability::{Capability, Keyboard, Mouse, MouseButton};
use crate::input::event::{native::NativeEvent, value::InputValue};
use crate::input::source::{InputError, SourceInputDevice, SourceOutputDevice};
use crate::udev::device::UdevDevice;

use std::{error::Error, fmt::Debug};

/// RazerTartarusPro source device implementation
pub struct RazerTartarusPro {
    driver: Driver,
}

impl RazerTartarusPro {
    /// Create a new source device with the given udev
    /// device information
    pub fn new(
        device_info: UdevDevice,
        conf: Option<SourceDevice>,
    ) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let driver = Driver::new(device_info, conf)?;
        Ok(Self { driver })
    }
}

impl SourceOutputDevice for RazerTartarusPro {}

impl SourceInputDevice for RazerTartarusPro {
    /// Poll the given input device for input events
    fn poll(&mut self) -> Result<Vec<NativeEvent>, InputError> {
        let events = match self.driver.poll() {
            Ok(events) => events,
            Err(err) => {
                log::error!("Got error polling!: {err:?}");
                return Err(err.into());
            }
        };
        let native_events = translate_events(events);
        Ok(native_events)
    }

    /// Returns the possible input events this device is capable of emitting
    fn get_capabilities(&self) -> Result<Vec<Capability>, InputError> {
        Ok(CAPABILITIES.into())
    }
}

impl Debug for RazerTartarusPro {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RazerTartarusPro").finish()
    }
}

/// Translate the given events into native events
fn translate_events(events: Vec<razer_tartarus_pro::event::Event>) -> Vec<NativeEvent> {
    events.into_iter().map(translate_event).collect()
}

fn translate_event(event: razer_tartarus_pro::event::Event) -> NativeEvent {
    match event.key {
        razer_tartarus_pro::event::KeyCodes::KeyOne => NativeEvent::new(
            Capability::Keyboard(Keyboard::Key1),
            InputValue::Bool(event.pressed),
        ),
        razer_tartarus_pro::event::KeyCodes::KeyTwo => NativeEvent::new(
            Capability::Keyboard(Keyboard::Key2),
            InputValue::Bool(event.pressed),
        ),
        razer_tartarus_pro::event::KeyCodes::KeyThree => NativeEvent::new(
            Capability::Keyboard(Keyboard::Key3),
            InputValue::Bool(event.pressed),
        ),
        razer_tartarus_pro::event::KeyCodes::KeyFour => NativeEvent::new(
            Capability::Keyboard(Keyboard::Key4),
            InputValue::Bool(event.pressed),
        ),
        razer_tartarus_pro::event::KeyCodes::KeyFive => NativeEvent::new(
            Capability::Keyboard(Keyboard::Key5),
            InputValue::Bool(event.pressed),
        ),
        razer_tartarus_pro::event::KeyCodes::KeySix => NativeEvent::new(
            Capability::Keyboard(Keyboard::KeyTab),
            InputValue::Bool(event.pressed),
        ),
        razer_tartarus_pro::event::KeyCodes::KeySeven => NativeEvent::new(
            Capability::Keyboard(Keyboard::KeyQ),
            InputValue::Bool(event.pressed),
        ),
        razer_tartarus_pro::event::KeyCodes::KeyEight => NativeEvent::new(
            Capability::Keyboard(Keyboard::KeyW),
            InputValue::Bool(event.pressed),
        ),
        razer_tartarus_pro::event::KeyCodes::KeyNine => NativeEvent::new(
            Capability::Keyboard(Keyboard::KeyE),
            InputValue::Bool(event.pressed),
        ),
        razer_tartarus_pro::event::KeyCodes::KeyTen => NativeEvent::new(
            Capability::Keyboard(Keyboard::KeyR),
            InputValue::Bool(event.pressed),
        ),
        razer_tartarus_pro::event::KeyCodes::KeyEleven => NativeEvent::new(
            Capability::Keyboard(Keyboard::KeyCapslock),
            InputValue::Bool(event.pressed),
        ),
        razer_tartarus_pro::event::KeyCodes::KeyTwelve => NativeEvent::new(
            Capability::Keyboard(Keyboard::KeyA),
            InputValue::Bool(event.pressed),
        ),
        razer_tartarus_pro::event::KeyCodes::KeyThirteen => NativeEvent::new(
            Capability::Keyboard(Keyboard::KeyS),
            InputValue::Bool(event.pressed),
        ),
        razer_tartarus_pro::event::KeyCodes::KeyFourteen => NativeEvent::new(
            Capability::Keyboard(Keyboard::KeyD),
            InputValue::Bool(event.pressed),
        ),
        razer_tartarus_pro::event::KeyCodes::KeyFifteen => NativeEvent::new(
            Capability::Keyboard(Keyboard::KeyF),
            InputValue::Bool(event.pressed),
        ),
        razer_tartarus_pro::event::KeyCodes::KeySixteen => NativeEvent::new(
            Capability::Keyboard(Keyboard::KeyLeftShift),
            InputValue::Bool(event.pressed),
        ),
        razer_tartarus_pro::event::KeyCodes::KeySeventeen => NativeEvent::new(
            Capability::Keyboard(Keyboard::KeyZ),
            InputValue::Bool(event.pressed),
        ),
        razer_tartarus_pro::event::KeyCodes::KeyEighteen => NativeEvent::new(
            Capability::Keyboard(Keyboard::KeyX),
            InputValue::Bool(event.pressed),
        ),
        razer_tartarus_pro::event::KeyCodes::KeyNineteen => NativeEvent::new(
            Capability::Keyboard(Keyboard::KeyC),
            InputValue::Bool(event.pressed),
        ),
        razer_tartarus_pro::event::KeyCodes::KeyTwenty => NativeEvent::new(
            Capability::Keyboard(Keyboard::KeySpace),
            InputValue::Bool(event.pressed),
        ),
        razer_tartarus_pro::event::KeyCodes::Right => NativeEvent::new(
            Capability::Keyboard(Keyboard::KeyRight),
            InputValue::Bool(event.pressed),
        ),
        razer_tartarus_pro::event::KeyCodes::Left => NativeEvent::new(
            Capability::Keyboard(Keyboard::KeyLeft),
            InputValue::Bool(event.pressed),
        ),
        razer_tartarus_pro::event::KeyCodes::Down => NativeEvent::new(
            Capability::Keyboard(Keyboard::KeyDown),
            InputValue::Bool(event.pressed),
        ),
        razer_tartarus_pro::event::KeyCodes::Up => NativeEvent::new(
            Capability::Keyboard(Keyboard::KeyUp),
            InputValue::Bool(event.pressed),
        ),
        razer_tartarus_pro::event::KeyCodes::PhantomOne => NativeEvent::new(
            Capability::Keyboard(Keyboard::KeyB),
            InputValue::Bool(event.pressed),
        ),
        razer_tartarus_pro::event::KeyCodes::PhantomTwo => NativeEvent::new(
            Capability::Keyboard(Keyboard::KeyG),
            InputValue::Bool(event.pressed),
        ),
        razer_tartarus_pro::event::KeyCodes::PhantomThree => NativeEvent::new(
            Capability::Keyboard(Keyboard::KeyH),
            InputValue::Bool(event.pressed),
        ),
        razer_tartarus_pro::event::KeyCodes::PhantomFour => NativeEvent::new(
            Capability::Keyboard(Keyboard::KeyI),
            InputValue::Bool(event.pressed),
        ),
        razer_tartarus_pro::event::KeyCodes::PhantomFive => NativeEvent::new(
            Capability::Keyboard(Keyboard::KeyJ),
            InputValue::Bool(event.pressed),
        ),
        razer_tartarus_pro::event::KeyCodes::PhantomSix => NativeEvent::new(
            Capability::Keyboard(Keyboard::KeyK),
            InputValue::Bool(event.pressed),
        ),
        razer_tartarus_pro::event::KeyCodes::PhantomSeven => NativeEvent::new(
            Capability::Keyboard(Keyboard::KeyL),
            InputValue::Bool(event.pressed),
        ),
        razer_tartarus_pro::event::KeyCodes::PhantomEight => NativeEvent::new(
            Capability::Keyboard(Keyboard::KeyM),
            InputValue::Bool(event.pressed),
        ),
        razer_tartarus_pro::event::KeyCodes::PhantomNine => NativeEvent::new(
            Capability::Keyboard(Keyboard::KeyN),
            InputValue::Bool(event.pressed),
        ),
        razer_tartarus_pro::event::KeyCodes::PhantomTen => NativeEvent::new(
            Capability::Keyboard(Keyboard::KeyO),
            InputValue::Bool(event.pressed),
        ),
        razer_tartarus_pro::event::KeyCodes::PhantomEleven => NativeEvent::new(
            Capability::Keyboard(Keyboard::KeyP),
            InputValue::Bool(event.pressed),
        ),
        razer_tartarus_pro::event::KeyCodes::PhantomTwelve => NativeEvent::new(
            Capability::Keyboard(Keyboard::KeyT),
            InputValue::Bool(event.pressed),
        ),
        razer_tartarus_pro::event::KeyCodes::PhantomThirteen => NativeEvent::new(
            Capability::Keyboard(Keyboard::KeyU),
            InputValue::Bool(event.pressed),
        ),
        razer_tartarus_pro::event::KeyCodes::PhantomFourteen => NativeEvent::new(
            Capability::Keyboard(Keyboard::KeyV),
            InputValue::Bool(event.pressed),
        ),
        razer_tartarus_pro::event::KeyCodes::PhantomFifteen => NativeEvent::new(
            Capability::Keyboard(Keyboard::KeyY),
            InputValue::Bool(event.pressed),
        ),
        razer_tartarus_pro::event::KeyCodes::PhantomSixteen => NativeEvent::new(
            Capability::Keyboard(Keyboard::Key6),
            InputValue::Bool(event.pressed),
        ),
        razer_tartarus_pro::event::KeyCodes::PhantomSeventeen => NativeEvent::new(
            Capability::Keyboard(Keyboard::Key7),
            InputValue::Bool(event.pressed),
        ),
        razer_tartarus_pro::event::KeyCodes::PhantomEighteen => NativeEvent::new(
            Capability::Keyboard(Keyboard::Key8),
            InputValue::Bool(event.pressed),
        ),
        razer_tartarus_pro::event::KeyCodes::PhantomNineteen => NativeEvent::new(
            Capability::Keyboard(Keyboard::Key9),
            InputValue::Bool(event.pressed),
        ),
        razer_tartarus_pro::event::KeyCodes::PhantomTwenty => NativeEvent::new(
            Capability::Keyboard(Keyboard::Key0),
            InputValue::Bool(event.pressed),
        ),
        razer_tartarus_pro::event::KeyCodes::PhantomAux => NativeEvent::new(
            Capability::Keyboard(Keyboard::KeyLeftAlt),
            InputValue::Bool(event.pressed),
        ),
        razer_tartarus_pro::event::KeyCodes::PhantomMClick => NativeEvent::new(
            Capability::Mouse(Mouse::Button(MouseButton::Middle)),
            InputValue::Bool(event.pressed),
        ),
        razer_tartarus_pro::event::KeyCodes::ScrollDown => {
            NativeEvent::new(Capability::Mouse(Mouse::Wheel), InputValue::Float(-1.0))
        }
        razer_tartarus_pro::event::KeyCodes::ScrollUp => {
            NativeEvent::new(Capability::Mouse(Mouse::Wheel), InputValue::Float(1.0))
        }
        razer_tartarus_pro::event::KeyCodes::PhantomBlank => {
            NativeEvent::new(Capability::NotImplemented, InputValue::None)
        }
    }
}

/// List of all capabilities that the Razer Tartarus Pro implements
pub const CAPABILITIES: &[Capability] = &[
    Capability::Keyboard(Keyboard::Key1),
    Capability::Keyboard(Keyboard::Key2),
    Capability::Keyboard(Keyboard::Key3),
    Capability::Keyboard(Keyboard::Key4),
    Capability::Keyboard(Keyboard::Key5),
    Capability::Keyboard(Keyboard::Key6),
    Capability::Keyboard(Keyboard::Key7),
    Capability::Keyboard(Keyboard::Key8),
    Capability::Keyboard(Keyboard::Key9),
    Capability::Keyboard(Keyboard::Key0),
    Capability::Keyboard(Keyboard::KeyA),
    Capability::Keyboard(Keyboard::KeyB),
    Capability::Keyboard(Keyboard::KeyC),
    Capability::Keyboard(Keyboard::KeyD),
    Capability::Keyboard(Keyboard::KeyE),
    Capability::Keyboard(Keyboard::KeyF),
    Capability::Keyboard(Keyboard::KeyG),
    Capability::Keyboard(Keyboard::KeyH),
    Capability::Keyboard(Keyboard::KeyI),
    Capability::Keyboard(Keyboard::KeyJ),
    Capability::Keyboard(Keyboard::KeyK),
    Capability::Keyboard(Keyboard::KeyL),
    Capability::Keyboard(Keyboard::KeyM),
    Capability::Keyboard(Keyboard::KeyN),
    Capability::Keyboard(Keyboard::KeyO),
    Capability::Keyboard(Keyboard::KeyP),
    Capability::Keyboard(Keyboard::KeyQ),
    Capability::Keyboard(Keyboard::KeyR),
    Capability::Keyboard(Keyboard::KeyS),
    Capability::Keyboard(Keyboard::KeyT),
    Capability::Keyboard(Keyboard::KeyU),
    Capability::Keyboard(Keyboard::KeyV),
    Capability::Keyboard(Keyboard::KeyW),
    Capability::Keyboard(Keyboard::KeyX),
    Capability::Keyboard(Keyboard::KeyY),
    Capability::Keyboard(Keyboard::KeyZ),
    Capability::Keyboard(Keyboard::KeyLeftShift),
    Capability::Keyboard(Keyboard::KeyLeftAlt),
    Capability::Keyboard(Keyboard::KeySpace),
    Capability::Keyboard(Keyboard::KeyCapslock),
    Capability::Keyboard(Keyboard::KeyTab),
    Capability::Keyboard(Keyboard::KeyUp),
    Capability::Keyboard(Keyboard::KeyLeft),
    Capability::Keyboard(Keyboard::KeyRight),
    Capability::Keyboard(Keyboard::KeyDown),
    Capability::Mouse(Mouse::Button(MouseButton::WheelUp)),
    Capability::Mouse(Mouse::Button(MouseButton::WheelDown)),
    Capability::Mouse(Mouse::Button(MouseButton::Middle)),
    Capability::NotImplemented,
];
