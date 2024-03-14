use std::{collections::HashMap, error::Error};

use evdev::{
    uinput::{VirtualDevice, VirtualDeviceBuilder},
    AbsInfo, AbsoluteAxisCode, AttributeSet, InputEvent, KeyCode, SynchronizationCode,
    SynchronizationEvent,
};
use tokio::sync::{broadcast, mpsc};
use zbus::{fdo, Connection};
use zbus_macros::dbus_interface;

use crate::input::{
    capability::{Capability, Keyboard},
    composite_device,
    event::{
        evdev::EvdevEvent,
        native::{InputValue, NativeEvent},
    },
};

use super::TargetCommand;

const BUFFER_SIZE: usize = 2048;

/// The [DBusInterface] provides a DBus interface that can be exposed for managing
/// a [KeyboardDevice]. It works by sending command messages to a channel that the
/// [KeyboardDevice] is listening on.
pub struct DBusInterface {
    command_tx: mpsc::Sender<TargetCommand>,
}

impl DBusInterface {
    fn new(command_tx: mpsc::Sender<TargetCommand>) -> DBusInterface {
        DBusInterface { command_tx }
    }
}

#[dbus_interface(name = "org.shadowblip.Input.Keyboard")]
impl DBusInterface {
    /// Name of the composite device
    #[dbus_interface(property)]
    async fn name(&self) -> fdo::Result<String> {
        Ok("Keyboard".into())
    }

    /// Send the given key to the virtual keyboard
    async fn send_key(&self, key: String, value: bool) -> fdo::Result<()> {
        // Create a NativeEvent to send to the keyboard
        let capability = capability_from_key_string(key.as_str());
        if matches!(capability, Capability::NotImplemented) {
            return Err(fdo::Error::NotSupported("Invalid key code".into()));
        }
        let value = InputValue::Bool(value);
        let event = NativeEvent::new(capability, value);

        // Write the event to the virtual device
        self.command_tx
            .send(TargetCommand::WriteEvent(event))
            .await
            .map_err(|err| fdo::Error::Failed(err.to_string()))?;

        Ok(())
    }
}

#[derive(Debug)]
pub struct KeyboardDevice {
    conn: Connection,
    dbus_path: Option<String>,
    tx: mpsc::Sender<TargetCommand>,
    rx: mpsc::Receiver<TargetCommand>,
    _composite_tx: Option<broadcast::Sender<composite_device::Command>>,
}

impl KeyboardDevice {
    pub fn new(conn: Connection) -> Self {
        let (tx, rx) = mpsc::channel(BUFFER_SIZE);
        Self {
            conn,
            dbus_path: None,
            _composite_tx: None,
            tx,
            rx,
        }
    }

    /// Returns the DBus path of this device
    pub fn get_dbus_path(&self) -> Option<String> {
        self.dbus_path.clone()
    }

    /// Returns a transmitter channel that can be used to send events to this device
    pub fn transmitter(&self) -> mpsc::Sender<TargetCommand> {
        self.tx.clone()
    }

    /// Creates a new instance of the device interface on DBus.
    pub async fn listen_on_dbus(&mut self, path: String) -> Result<(), Box<dyn Error>> {
        let conn = self.conn.clone();
        self.dbus_path = Some(path.clone());
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let iface = DBusInterface::new(tx);
            if let Err(e) = conn.object_server().at(path, iface).await {
                log::error!("Failed to setup DBus interface for device: {:?}", e);
            }
        });
        Ok(())
    }

    /// Creates and runs the target device
    pub async fn run(&mut self) -> Result<(), Box<dyn Error>> {
        log::debug!("Creating virtual keyboard");
        let mut device = self.create_virtual_device()?;
        let axis_map = HashMap::new();

        // Listen for send events
        log::debug!("Started listening for events to send");
        while let Some(command) = self.rx.recv().await {
            match command {
                TargetCommand::WriteEvent(event) => {
                    //log::debug!("Got event to emit: {:?}", event);
                    let evdev_events = self.translate_event(event, axis_map.clone());
                    device.emit(evdev_events.as_slice())?;
                    device.emit(&[SynchronizationEvent::new(
                        SynchronizationCode::SYN_REPORT,
                        0,
                    )
                    .into()])?;
                }
                TargetCommand::Stop => break,
            };
        }

        log::debug!("Stopping device");

        // Remove the DBus interface
        if let Some(path) = self.dbus_path.clone() {
            log::debug!("Removing DBus interface");
            self.conn
                .object_server()
                .remove::<DBusInterface, String>(path)
                .await?;
        }

        Ok(())
    }

    /// Translate the given native event into an evdev event
    fn translate_event(
        &self,
        event: NativeEvent,
        axis_map: HashMap<AbsoluteAxisCode, AbsInfo>,
    ) -> Vec<InputEvent> {
        EvdevEvent::from_native_event(event, axis_map)
            .into_iter()
            .map(|event| event.as_input_event())
            .collect()
    }

    /// Create the virtual device to emulate
    fn create_virtual_device(&self) -> Result<VirtualDevice, Box<dyn Error>> {
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
