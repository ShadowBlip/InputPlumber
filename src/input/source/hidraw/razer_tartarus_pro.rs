use crate::drivers::razer_tartarus_pro::{driver::Driver, event};
use crate::input::capability::{
    Capability, Gamepad, GamepadAxis, GamepadButton, GamepadTrigger, Keyboard, Mouse, Touch,
    TouchButton, Touchpad,
};
use crate::input::event::{
    native::NativeEvent,
    value::InputValue,
    value::{normalize_signed_value, normalize_unsigned_value},
};
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
fn translate_events(events: Vec<event::Event>) -> Vec<NativeEvent> {
    Vec::new()
}

/// List of all capabilities that the Razer Tartarus Pro implements
pub const CAPABILITIES: &[Capability] = &[
    Capability::Keyboard(Keyboard::KeyEsc),
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
    Capability::Keyboard(Keyboard::KeyMinus),
    Capability::Keyboard(Keyboard::KeyEqual),
    Capability::Keyboard(Keyboard::KeyBackspace),
    Capability::Keyboard(Keyboard::KeyBrightnessDown),
    Capability::Keyboard(Keyboard::KeyBrightnessUp),
    Capability::Keyboard(Keyboard::KeyTab),
    Capability::Keyboard(Keyboard::KeyQ),
    Capability::Keyboard(Keyboard::KeyW),
    Capability::Keyboard(Keyboard::KeyE),
    Capability::Keyboard(Keyboard::KeyR),
    Capability::Keyboard(Keyboard::KeyT),
    Capability::Keyboard(Keyboard::KeyY),
    Capability::Keyboard(Keyboard::KeyU),
    Capability::Keyboard(Keyboard::KeyI),
    Capability::Keyboard(Keyboard::KeyO),
    Capability::Keyboard(Keyboard::KeyP),
    Capability::Keyboard(Keyboard::KeyLeftBrace),
    Capability::Keyboard(Keyboard::KeyRightBrace),
    Capability::Keyboard(Keyboard::KeyEnter),
    Capability::Keyboard(Keyboard::KeyLeftCtrl),
    Capability::Keyboard(Keyboard::KeyA),
    Capability::Keyboard(Keyboard::KeyS),
    Capability::Keyboard(Keyboard::KeyD),
    Capability::Keyboard(Keyboard::KeyF),
    Capability::Keyboard(Keyboard::KeyG),
    Capability::Keyboard(Keyboard::KeyH),
    Capability::Keyboard(Keyboard::KeyJ),
    Capability::Keyboard(Keyboard::KeyK),
    Capability::Keyboard(Keyboard::KeyL),
    Capability::Keyboard(Keyboard::KeySemicolon),
    Capability::Keyboard(Keyboard::KeyApostrophe),
    Capability::Keyboard(Keyboard::KeyGrave),
    Capability::Keyboard(Keyboard::KeyLeftShift),
    Capability::Keyboard(Keyboard::KeyBackslash),
    Capability::Keyboard(Keyboard::KeyZ),
    Capability::Keyboard(Keyboard::KeyX),
    Capability::Keyboard(Keyboard::KeyC),
    Capability::Keyboard(Keyboard::KeyV),
    Capability::Keyboard(Keyboard::KeyB),
    Capability::Keyboard(Keyboard::KeyN),
    Capability::Keyboard(Keyboard::KeyM),
    Capability::Keyboard(Keyboard::KeyComma),
    Capability::Keyboard(Keyboard::KeyDot),
    Capability::Keyboard(Keyboard::KeySlash),
    Capability::Keyboard(Keyboard::KeyRightShift),
    Capability::Keyboard(Keyboard::KeyKpAsterisk),
    Capability::Keyboard(Keyboard::KeyLeftAlt),
    Capability::Keyboard(Keyboard::KeySpace),
    Capability::Keyboard(Keyboard::KeyCapslock),
    Capability::Keyboard(Keyboard::KeyF1),
    Capability::Keyboard(Keyboard::KeyF2),
    Capability::Keyboard(Keyboard::KeyF3),
    Capability::Keyboard(Keyboard::KeyF4),
    Capability::Keyboard(Keyboard::KeyF5),
    Capability::Keyboard(Keyboard::KeyF6),
    Capability::Keyboard(Keyboard::KeyF7),
    Capability::Keyboard(Keyboard::KeyF8),
    Capability::Keyboard(Keyboard::KeyF9),
    Capability::Keyboard(Keyboard::KeyF10),
    Capability::Keyboard(Keyboard::KeyNumlock),
    Capability::Keyboard(Keyboard::KeyScrollLock),
    Capability::Keyboard(Keyboard::KeyKp7),
    Capability::Keyboard(Keyboard::KeyKp8),
    Capability::Keyboard(Keyboard::KeyKp9),
    Capability::Keyboard(Keyboard::KeyKpMinus),
    Capability::Keyboard(Keyboard::KeyKp4),
    Capability::Keyboard(Keyboard::KeyKp5),
    Capability::Keyboard(Keyboard::KeyKp6),
    Capability::Keyboard(Keyboard::KeyKpPlus),
    Capability::Keyboard(Keyboard::KeyKp1),
    Capability::Keyboard(Keyboard::KeyKp2),
    Capability::Keyboard(Keyboard::KeyKp3),
    Capability::Keyboard(Keyboard::KeyKp0),
    Capability::Keyboard(Keyboard::KeyKpDot),
    Capability::Keyboard(Keyboard::KeyZenkakuhankaku),
    Capability::Keyboard(Keyboard::Key102nd),
    Capability::Keyboard(Keyboard::KeyF11),
    Capability::Keyboard(Keyboard::KeyF12),
    Capability::Keyboard(Keyboard::KeyRo),
    Capability::Keyboard(Keyboard::KeyKatakana),
    Capability::Keyboard(Keyboard::KeyHiragana),
    Capability::Keyboard(Keyboard::KeyHenkan),
    Capability::Keyboard(Keyboard::KeyKatakanaHiragana),
    Capability::Keyboard(Keyboard::KeyMuhenkan),
    Capability::Keyboard(Keyboard::KeyKpJpComma),
    Capability::Keyboard(Keyboard::KeyKpEnter),
    Capability::Keyboard(Keyboard::KeyRightCtrl),
    Capability::Keyboard(Keyboard::KeyKpSlash),
    Capability::Keyboard(Keyboard::KeySysrq),
    Capability::Keyboard(Keyboard::KeyRightAlt),
    Capability::Keyboard(Keyboard::KeyHome),
    Capability::Keyboard(Keyboard::KeyUp),
    Capability::Keyboard(Keyboard::KeyPageUp),
    Capability::Keyboard(Keyboard::KeyLeft),
    Capability::Keyboard(Keyboard::KeyRight),
    Capability::Keyboard(Keyboard::KeyEnd),
    Capability::Keyboard(Keyboard::KeyDown),
    Capability::Keyboard(Keyboard::KeyPageDown),
    Capability::Keyboard(Keyboard::KeyInsert),
    Capability::Keyboard(Keyboard::KeyDelete),
    Capability::Keyboard(Keyboard::KeyMute),
    Capability::Keyboard(Keyboard::KeyVolumeDown),
    Capability::Keyboard(Keyboard::KeyVolumeUp),
    Capability::Keyboard(Keyboard::KeyPower),
    Capability::Keyboard(Keyboard::KeyKpEqual),
    Capability::Keyboard(Keyboard::KeyPause),
    Capability::Keyboard(Keyboard::KeyKpComma),
    Capability::Keyboard(Keyboard::KeyHanja),
    Capability::Keyboard(Keyboard::KeyYen),
    Capability::Keyboard(Keyboard::KeyLeftMeta),
    Capability::Keyboard(Keyboard::KeyRightMeta),
    Capability::Keyboard(Keyboard::KeyCompose),
    Capability::Keyboard(Keyboard::KeyStop),
    Capability::Keyboard(Keyboard::KeyAgain),
    Capability::Keyboard(Keyboard::KeyProps),
    Capability::Keyboard(Keyboard::KeyUndo),
    Capability::Keyboard(Keyboard::KeyFront),
    Capability::Keyboard(Keyboard::KeyCopy),
    Capability::Keyboard(Keyboard::KeyOpen),
    Capability::Keyboard(Keyboard::KeyPaste),
    Capability::Keyboard(Keyboard::KeyFind),
    Capability::Keyboard(Keyboard::KeyCut),
    Capability::Keyboard(Keyboard::KeyHelp),
    Capability::Keyboard(Keyboard::KeyCalc),
    Capability::Keyboard(Keyboard::KeySleep),
    Capability::Keyboard(Keyboard::KeyWww),
    Capability::Keyboard(Keyboard::KeyBack),
    Capability::Keyboard(Keyboard::KeyForward),
    Capability::Keyboard(Keyboard::KeyEjectCD),
    Capability::Keyboard(Keyboard::KeyNextSong),
    Capability::Keyboard(Keyboard::KeyPlayPause),
    Capability::Keyboard(Keyboard::KeyPreviousSong),
    Capability::Keyboard(Keyboard::KeyStopCD),
    Capability::Keyboard(Keyboard::KeyRefresh),
    Capability::Keyboard(Keyboard::KeyEdit),
    Capability::Keyboard(Keyboard::KeyScrollUp),
    Capability::Keyboard(Keyboard::KeyScrollDown),
    Capability::Keyboard(Keyboard::KeyKpLeftParen),
    Capability::Keyboard(Keyboard::KeyKpRightParen),
    Capability::Keyboard(Keyboard::KeyF13),
    Capability::Keyboard(Keyboard::KeyF14),
    Capability::Keyboard(Keyboard::KeyF15),
    Capability::Keyboard(Keyboard::KeyF16),
    Capability::Keyboard(Keyboard::KeyF17),
    Capability::Keyboard(Keyboard::KeyF18),
    Capability::Keyboard(Keyboard::KeyF19),
    Capability::Keyboard(Keyboard::KeyF20),
    Capability::Keyboard(Keyboard::KeyF21),
    Capability::Keyboard(Keyboard::KeyF22),
    Capability::Keyboard(Keyboard::KeyF23),
    Capability::Keyboard(Keyboard::KeyF24),
    Capability::Keyboard(Keyboard::KeyProg1),
    Capability::Keyboard(Keyboard::KeyProg2),
    Capability::Keyboard(Keyboard::KeyProg3),
    Capability::Keyboard(Keyboard::KeyProg4),
    Capability::Gamepad(Gamepad::Accelerometer),
    Capability::Gamepad(Gamepad::Axis(GamepadAxis::LeftStick)),
    Capability::Gamepad(Gamepad::Axis(GamepadAxis::RightStick)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::DPadDown)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::DPadLeft)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::DPadRight)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::DPadUp)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::East)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::Guide)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftBumper)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftPaddle1)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftPaddle2)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftStick)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftStickTouch)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftTrigger)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::North)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::Mute)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::RightBumper)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::RightPaddle1)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::RightPaddle2)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::RightStick)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::RightStickTouch)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::RightTrigger)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::Select)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::South)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::Start)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::West)),
    Capability::Gamepad(Gamepad::Gyro),
    Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::LeftTrigger)),
    Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::RightTrigger)),
    Capability::Touchpad(Touchpad::CenterPad(Touch::Button(TouchButton::Press))),
    Capability::Touchpad(Touchpad::CenterPad(Touch::Button(TouchButton::Touch))),
    Capability::Touchpad(Touchpad::CenterPad(Touch::Motion)),
];
