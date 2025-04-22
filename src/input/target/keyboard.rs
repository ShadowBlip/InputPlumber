use std::{collections::HashMap, error::Error};

use evdev::{
    uinput::{VirtualDevice, VirtualDeviceBuilder},
    AbsInfo, AbsoluteAxisCode, AttributeSet, InputEvent, KeyCode,
};
use zbus::Connection;

use crate::{
    dbus::interface::target::{keyboard::TargetKeyboardInterface, TargetInterface},
    input::{
        capability::{Capability, Keyboard},
        event::{evdev::EvdevEvent, native::NativeEvent},
    },
};

use super::{
    client::TargetDeviceClient, InputError, TargetDeviceTypeId, TargetInputDevice,
    TargetOutputDevice,
};

#[derive(Debug)]
pub struct KeyboardDevice {
    device: VirtualDevice,
    axis_map: HashMap<AbsoluteAxisCode, AbsInfo>,
}

impl KeyboardDevice {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let device = KeyboardDevice::create_virtual_device()?;
        Ok(Self {
            device,
            axis_map: HashMap::new(),
        })
    }
}

impl KeyboardDevice {
    /// Translate the given native event into an evdev event
    fn translate_event(&self, event: NativeEvent) -> Vec<InputEvent> {
        EvdevEvent::from_native_event(event, self.axis_map.clone())
            .into_iter()
            .map(|event| event.as_input_event())
            .collect()
    }

