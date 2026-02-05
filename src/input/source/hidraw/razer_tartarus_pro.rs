use crate::drivers::razer_tartarus_pro::{self, driver::Driver};
use crate::input::capability::{Capability, Keyboard, Mouse, MouseButton};
use crate::input::event::{native::NativeEvent, value::InputValue};
use crate::input::source::{InputError, OutputError, SourceInputDevice, SourceOutputDevice};
use crate::udev::device::UdevDevice;

use std::{error::Error, fmt::Debug};

/// RazerTartarusPro source device implementation
pub struct RazerTartarusPro {
    driver: Driver,
}

impl RazerTartarusPro {
    /// Create a new source device with the given udev
    /// device information
    pub fn new(device_info: UdevDevice) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let driver = Driver::new(device_info)?;
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
        razer_tartarus_pro::event::KeyCodes::Blank => {
            NativeEvent::new(Capability::NotImplemented, InputValue::None)
        }
        razer_tartarus_pro::event::KeyCodes::Sdown => //NativeEvent::new(
            //Capability::Mouse(Mouse::Button(MouseButton::WheelDown)),
            //InputValue::Bool(event.pressed),
            NativeEvent::new(Capability::NotImplemented, InputValue::None
        ),
        razer_tartarus_pro::event::KeyCodes::KeySixteen => NativeEvent::new(
            Capability::Keyboard(Keyboard::KeyLeftShift),
            InputValue::Bool(event.pressed),
        ),
        razer_tartarus_pro::event::KeyCodes::KeyTwelve => NativeEvent::new(
            Capability::Keyboard(Keyboard::KeyA),
            InputValue::Bool(event.pressed),
        ),
        razer_tartarus_pro::event::KeyCodes::KeyNineteen => NativeEvent::new(
            Capability::Keyboard(Keyboard::KeyC),
            InputValue::Bool(event.pressed),
        ),
        razer_tartarus_pro::event::KeyCodes::KeyFourteen => NativeEvent::new(
            Capability::Keyboard(Keyboard::KeyD),
            InputValue::Bool(event.pressed),
        ),
        razer_tartarus_pro::event::KeyCodes::KeyNine => NativeEvent::new(
            Capability::Keyboard(Keyboard::KeyE),
            InputValue::Bool(event.pressed),
        ),
        razer_tartarus_pro::event::KeyCodes::KeyFifteen => NativeEvent::new(
            Capability::Keyboard(Keyboard::KeyF),
            InputValue::Bool(event.pressed),
        ),
        razer_tartarus_pro::event::KeyCodes::KeySeven => NativeEvent::new(
            Capability::Keyboard(Keyboard::KeyQ),
            InputValue::Bool(event.pressed),
        ),
        razer_tartarus_pro::event::KeyCodes::KeyTen => NativeEvent::new(
            Capability::Keyboard(Keyboard::KeyR),
            InputValue::Bool(event.pressed),
        ),
        razer_tartarus_pro::event::KeyCodes::KeyThirteen => NativeEvent::new(
            Capability::Keyboard(Keyboard::KeyS),
            InputValue::Bool(event.pressed),
        ),
        razer_tartarus_pro::event::KeyCodes::KeyEight => NativeEvent::new(
            Capability::Keyboard(Keyboard::KeyW),
            InputValue::Bool(event.pressed),
        ),
        razer_tartarus_pro::event::KeyCodes::KeyEighteen => NativeEvent::new(
            Capability::Keyboard(Keyboard::KeyX),
            InputValue::Bool(event.pressed),
        ),
        razer_tartarus_pro::event::KeyCodes::KeySeventeen => NativeEvent::new(
            Capability::Keyboard(Keyboard::KeyZ),
            InputValue::Bool(event.pressed),
        ),
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
        razer_tartarus_pro::event::KeyCodes::KeyTwenty => NativeEvent::new(
            Capability::Keyboard(Keyboard::KeySpace),
            InputValue::Bool(event.pressed),
        ),
        razer_tartarus_pro::event::KeyCodes::KeyEleven => NativeEvent::new(
            Capability::Keyboard(Keyboard::KeyCapslock),
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
        razer_tartarus_pro::event::KeyCodes::Aux => NativeEvent::new(
            Capability::Keyboard(Keyboard::KeyLeftAlt),
            InputValue::Bool(event.pressed),
        ),
        razer_tartarus_pro::event::KeyCodes::MClick => NativeEvent::new(
            Capability::Mouse(Mouse::Button(MouseButton::Middle)),
            InputValue::Bool(event.pressed),
        ),
        razer_tartarus_pro::event::KeyCodes::Sup => //NativeEvent::new(
            //Capability::Mouse(Mouse::Button(MouseButton::WheelUp)),
            //InputValue::Bool(event.pressed),
            NativeEvent::new(Capability::NotImplemented, InputValue::None
        ),
    }
}

/// List of all capabilities that the Razer Tartarus Pro implements
pub const CAPABILITIES: &[Capability] = &[
    Capability::Keyboard(Keyboard::Key1),
    Capability::Keyboard(Keyboard::Key2),
    Capability::Keyboard(Keyboard::Key3),
    Capability::Keyboard(Keyboard::Key4),
    Capability::Keyboard(Keyboard::Key5),
    Capability::Keyboard(Keyboard::KeyQ),
    Capability::Keyboard(Keyboard::KeyW),
    Capability::Keyboard(Keyboard::KeyE),
    Capability::Keyboard(Keyboard::KeyR),
    Capability::Keyboard(Keyboard::KeyT),
    Capability::Keyboard(Keyboard::KeyA),
    Capability::Keyboard(Keyboard::KeyS),
    Capability::Keyboard(Keyboard::KeyD),
    Capability::Keyboard(Keyboard::KeyF),
    Capability::Keyboard(Keyboard::KeyLeftShift),
    Capability::Keyboard(Keyboard::KeyZ),
    Capability::Keyboard(Keyboard::KeyX),
    Capability::Keyboard(Keyboard::KeyC),
    Capability::Keyboard(Keyboard::KeyLeftAlt),
    Capability::Keyboard(Keyboard::KeySpace),
    Capability::Keyboard(Keyboard::KeyCapslock),
    Capability::Keyboard(Keyboard::KeyUp),
    Capability::Keyboard(Keyboard::KeyLeft),
    Capability::Keyboard(Keyboard::KeyRight),
    Capability::Keyboard(Keyboard::KeyDown),
    Capability::Mouse(Mouse::Button(MouseButton::WheelUp)),
    Capability::Mouse(Mouse::Button(MouseButton::WheelDown)),
    Capability::Mouse(Mouse::Button(MouseButton::Middle)),
    Capability::NotImplemented,
];
