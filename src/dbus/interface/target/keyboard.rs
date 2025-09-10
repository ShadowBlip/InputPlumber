use zbus::{fdo, message::Header};
use zbus_macros::interface;

use crate::{
    dbus::{interface::Unregisterable, polkit::check_polkit},
    input::{
        capability::{Capability, Keyboard},
        event::{native::NativeEvent, value::InputValue},
        target::client::TargetDeviceClient,
    },
};

/// The [DBusInterface] provides a DBus interface that can be exposed for managing
/// a [KeyboardDevice]. It works by sending command messages to a channel that the
/// [KeyboardDevice] is listening on.
pub struct TargetKeyboardInterface {
    target_device: TargetDeviceClient,
}

impl TargetKeyboardInterface {
    pub fn new(target_device: TargetDeviceClient) -> TargetKeyboardInterface {
        TargetKeyboardInterface { target_device }
    }
}

#[interface(name = "org.shadowblip.Input.Keyboard")]
impl TargetKeyboardInterface {
    /// Name of the composite device
    #[zbus(property)]
    async fn name(&self, #[zbus(header)] hdr: Option<Header<'_>>) -> fdo::Result<String> {
        check_polkit(hdr, "org.shadowblip.Input.Keyboard.Name").await?;
        Ok("Keyboard".into())
    }

    /// Send the given key to the virtual keyboard
    async fn send_key(
        &self,
        key: String,
        value: bool,
        #[zbus(header)] hdr: Header<'_>,
    ) -> fdo::Result<()> {
        check_polkit(Some(hdr), "org.shadowblip.Input.Keyboard.SendKey").await?;
        // Create a NativeEvent to send to the keyboard
        let capability = capability_from_key_string(key.as_str());
        if matches!(capability, Capability::NotImplemented) {
            return Err(fdo::Error::NotSupported("Invalid key code".into()));
        }
        let value = InputValue::Bool(value);
        let event = NativeEvent::new(capability, value);

        // Write the event to the virtual device
        self.target_device
            .write_event(event)
            .await
            .map_err(|err| fdo::Error::Failed(err.to_string()))?;

        Ok(())
    }
}