    /// Create the virtual device to emulate
    fn create_virtual_device() -> Result<VirtualDevice, Box<dyn Error>> {
        let mut keys = AttributeSet::<KeyCode>::new();
        keys.insert(KeyCode::KEY_ESC);
        keys.insert(KeyCode::KEY_1);
        keys.insert(KeyCode::KEY_2);
        keys.insert(KeyCode::KEY_3);
        keys.insert(KeyCode::KEY_4);
        keys.insert(KeyCode::KEY_5);
        keys.insert(KeyCode::KEY_6);
        keys.insert(KeyCode::KEY_7);
        keys.insert(KeyCode::KEY_8);
        keys.insert(KeyCode::KEY_9);
        keys.insert(KeyCode::KEY_0);
        keys.insert(KeyCode::KEY_MINUS);
        keys.insert(KeyCode::KEY_EQUAL);
        keys.insert(KeyCode::KEY_BACKSPACE);
        keys.insert(KeyCode::KEY_TAB);
        keys.insert(KeyCode::KEY_Q);
        keys.insert(KeyCode::KEY_W);
        keys.insert(KeyCode::KEY_E);
        keys.insert(KeyCode::KEY_R);
        keys.insert(KeyCode::KEY_T);
        keys.insert(KeyCode::KEY_Y);
        keys.insert(KeyCode::KEY_U);
        keys.insert(KeyCode::KEY_I);
        keys.insert(KeyCode::KEY_O);
        keys.insert(KeyCode::KEY_P);
        keys.insert(KeyCode::KEY_LEFTBRACE);
        keys.insert(KeyCode::KEY_RIGHTBRACE);
        keys.insert(KeyCode::KEY_ENTER);
        keys.insert(KeyCode::KEY_LEFTCTRL);
        keys.insert(KeyCode::KEY_A);
        keys.insert(KeyCode::KEY_S);
        keys.insert(KeyCode::KEY_D);
        keys.insert(KeyCode::KEY_F);
        keys.insert(KeyCode::KEY_G);
        keys.insert(KeyCode::KEY_H);
        keys.insert(KeyCode::KEY_J);
        keys.insert(KeyCode::KEY_K);
        keys.insert(KeyCode::KEY_L);
        keys.insert(KeyCode::KEY_SEMICOLON);
        keys.insert(KeyCode::KEY_APOSTROPHE);
        keys.insert(KeyCode::KEY_GRAVE);
        keys.insert(KeyCode::KEY_LEFTSHIFT);
        keys.insert(KeyCode::KEY_BACKSLASH);
        keys.insert(KeyCode::KEY_Z);
        keys.insert(KeyCode::KEY_X);
        keys.insert(KeyCode::KEY_C);
        keys.insert(KeyCode::KEY_V);
        keys.insert(KeyCode::KEY_B);
        keys.insert(KeyCode::KEY_N);
        keys.insert(KeyCode::KEY_M);
        keys.insert(KeyCode::KEY_COMMA);
        keys.insert(KeyCode::KEY_DOT);
        keys.insert(KeyCode::KEY_SLASH);
        keys.insert(KeyCode::KEY_RIGHTSHIFT);
        keys.insert(KeyCode::KEY_KPASTERISK);
        keys.insert(KeyCode::KEY_LEFTALT);
        keys.insert(KeyCode::KEY_SPACE);
        keys.insert(KeyCode::KEY_CAPSLOCK);
        keys.insert(KeyCode::KEY_F1);
        keys.insert(KeyCode::KEY_F2);
        keys.insert(KeyCode::KEY_F3);
        keys.insert(KeyCode::KEY_F4);
        keys.insert(KeyCode::KEY_F5);
        keys.insert(KeyCode::KEY_F6);
        keys.insert(KeyCode::KEY_F7);
        keys.insert(KeyCode::KEY_F8);
        keys.insert(KeyCode::KEY_F9);
        keys.insert(KeyCode::KEY_F10);
        keys.insert(KeyCode::KEY_NUMLOCK);
        keys.insert(KeyCode::KEY_SCROLLLOCK);
        keys.insert(KeyCode::KEY_KP7);
        keys.insert(KeyCode::KEY_KP8);
        keys.insert(KeyCode::KEY_KP9);
        keys.insert(KeyCode::KEY_KPMINUS);
        keys.insert(KeyCode::KEY_KP4);
        keys.insert(KeyCode::KEY_KP5);
        keys.insert(KeyCode::KEY_KP6);
        keys.insert(KeyCode::KEY_KPPLUS);
        keys.insert(KeyCode::KEY_KP1);
        keys.insert(KeyCode::KEY_KP2);
        keys.insert(KeyCode::KEY_KP3);
        keys.insert(KeyCode::KEY_KP0);
        keys.insert(KeyCode::KEY_KPDOT);
        keys.insert(KeyCode::KEY_ZENKAKUHANKAKU);
        keys.insert(KeyCode::KEY_102ND);
        keys.insert(KeyCode::KEY_F11);
        keys.insert(KeyCode::KEY_F12);
        keys.insert(KeyCode::KEY_RO);
        keys.insert(KeyCode::KEY_KATAKANA);
        keys.insert(KeyCode::KEY_HIRAGANA);
        keys.insert(KeyCode::KEY_HENKAN);
        keys.insert(KeyCode::KEY_KATAKANAHIRAGANA);
        keys.insert(KeyCode::KEY_MUHENKAN);
        keys.insert(KeyCode::KEY_KPJPCOMMA);
        keys.insert(KeyCode::KEY_KPENTER);
        keys.insert(KeyCode::KEY_RIGHTCTRL);
        keys.insert(KeyCode::KEY_KPSLASH);
        keys.insert(KeyCode::KEY_SYSRQ);
        keys.insert(KeyCode::KEY_RIGHTALT);
        keys.insert(KeyCode::KEY_HOME);
        keys.insert(KeyCode::KEY_UP);
        keys.insert(KeyCode::KEY_PAGEUP);
        keys.insert(KeyCode::KEY_LEFT);
        keys.insert(KeyCode::KEY_RIGHT);
        keys.insert(KeyCode::KEY_END);
        keys.insert(KeyCode::KEY_DOWN);
        keys.insert(KeyCode::KEY_PAGEDOWN);
        keys.insert(KeyCode::KEY_INSERT);
        keys.insert(KeyCode::KEY_DELETE);
        keys.insert(KeyCode::KEY_MUTE);
        keys.insert(KeyCode::KEY_VOLUMEDOWN);
        keys.insert(KeyCode::KEY_VOLUMEUP);
        keys.insert(KeyCode::KEY_POWER);
        keys.insert(KeyCode::KEY_KPEQUAL);
        keys.insert(KeyCode::KEY_PAUSE);
        keys.insert(KeyCode::KEY_KPCOMMA);
        keys.insert(KeyCode::KEY_HANJA);
        keys.insert(KeyCode::KEY_YEN);
        keys.insert(KeyCode::KEY_LEFTMETA);
        keys.insert(KeyCode::KEY_RIGHTMETA);
        keys.insert(KeyCode::KEY_COMPOSE);
        keys.insert(KeyCode::KEY_STOP);
        keys.insert(KeyCode::KEY_AGAIN);
        keys.insert(KeyCode::KEY_PROPS);
        keys.insert(KeyCode::KEY_UNDO);
        keys.insert(KeyCode::KEY_FRONT);
        keys.insert(KeyCode::KEY_COPY);
        keys.insert(KeyCode::KEY_OPEN);
        keys.insert(KeyCode::KEY_PASTE);
        keys.insert(KeyCode::KEY_FIND);
        keys.insert(KeyCode::KEY_CUT);
        keys.insert(KeyCode::KEY_HELP);
        keys.insert(KeyCode::KEY_CALC);
        keys.insert(KeyCode::KEY_SLEEP);
        keys.insert(KeyCode::KEY_WWW);
        keys.insert(KeyCode::KEY_BACK);
        keys.insert(KeyCode::KEY_FORWARD);
        keys.insert(KeyCode::KEY_EJECTCD);
        keys.insert(KeyCode::KEY_NEXTSONG);
        keys.insert(KeyCode::KEY_PLAYPAUSE);
        keys.insert(KeyCode::KEY_PREVIOUSSONG);
        keys.insert(KeyCode::KEY_STOPCD);
        keys.insert(KeyCode::KEY_REFRESH);
        keys.insert(KeyCode::KEY_EDIT);
        keys.insert(KeyCode::KEY_SCROLLUP);
        keys.insert(KeyCode::KEY_SCROLLDOWN);
        keys.insert(KeyCode::KEY_KPLEFTPAREN);
        keys.insert(KeyCode::KEY_KPRIGHTPAREN);
        keys.insert(KeyCode::KEY_BRIGHTNESSDOWN);
        keys.insert(KeyCode::KEY_BRIGHTNESSUP);
        keys.insert(KeyCode::KEY_F13);
        keys.insert(KeyCode::KEY_F14);
        keys.insert(KeyCode::KEY_F15);
        keys.insert(KeyCode::KEY_F16);
        keys.insert(KeyCode::KEY_F17);
        keys.insert(KeyCode::KEY_F18);
        keys.insert(KeyCode::KEY_F19);
        keys.insert(KeyCode::KEY_F20);
        keys.insert(KeyCode::KEY_F21);
        keys.insert(KeyCode::KEY_F22);
        keys.insert(KeyCode::KEY_F23);
        keys.insert(KeyCode::KEY_F24);
        keys.insert(KeyCode::KEY_PROG1);

        let device = VirtualDeviceBuilder::new()?
            .name("InputPlumber Keyboard")
            .with_keys(&keys)?
            .build()?;

        Ok(device)
    }
}

