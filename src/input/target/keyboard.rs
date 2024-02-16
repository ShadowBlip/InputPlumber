use std::{collections::HashMap, error::Error};

use evdev::{
    uinput::{VirtualDevice, VirtualDeviceBuilder},
    AbsInfo, AbsoluteAxisCode, AttributeSet, InputEvent, KeyCode, SynchronizationCode,
    SynchronizationEvent,
};
use tokio::sync::{broadcast, mpsc};

use crate::input::{
    composite_device,
    event::{evdev::EvdevEvent, native::NativeEvent},
};

#[derive(Debug)]
pub struct KeyboardDevice {
    dbus_path: Option<String>,
    tx: mpsc::Sender<NativeEvent>,
    rx: mpsc::Receiver<NativeEvent>,
    _composite_tx: broadcast::Sender<composite_device::Command>,
}

impl KeyboardDevice {
    pub fn new(composite_tx: broadcast::Sender<composite_device::Command>) -> Self {
        let (tx, rx) = mpsc::channel(1024);
        Self {
            dbus_path: None,
            _composite_tx: composite_tx,
            tx,
            rx,
        }
    }

    /// Returns the DBus path of this device
    pub fn get_dbus_path(&self) -> Option<String> {
        self.dbus_path.clone()
    }

    /// Creates and runs the target device
    pub async fn run(&mut self) -> Result<(), Box<dyn Error>> {
        log::debug!("Creating virtual keyboard");
        let mut device = self.create_virtual_device()?;
        let axis_map = HashMap::new();

        // Listen for send events
        log::debug!("Started listening for events to send");
        while let Some(event) = self.rx.recv().await {
            //log::debug!("Got event to emit: {:?}", event);
            let evdev_events = self.translate_event(event, axis_map.clone());
            device.emit(evdev_events.as_slice())?;
            device.emit(&[SynchronizationEvent::new(SynchronizationCode::SYN_REPORT, 0).into()])?;
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

        let mut device = VirtualDeviceBuilder::new()?
            .name("InputPlumber Keyboard")
            .with_keys(&keys)?
            .build()?;

        Ok(device)
    }
}