/// Returns an input device capability from the given key string.
fn capability_from_key_string(key: &str) -> Capability {
    match key {
        "KEY_ESC" => Capability::Keyboard(Keyboard::KeyEsc),
        "KEY_1" => Capability::Keyboard(Keyboard::Key1),
        "KEY_2" => Capability::Keyboard(Keyboard::Key2),
        "KEY_3" => Capability::Keyboard(Keyboard::Key3),
        "KEY_4" => Capability::Keyboard(Keyboard::Key4),
        "KEY_5" => Capability::Keyboard(Keyboard::Key5),
        "KEY_6" => Capability::Keyboard(Keyboard::Key6),
        "KEY_7" => Capability::Keyboard(Keyboard::Key7),
        "KEY_8" => Capability::Keyboard(Keyboard::Key8),
        "KEY_9" => Capability::Keyboard(Keyboard::Key9),
        "KEY_0" => Capability::Keyboard(Keyboard::Key0),
        "KEY_MINUS" => Capability::Keyboard(Keyboard::KeyMinus),
        "KEY_EQUAL" => Capability::Keyboard(Keyboard::KeyEqual),
        "KEY_BACKSPACE" => Capability::Keyboard(Keyboard::KeyBackspace),
        "KEY_TAB" => Capability::Keyboard(Keyboard::KeyTab),
        "KEY_Q" => Capability::Keyboard(Keyboard::KeyQ),
        "KEY_W" => Capability::Keyboard(Keyboard::KeyW),
        "KEY_E" => Capability::Keyboard(Keyboard::KeyE),
        "KEY_R" => Capability::Keyboard(Keyboard::KeyR),
        "KEY_T" => Capability::Keyboard(Keyboard::KeyT),
        "KEY_Y" => Capability::Keyboard(Keyboard::KeyY),
        "KEY_U" => Capability::Keyboard(Keyboard::KeyU),
        "KEY_I" => Capability::Keyboard(Keyboard::KeyI),
        "KEY_O" => Capability::Keyboard(Keyboard::KeyO),
        "KEY_P" => Capability::Keyboard(Keyboard::KeyP),
        "KEY_LEFTBRACE" => Capability::Keyboard(Keyboard::KeyLeftBrace),
        "KEY_RIGHTBRACE" => Capability::Keyboard(Keyboard::KeyRightBrace),
        "KEY_ENTER" => Capability::Keyboard(Keyboard::KeyEnter),
        "KEY_LEFTCTRL" => Capability::Keyboard(Keyboard::KeyLeftCtrl),
        "KEY_A" => Capability::Keyboard(Keyboard::KeyA),
        "KEY_S" => Capability::Keyboard(Keyboard::KeyS),
        "KEY_D" => Capability::Keyboard(Keyboard::KeyD),
        "KEY_F" => Capability::Keyboard(Keyboard::KeyF),
        "KEY_G" => Capability::Keyboard(Keyboard::KeyG),
        "KEY_H" => Capability::Keyboard(Keyboard::KeyH),
        "KEY_J" => Capability::Keyboard(Keyboard::KeyJ),
        "KEY_K" => Capability::Keyboard(Keyboard::KeyK),
        "KEY_L" => Capability::Keyboard(Keyboard::KeyL),
        "KEY_SEMICOLON" => Capability::Keyboard(Keyboard::KeySemicolon),
        "KEY_APOSTROPHE" => Capability::Keyboard(Keyboard::KeyApostrophe),
        "KEY_GRAVE" => Capability::Keyboard(Keyboard::KeyGrave),
        "KEY_LEFTSHIFT" => Capability::Keyboard(Keyboard::KeyLeftShift),
        "KEY_BACKSLASH" => Capability::Keyboard(Keyboard::KeyBackslash),
        "KEY_Z" => Capability::Keyboard(Keyboard::KeyZ),
        "KEY_X" => Capability::Keyboard(Keyboard::KeyX),
        "KEY_C" => Capability::Keyboard(Keyboard::KeyC),
        "KEY_V" => Capability::Keyboard(Keyboard::KeyV),
        "KEY_B" => Capability::Keyboard(Keyboard::KeyB),
        "KEY_N" => Capability::Keyboard(Keyboard::KeyN),
        "KEY_M" => Capability::Keyboard(Keyboard::KeyM),
        "KEY_COMMA" => Capability::Keyboard(Keyboard::KeyComma),
        "KEY_DOT" => Capability::Keyboard(Keyboard::KeyDot),
        "KEY_SLASH" => Capability::Keyboard(Keyboard::KeySlash),
        "KEY_RIGHTSHIFT" => Capability::Keyboard(Keyboard::KeyRightShift),
        "KEY_KPASTERISK" => Capability::Keyboard(Keyboard::KeyKpAsterisk),
        "KEY_LEFTALT" => Capability::Keyboard(Keyboard::KeyLeftAlt),
        "KEY_SPACE" => Capability::Keyboard(Keyboard::KeySpace),
        "KEY_CAPSLOCK" => Capability::Keyboard(Keyboard::KeyCapslock),
        "KEY_F1" => Capability::Keyboard(Keyboard::KeyF1),
        "KEY_F2" => Capability::Keyboard(Keyboard::KeyF2),
        "KEY_F3" => Capability::Keyboard(Keyboard::KeyF3),
        "KEY_F4" => Capability::Keyboard(Keyboard::KeyF4),
        "KEY_F5" => Capability::Keyboard(Keyboard::KeyF5),
        "KEY_F6" => Capability::Keyboard(Keyboard::KeyF6),
        "KEY_F7" => Capability::Keyboard(Keyboard::KeyF7),
        "KEY_F8" => Capability::Keyboard(Keyboard::KeyF8),
        "KEY_F9" => Capability::Keyboard(Keyboard::KeyF9),
        "KEY_F10" => Capability::Keyboard(Keyboard::KeyF10),
        "KEY_NUMLOCK" => Capability::Keyboard(Keyboard::KeyNumlock),
        "KEY_SCROLLLOCK" => Capability::Keyboard(Keyboard::KeyScrollLock),
        "KEY_KP7" => Capability::Keyboard(Keyboard::KeyKp7),
        "KEY_KP8" => Capability::Keyboard(Keyboard::KeyKp8),
        "KEY_KP9" => Capability::Keyboard(Keyboard::KeyKp9),
        "KEY_KPMINUS" => Capability::Keyboard(Keyboard::KeyKpMinus),
        "KEY_KP4" => Capability::Keyboard(Keyboard::KeyKp4),
        "KEY_KP5" => Capability::Keyboard(Keyboard::KeyKp5),
        "KEY_KP6" => Capability::Keyboard(Keyboard::KeyKp6),
        "KEY_KPPLUS" => Capability::Keyboard(Keyboard::KeyKpPlus),
        "KEY_KP1" => Capability::Keyboard(Keyboard::KeyKp1),
        "KEY_KP2" => Capability::Keyboard(Keyboard::KeyKp2),
        "KEY_KP3" => Capability::Keyboard(Keyboard::KeyKp3),
        "KEY_KP0" => Capability::Keyboard(Keyboard::KeyKp0),
        "KEY_KPDOT" => Capability::Keyboard(Keyboard::KeyKpDot),
        "KEY_ZENKAKUHANKAKU" => Capability::Keyboard(Keyboard::KeyZenkakuhankaku),
        "KEY_102ND" => Capability::Keyboard(Keyboard::Key102nd),
        "KEY_F11" => Capability::Keyboard(Keyboard::KeyF11),
        "KEY_F12" => Capability::Keyboard(Keyboard::KeyF12),
        "KEY_RO" => Capability::Keyboard(Keyboard::KeyRo),
        "KEY_KATAKANA" => Capability::Keyboard(Keyboard::KeyKatakana),
        "KEY_HIRAGANA" => Capability::Keyboard(Keyboard::KeyHiragana),
        "KEY_HENKAN" => Capability::Keyboard(Keyboard::KeyHenkan),
        "KEY_KATAKANAHIRAGANA" => Capability::Keyboard(Keyboard::KeyKatakanaHiragana),
        "KEY_MUHENKAN" => Capability::Keyboard(Keyboard::KeyMuhenkan),
        "KEY_KPJPCOMMA" => Capability::Keyboard(Keyboard::KeyKpJpComma),
        "KEY_KPENTER" => Capability::Keyboard(Keyboard::KeyKpEnter),
        "KEY_RIGHTCTRL" => Capability::Keyboard(Keyboard::KeyRightCtrl),
        "KEY_KPSLASH" => Capability::Keyboard(Keyboard::KeyKpSlash),
        "KEY_SYSRQ" => Capability::Keyboard(Keyboard::KeySysrq),
        "KEY_RIGHTALT" => Capability::Keyboard(Keyboard::KeyRightAlt),
        "KEY_HOME" => Capability::Keyboard(Keyboard::KeyHome),
        "KEY_UP" => Capability::Keyboard(Keyboard::KeyUp),
        "KEY_PAGEUP" => Capability::Keyboard(Keyboard::KeyPageUp),
        "KEY_LEFT" => Capability::Keyboard(Keyboard::KeyLeft),
        "KEY_RIGHT" => Capability::Keyboard(Keyboard::KeyRight),
        "KEY_END" => Capability::Keyboard(Keyboard::KeyEnd),
        "KEY_DOWN" => Capability::Keyboard(Keyboard::KeyDown),
        "KEY_PAGEDOWN" => Capability::Keyboard(Keyboard::KeyPageDown),
        "KEY_INSERT" => Capability::Keyboard(Keyboard::KeyInsert),
        "KEY_DELETE" => Capability::Keyboard(Keyboard::KeyDelete),
        "KEY_MUTE" => Capability::Keyboard(Keyboard::KeyMute),
        "KEY_VOLUMEDOWN" => Capability::Keyboard(Keyboard::KeyVolumeDown),
        "KEY_VOLUMEUP" => Capability::Keyboard(Keyboard::KeyVolumeUp),
        "KEY_POWER" => Capability::Keyboard(Keyboard::KeyPower),
        "KEY_KPEQUAL" => Capability::Keyboard(Keyboard::KeyKpEqual),
        "KEY_PAUSE" => Capability::Keyboard(Keyboard::KeyPause),
        "KEY_KPCOMMA" => Capability::Keyboard(Keyboard::KeyKpComma),
        "KEY_HANJA" => Capability::Keyboard(Keyboard::KeyHanja),
        "KEY_YEN" => Capability::Keyboard(Keyboard::KeyYen),
        "KEY_LEFTMETA" => Capability::Keyboard(Keyboard::KeyLeftMeta),
        "KEY_RIGHTMETA" => Capability::Keyboard(Keyboard::KeyRightMeta),
        "KEY_COMPOSE" => Capability::Keyboard(Keyboard::KeyCompose),
        "KEY_STOP" => Capability::Keyboard(Keyboard::KeyStop),
        "KEY_AGAIN" => Capability::Keyboard(Keyboard::KeyAgain),
        "KEY_PROPS" => Capability::Keyboard(Keyboard::KeyProps),
        "KEY_UNDO" => Capability::Keyboard(Keyboard::KeyUndo),
        "KEY_FRONT" => Capability::Keyboard(Keyboard::KeyFront),
        "KEY_COPY" => Capability::Keyboard(Keyboard::KeyCopy),
        "KEY_OPEN" => Capability::Keyboard(Keyboard::KeyOpen),
        "KEY_PASTE" => Capability::Keyboard(Keyboard::KeyPaste),
        "KEY_FIND" => Capability::Keyboard(Keyboard::KeyFind),
        "KEY_CUT" => Capability::Keyboard(Keyboard::KeyCut),
        "KEY_HELP" => Capability::Keyboard(Keyboard::KeyHelp),
        "KEY_CALC" => Capability::Keyboard(Keyboard::KeyCalc),
        "KEY_SLEEP" => Capability::Keyboard(Keyboard::KeySleep),
        "KEY_WWW" => Capability::Keyboard(Keyboard::KeyWww),
        "KEY_BACK" => Capability::Keyboard(Keyboard::KeyBack),
        "KEY_FORWARD" => Capability::Keyboard(Keyboard::KeyForward),
        "KEY_EJECTCD" => Capability::Keyboard(Keyboard::KeyEjectCD),
        "KEY_NEXTSONG" => Capability::Keyboard(Keyboard::KeyNextSong),
        "KEY_PLAYPAUSE" => Capability::Keyboard(Keyboard::KeyPlayPause),
        "KEY_PREVIOUSSONG" => Capability::Keyboard(Keyboard::KeyPreviousSong),
        "KEY_STOPCD" => Capability::Keyboard(Keyboard::KeyStopCD),
        "KEY_REFRESH" => Capability::Keyboard(Keyboard::KeyRefresh),
        "KEY_EDIT" => Capability::Keyboard(Keyboard::KeyEdit),
        "KEY_SCROLLUP" => Capability::Keyboard(Keyboard::KeyScrollUp),
        "KEY_SCROLLDOWN" => Capability::Keyboard(Keyboard::KeyScrollDown),
        "KEY_KPLEFTPAREN" => Capability::Keyboard(Keyboard::KeyKpLeftParen),
        "KEY_KPRIGHTPAREN" => Capability::Keyboard(Keyboard::KeyKpRightParen),
        "KEY_BRIGHTNESSDOWN" => Capability::Keyboard(Keyboard::KeyBrightnessDown),
        "KEY_BRIGHTNESSUP" => Capability::Keyboard(Keyboard::KeyBrightnessUp),
        "KEY_F13" => Capability::Keyboard(Keyboard::KeyF13),
        "KEY_F14" => Capability::Keyboard(Keyboard::KeyF14),
        "KEY_F15" => Capability::Keyboard(Keyboard::KeyF15),
        "KEY_F16" => Capability::Keyboard(Keyboard::KeyF16),
        "KEY_F17" => Capability::Keyboard(Keyboard::KeyF17),
        "KEY_F18" => Capability::Keyboard(Keyboard::KeyF18),
        "KEY_F19" => Capability::Keyboard(Keyboard::KeyF19),
        "KEY_F20" => Capability::Keyboard(Keyboard::KeyF20),
        "KEY_F21" => Capability::Keyboard(Keyboard::KeyF21),
        "KEY_F22" => Capability::Keyboard(Keyboard::KeyF22),
        "KEY_F23" => Capability::Keyboard(Keyboard::KeyF23),
        "KEY_F24" => Capability::Keyboard(Keyboard::KeyF24),
        "KEY_PROG1" => Capability::Keyboard(Keyboard::KeyProg1),
        _ => Capability::NotImplemented,
    }
}

impl Unregisterable for TargetKeyboardInterface {}