impl TargetInputDevice for KeyboardDevice {
    fn start_dbus_interface(
        &mut self,
        dbus: Connection,
        path: String,
        client: TargetDeviceClient,
        type_id: TargetDeviceTypeId,
    ) {
        log::debug!("Starting dbus interface: {path}");
        tokio::task::spawn(async move {
            let generic_interface = TargetInterface::new(&type_id);
            let iface = TargetKeyboardInterface::new(client);

            let object_server = dbus.object_server();
            let (gen_result, result) = tokio::join!(
                object_server.at(path.clone(), generic_interface),
                object_server.at(path.clone(), iface)
            );
            if gen_result.is_err() || result.is_err() {
                log::debug!("Failed to start dbus interface: {path} generic: {gen_result:?} type-specific: {result:?}");
            } else {
                log::debug!("Started dbus interface: {path}");
            }
        });
    }

    fn write_event(&mut self, event: NativeEvent) -> Result<(), InputError> {
        log::trace!("Received event: {event:?}");
        let evdev_events = self.translate_event(event);
        if let Err(e) = self.device.emit(evdev_events.as_slice()) {
            return Err(e.to_string().into());
        }

        Ok(())
    }

    fn get_capabilities(&self) -> Result<Vec<crate::input::capability::Capability>, InputError> {
        Ok(vec![
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
        ])
    }

    fn stop_dbus_interface(&mut self, dbus: Connection, path: String) {
        log::debug!("Stopping dbus interface for {path}");
        tokio::task::spawn(async move {
            let object_server = dbus.object_server();
            let (target, generic) = tokio::join!(
                object_server.remove::<TargetKeyboardInterface, String>(path.clone()),
                object_server.remove::<TargetInterface, String>(path.clone())
            );
            if generic.is_err() || target.is_err() {
                log::debug!("Failed to stop dbus interface: {path} generic: {generic:?} type-specific: {target:?}");
            } else {
                log::debug!("Stopped dbus interface for {path}");
            }
        });
    }
}

impl TargetOutputDevice for KeyboardDevice {}
