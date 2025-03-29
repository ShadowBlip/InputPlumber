use crate::{
    drivers::unified_gamepad::{capability::InputCapability, event::Event, value::Value},
    input::capability::{
        Capability, Gamepad, GamepadAxis, GamepadButton, GamepadTrigger, Touch, TouchButton,
        Touchpad,
    },
};
use packed_struct::prelude::*;

use super::{native::NativeEvent, value::InputValue};

const GYRO_SCALE_FACTOR: f64 = 10.0; // amount to scale imu data
const ACCEL_SCALE_FACTOR: f64 = 3000.0; // amount to scale imu data

impl From<Event> for NativeEvent {
    fn from(value: Event) -> Self {
        let capability = value.capability.into();
        let value = value.value.into();
        Self::new(capability, value)
    }
}

impl From<Value> for InputValue {
    fn from(value: Value) -> Self {
        match value {
            Value::None => InputValue::None,
            Value::Bool(value) => InputValue::Bool(value.value),
            Value::UInt8(value) => InputValue::Float(value.value as f64 / u8::MAX as f64),
            Value::UInt16(value) => InputValue::Float(value.value as f64 / u16::MAX as f64),
            Value::UInt16Vector2(value) => {
                // Denormalize the x and y values from 0 and u16::MAX to -1.0 -> 1.0
                let x = ((value.x as f64 / u16::MAX as f64) * 2.0) - 1.0;
                let y = ((value.y as f64 / u16::MAX as f64) * 2.0) - 1.0;
                InputValue::Vector2 {
                    x: Some(x),
                    y: Some(y),
                }
            }
            Value::Int16Vector3(value) => {
                let x = Some(value.x as f64);
                let y = Some(value.y as f64);
                let z = Some(value.z as f64);
                InputValue::Vector3 { x, y, z }
            }
            Value::Touch(value) => InputValue::Touch {
                index: value.index.to_primitive(),
                is_touching: value.is_touching,
                pressure: Some((value.pressure / u8::MAX) as f64),
                x: Some(value.x as f64 / u16::MAX as f64),
                y: Some(value.y as f64 / u16::MAX as f64),
            },
        }
    }
}

impl From<InputCapability> for Capability {
    fn from(value: InputCapability) -> Self {
        let capability = match value {
            InputCapability::None => Capability::None,
            InputCapability::KeyboardKeyEsc => Capability::NotImplemented,
            InputCapability::KeyboardKey1 => Capability::NotImplemented,
            InputCapability::KeyboardKey2 => Capability::NotImplemented,
            InputCapability::KeyboardKey3 => Capability::NotImplemented,
            InputCapability::KeyboardKey4 => Capability::NotImplemented,
            InputCapability::KeyboardKey5 => Capability::NotImplemented,
            InputCapability::KeyboardKey6 => Capability::NotImplemented,
            InputCapability::KeyboardKey7 => Capability::NotImplemented,
            InputCapability::KeyboardKey8 => Capability::NotImplemented,
            InputCapability::KeyboardKey9 => Capability::NotImplemented,
            InputCapability::KeyboardKey0 => Capability::NotImplemented,
            InputCapability::KeyboardKeyMinus => Capability::NotImplemented,
            InputCapability::KeyboardKeyEqual => Capability::NotImplemented,
            InputCapability::KeyboardKeyBackspace => Capability::NotImplemented,
            InputCapability::KeyboardKeyTab => Capability::NotImplemented,
            InputCapability::KeyboardKeyQ => Capability::NotImplemented,
            InputCapability::KeyboardKeyW => Capability::NotImplemented,
            InputCapability::KeyboardKeyE => Capability::NotImplemented,
            InputCapability::KeyboardKeyR => Capability::NotImplemented,
            InputCapability::KeyboardKeyT => Capability::NotImplemented,
            InputCapability::KeyboardKeyY => Capability::NotImplemented,
            InputCapability::KeyboardKeyU => Capability::NotImplemented,
            InputCapability::KeyboardKeyI => Capability::NotImplemented,
            InputCapability::KeyboardKeyO => Capability::NotImplemented,
            InputCapability::KeyboardKeyP => Capability::NotImplemented,
            InputCapability::KeyboardKeyLeftBrace => Capability::NotImplemented,
            InputCapability::KeyboardKeyRightBrace => Capability::NotImplemented,
            InputCapability::KeyboardKeyEnter => Capability::NotImplemented,
            InputCapability::KeyboardKeyLeftCtrl => Capability::NotImplemented,
            InputCapability::KeyboardKeyA => Capability::NotImplemented,
            InputCapability::KeyboardKeyS => Capability::NotImplemented,
            InputCapability::KeyboardKeyD => Capability::NotImplemented,
            InputCapability::KeyboardKeyF => Capability::NotImplemented,
            InputCapability::KeyboardKeyG => Capability::NotImplemented,
            InputCapability::KeyboardKeyH => Capability::NotImplemented,
            InputCapability::KeyboardKeyJ => Capability::NotImplemented,
            InputCapability::KeyboardKeyK => Capability::NotImplemented,
            InputCapability::KeyboardKeyL => Capability::NotImplemented,
            InputCapability::KeyboardKeySemicolon => Capability::NotImplemented,
            InputCapability::KeyboardKeyApostrophe => Capability::NotImplemented,
            InputCapability::KeyboardKeyGrave => Capability::NotImplemented,
            InputCapability::KeyboardKeyLeftShift => Capability::NotImplemented,
            InputCapability::KeyboardKeyBackslash => Capability::NotImplemented,
            InputCapability::KeyboardKeyZ => Capability::NotImplemented,
            InputCapability::KeyboardKeyX => Capability::NotImplemented,
            InputCapability::KeyboardKeyC => Capability::NotImplemented,
            InputCapability::KeyboardKeyV => Capability::NotImplemented,
            InputCapability::KeyboardKeyB => Capability::NotImplemented,
            InputCapability::KeyboardKeyN => Capability::NotImplemented,
            InputCapability::KeyboardKeyM => Capability::NotImplemented,
            InputCapability::KeyboardKeyComma => Capability::NotImplemented,
            InputCapability::KeyboardKeyDot => Capability::NotImplemented,
            InputCapability::KeyboardKeySlash => Capability::NotImplemented,
            InputCapability::KeyboardKeyRightShift => Capability::NotImplemented,
            InputCapability::KeyboardKeyKpAsterisk => Capability::NotImplemented,
            InputCapability::KeyboardKeyLeftAlt => Capability::NotImplemented,
            InputCapability::KeyboardKeySpace => Capability::NotImplemented,
            InputCapability::KeyboardKeyCapsLock => Capability::NotImplemented,
            InputCapability::KeyboardKeyF1 => Capability::NotImplemented,
            InputCapability::KeyboardKeyF2 => Capability::NotImplemented,
            InputCapability::KeyboardKeyF3 => Capability::NotImplemented,
            InputCapability::KeyboardKeyF4 => Capability::NotImplemented,
            InputCapability::KeyboardKeyF5 => Capability::NotImplemented,
            InputCapability::KeyboardKeyF6 => Capability::NotImplemented,
            InputCapability::KeyboardKeyF7 => Capability::NotImplemented,
            InputCapability::KeyboardKeyF8 => Capability::NotImplemented,
            InputCapability::KeyboardKeyF9 => Capability::NotImplemented,
            InputCapability::KeyboardKeyF10 => Capability::NotImplemented,
            InputCapability::KeyboardKeyNumLock => Capability::NotImplemented,
            InputCapability::KeyboardKeyScrollLock => Capability::NotImplemented,
            InputCapability::KeyboardKeyKP7 => Capability::NotImplemented,
            InputCapability::KeyboardKeyKP8 => Capability::NotImplemented,
            InputCapability::KeyboardKeyKP9 => Capability::NotImplemented,
            InputCapability::KeyboardKeyKPMinus => Capability::NotImplemented,
            InputCapability::KeyboardKeyKP4 => Capability::NotImplemented,
            InputCapability::KeyboardKeyKP5 => Capability::NotImplemented,
            InputCapability::KeyboardKeyKP6 => Capability::NotImplemented,
            InputCapability::KeyboardKeyKPPlus => Capability::NotImplemented,
            InputCapability::KeyboardKeyKP1 => Capability::NotImplemented,
            InputCapability::KeyboardKeyKP2 => Capability::NotImplemented,
            InputCapability::KeyboardKeyKP3 => Capability::NotImplemented,
            InputCapability::KeyboardKeyKP0 => Capability::NotImplemented,
            InputCapability::KeyboardKeyKPDot => Capability::NotImplemented,
            InputCapability::KeyboardKeyZenkakuhankaku => Capability::NotImplemented,
            InputCapability::KeyboardKey102nd => Capability::NotImplemented,
            InputCapability::KeyboardKeyF11 => Capability::NotImplemented,
            InputCapability::KeyboardKeyF12 => Capability::NotImplemented,
            InputCapability::KeyboardKeyRo => Capability::NotImplemented,
            InputCapability::KeyboardKeyKatakana => Capability::NotImplemented,
            InputCapability::KeyboardKeyHiragana => Capability::NotImplemented,
            InputCapability::KeyboardKeyHenkan => Capability::NotImplemented,
            InputCapability::KeyboardKeyKatakanaHiragana => Capability::NotImplemented,
            InputCapability::KeyboardKeyMuhenkan => Capability::NotImplemented,
            InputCapability::KeyboardKeyKPJpComma => Capability::NotImplemented,
            InputCapability::KeyboardKeyKPEnter => Capability::NotImplemented,
            InputCapability::KeyboardKeyRightCtrl => Capability::NotImplemented,
            InputCapability::KeyboardKeyKPSlash => Capability::NotImplemented,
            InputCapability::KeyboardKeySysRq => Capability::NotImplemented,
            InputCapability::KeyboardKeyRightAlt => Capability::NotImplemented,
            InputCapability::KeyboardKeyLineFeed => Capability::NotImplemented,
            InputCapability::KeyboardKeyHome => Capability::NotImplemented,
            InputCapability::KeyboardKeyUp => Capability::NotImplemented,
            InputCapability::KeyboardKeyPageUp => Capability::NotImplemented,
            InputCapability::KeyboardKeyLeft => Capability::NotImplemented,
            InputCapability::KeyboardKeyRight => Capability::NotImplemented,
            InputCapability::KeyboardKeyEnd => Capability::NotImplemented,
            InputCapability::KeyboardKeyDown => Capability::NotImplemented,
            InputCapability::KeyboardKeyPageDown => Capability::NotImplemented,
            InputCapability::KeyboardKeyInsert => Capability::NotImplemented,
            InputCapability::KeyboardKeyDelete => Capability::NotImplemented,
            InputCapability::KeyboardKeyMacro => Capability::NotImplemented,
            InputCapability::KeyboardKeyMute => Capability::NotImplemented,
            InputCapability::KeyboardKeyVolumeDown => Capability::NotImplemented,
            InputCapability::KeyboardKeyVolumeUp => Capability::NotImplemented,
            InputCapability::KeyboardKeyPower => Capability::NotImplemented,
            InputCapability::KeyboardKeyKPEqual => Capability::NotImplemented,
            InputCapability::KeyboardKeyKPPlusMinus => Capability::NotImplemented,
            InputCapability::KeyboardKeyPause => Capability::NotImplemented,
            InputCapability::KeyboardKeyScale => Capability::NotImplemented,
            InputCapability::KeyboardKeyKPComma => Capability::NotImplemented,
            InputCapability::KeyboardKeyHangeul => Capability::NotImplemented,
            InputCapability::KeyboardKeyHanja => Capability::NotImplemented,
            InputCapability::KeyboardKeyYen => Capability::NotImplemented,
            InputCapability::KeyboardKeyLeftMeta => Capability::NotImplemented,
            InputCapability::KeyboardKeyRightMeta => Capability::NotImplemented,
            InputCapability::KeyboardKeyCompose => Capability::NotImplemented,
            InputCapability::KeyboardKeyStop => Capability::NotImplemented,
            InputCapability::KeyboardKeyAgain => Capability::NotImplemented,
            InputCapability::KeyboardKeyProps => Capability::NotImplemented,
            InputCapability::KeyboardKeyUndo => Capability::NotImplemented,
            InputCapability::KeyboardKeyFront => Capability::NotImplemented,
            InputCapability::KeyboardKeyCopy => Capability::NotImplemented,
            InputCapability::KeyboardKeyOpen => Capability::NotImplemented,
            InputCapability::KeyboardKeyPaste => Capability::NotImplemented,
            InputCapability::KeyboardKeyFind => Capability::NotImplemented,
            InputCapability::KeyboardKeyCut => Capability::NotImplemented,
            InputCapability::KeyboardKeyHelp => Capability::NotImplemented,
            InputCapability::KeyboardKeyMenu => Capability::NotImplemented,
            InputCapability::KeyboardKeyCalc => Capability::NotImplemented,
            InputCapability::KeyboardKeySetup => Capability::NotImplemented,
            InputCapability::KeyboardKeySleep => Capability::NotImplemented,
            InputCapability::KeyboardKeyWakeup => Capability::NotImplemented,
            InputCapability::KeyboardKeyFile => Capability::NotImplemented,
            InputCapability::KeyboardKeySendFile => Capability::NotImplemented,
            InputCapability::KeyboardKeyDeleteFile => Capability::NotImplemented,
            InputCapability::KeyboardKeyXfer => Capability::NotImplemented,
            InputCapability::KeyboardKeyProg1 => Capability::NotImplemented,
            InputCapability::KeyboardKeyProg2 => Capability::NotImplemented,
            InputCapability::KeyboardKeyWww => Capability::NotImplemented,
            InputCapability::KeyboardKeyMsdos => Capability::NotImplemented,
            InputCapability::KeyboardKeyScreenLock => Capability::NotImplemented,
            InputCapability::KeyboardKeyRotateDisplay => Capability::NotImplemented,
            InputCapability::KeyboardKeyCycleWindows => Capability::NotImplemented,
            InputCapability::KeyboardKeyMail => Capability::NotImplemented,
            InputCapability::KeyboardKeyBookmarks => Capability::NotImplemented,
            InputCapability::KeyboardKeyComputer => Capability::NotImplemented,
            InputCapability::KeyboardKeyBack => Capability::NotImplemented,
            InputCapability::KeyboardKeyForward => Capability::NotImplemented,
            InputCapability::KeyboardKeyCloseCd => Capability::NotImplemented,
            InputCapability::KeyboardKeyEjectCd => Capability::NotImplemented,
            InputCapability::KeyboardKeyEjectCloseCd => Capability::NotImplemented,
            InputCapability::KeyboardKeyNextSong => Capability::NotImplemented,
            InputCapability::KeyboardKeyPlayPause => Capability::NotImplemented,
            InputCapability::KeyboardKeyPreviousSong => Capability::NotImplemented,
            InputCapability::KeyboardKeyStopCd => Capability::NotImplemented,
            InputCapability::KeyboardKeyRecord => Capability::NotImplemented,
            InputCapability::KeyboardKeyRewind => Capability::NotImplemented,
            InputCapability::KeyboardKeyPhone => Capability::NotImplemented,
            InputCapability::KeyboardKeyIso => Capability::NotImplemented,
            InputCapability::KeyboardKeyConfig => Capability::NotImplemented,
            InputCapability::KeyboardKeyHomepage => Capability::NotImplemented,
            InputCapability::KeyboardKeyRefresh => Capability::NotImplemented,
            InputCapability::KeyboardKeyExit => Capability::NotImplemented,
            InputCapability::KeyboardKeyMove => Capability::NotImplemented,
            InputCapability::KeyboardKeyEdit => Capability::NotImplemented,
            InputCapability::KeyboardKeyScrollUp => Capability::NotImplemented,
            InputCapability::KeyboardKeyScrollDown => Capability::NotImplemented,
            InputCapability::KeyboardKeyKPLeftParen => Capability::NotImplemented,
            InputCapability::KeyboardKeyKPRightParen => Capability::NotImplemented,
            InputCapability::KeyboardKeyNew => Capability::NotImplemented,
            InputCapability::KeyboardKeyRedo => Capability::NotImplemented,
            InputCapability::KeyboardKeyF13 => Capability::NotImplemented,
            InputCapability::KeyboardKeyF14 => Capability::NotImplemented,
            InputCapability::KeyboardKeyF15 => Capability::NotImplemented,
            InputCapability::KeyboardKeyF16 => Capability::NotImplemented,
            InputCapability::KeyboardKeyF17 => Capability::NotImplemented,
            InputCapability::KeyboardKeyF18 => Capability::NotImplemented,
            InputCapability::KeyboardKeyF19 => Capability::NotImplemented,
            InputCapability::KeyboardKeyF20 => Capability::NotImplemented,
            InputCapability::KeyboardKeyF21 => Capability::NotImplemented,
            InputCapability::KeyboardKeyF22 => Capability::NotImplemented,
            InputCapability::KeyboardKeyF23 => Capability::NotImplemented,
            InputCapability::KeyboardKeyF24 => Capability::NotImplemented,
            InputCapability::KeyboardKeyPlayCd => Capability::NotImplemented,
            InputCapability::KeyboardKeyPauseCd => Capability::NotImplemented,
            InputCapability::KeyboardKeyProg3 => Capability::NotImplemented,
            InputCapability::KeyboardKeyProg4 => Capability::NotImplemented,
            InputCapability::KeyboardKeyDashboard => Capability::NotImplemented,
            InputCapability::KeyboardKeySuspend => Capability::NotImplemented,
            InputCapability::KeyboardKeyClose => Capability::NotImplemented,
            InputCapability::KeyboardKeyPlay => Capability::NotImplemented,
            InputCapability::KeyboardKeyFastForward => Capability::NotImplemented,
            InputCapability::KeyboardKeyBassBoost => Capability::NotImplemented,
            InputCapability::KeyboardKeyPrint => Capability::NotImplemented,
            InputCapability::KeyboardKeyHp => Capability::NotImplemented,
            InputCapability::KeyboardKeyCamera => Capability::NotImplemented,
            InputCapability::KeyboardKeySound => Capability::NotImplemented,
            InputCapability::KeyboardKeyQuestion => Capability::NotImplemented,
            InputCapability::KeyboardKeyEmail => Capability::NotImplemented,
            InputCapability::KeyboardKeyChat => Capability::NotImplemented,
            InputCapability::KeyboardKeySearch => Capability::NotImplemented,
            InputCapability::KeyboardKeyConnect => Capability::NotImplemented,
            InputCapability::KeyboardKeyFinance => Capability::NotImplemented,
            InputCapability::KeyboardKeySport => Capability::NotImplemented,
            InputCapability::KeyboardKeyShop => Capability::NotImplemented,
            InputCapability::KeyboardKeyAltErase => Capability::NotImplemented,
            InputCapability::KeyboardKeyCancel => Capability::NotImplemented,
            InputCapability::KeyboardKeyBrightnessDown => Capability::NotImplemented,
            InputCapability::KeyboardKeyBrightnessUp => Capability::NotImplemented,
            InputCapability::KeyboardKeyMedia => Capability::NotImplemented,
            InputCapability::KeyboardKeySwitchVideoMode => Capability::NotImplemented,
            InputCapability::KeyboardKeyKBDillumToggle => Capability::NotImplemented,
            InputCapability::KeyboardKeyKBDillumDown => Capability::NotImplemented,
            InputCapability::KeyboardKeyKBDillumUp => Capability::NotImplemented,
            InputCapability::KeyboardKeySend => Capability::NotImplemented,
            InputCapability::KeyboardKeyReply => Capability::NotImplemented,
            InputCapability::KeyboardKeyForwardMail => Capability::NotImplemented,
            InputCapability::KeyboardKeySave => Capability::NotImplemented,
            InputCapability::KeyboardKeyDocuments => Capability::NotImplemented,
            InputCapability::KeyboardKeyBattery => Capability::NotImplemented,
            InputCapability::KeyboardKeyBluetooth => Capability::NotImplemented,
            InputCapability::KeyboardKeyWlan => Capability::NotImplemented,
            InputCapability::KeyboardKeyUwb => Capability::NotImplemented,
            InputCapability::KeyboardKeyUnknown => Capability::NotImplemented,
            InputCapability::KeyboardKeyVideoNext => Capability::NotImplemented,
            InputCapability::KeyboardKeyVideoPrev => Capability::NotImplemented,
            InputCapability::KeyboardKeyBrightnessCycle => Capability::NotImplemented,
            InputCapability::KeyboardKeyBrightnessAuto => Capability::NotImplemented,
            InputCapability::KeyboardKeyDisplayOff => Capability::NotImplemented,
            InputCapability::KeyboardKeyWwan => Capability::NotImplemented,
            InputCapability::KeyboardKeyRfKill => Capability::NotImplemented,
            InputCapability::KeyboardKeyMicMute => Capability::NotImplemented,
            InputCapability::KeyboardKeyOk => Capability::NotImplemented,
            InputCapability::KeyboardKeySelect => Capability::NotImplemented,
            InputCapability::KeyboardKeyGoto => Capability::NotImplemented,
            InputCapability::KeyboardKeyClear => Capability::NotImplemented,
            InputCapability::KeyboardKeyPower2 => Capability::NotImplemented,
            InputCapability::KeyboardKeyOption => Capability::NotImplemented,
            InputCapability::KeyboardKeyInfo => Capability::NotImplemented,
            InputCapability::KeyboardKeyTime => Capability::NotImplemented,
            InputCapability::KeyboardKeyVendor => Capability::NotImplemented,
            InputCapability::KeyboardKeyArchive => Capability::NotImplemented,
            InputCapability::KeyboardKeyProgram => Capability::NotImplemented,
            InputCapability::KeyboardKeyChannel => Capability::NotImplemented,
            InputCapability::KeyboardKeyFavorites => Capability::NotImplemented,
            InputCapability::KeyboardKeyEpg => Capability::NotImplemented,
            InputCapability::KeyboardKeyPvr => Capability::NotImplemented,
            InputCapability::KeyboardKeyMhp => Capability::NotImplemented,
            InputCapability::KeyboardKeyLanguage => Capability::NotImplemented,
            InputCapability::KeyboardKeyTitle => Capability::NotImplemented,
            InputCapability::KeyboardKeySubtitle => Capability::NotImplemented,
            InputCapability::KeyboardKeyAngle => Capability::NotImplemented,
            InputCapability::KeyboardKeyFullScreen => Capability::NotImplemented,
            InputCapability::KeyboardKeyMode => Capability::NotImplemented,
            InputCapability::KeyboardKeyKeyboard => Capability::NotImplemented,
            InputCapability::KeyboardKeyAspectRatio => Capability::NotImplemented,
            InputCapability::KeyboardKeyPc => Capability::NotImplemented,
            InputCapability::KeyboardKeyTv => Capability::NotImplemented,
            InputCapability::KeyboardKeyTv2 => Capability::NotImplemented,
            InputCapability::KeyboardKeyVcr => Capability::NotImplemented,
            InputCapability::KeyboardKeyVcr2 => Capability::NotImplemented,
            InputCapability::KeyboardKeySat => Capability::NotImplemented,
            InputCapability::KeyboardKeySat2 => Capability::NotImplemented,
            InputCapability::KeyboardKeyCd => Capability::NotImplemented,
            InputCapability::KeyboardKeyTape => Capability::NotImplemented,
            InputCapability::KeyboardKeyRadio => Capability::NotImplemented,
            InputCapability::KeyboardKeyTuner => Capability::NotImplemented,
            InputCapability::KeyboardKeyPlayer => Capability::NotImplemented,
            InputCapability::KeyboardKeyText => Capability::NotImplemented,
            InputCapability::KeyboardKeyDvd => Capability::NotImplemented,
            InputCapability::KeyboardKeyAux => Capability::NotImplemented,
            InputCapability::KeyboardKeyMp3 => Capability::NotImplemented,
            InputCapability::KeyboardKeyAudio => Capability::NotImplemented,
            InputCapability::KeyboardKeyVideo => Capability::NotImplemented,
            InputCapability::KeyboardKeyDirectory => Capability::NotImplemented,
            InputCapability::KeyboardKeyList => Capability::NotImplemented,
            InputCapability::KeyboardKeyMemo => Capability::NotImplemented,
            InputCapability::KeyboardKeyCalendar => Capability::NotImplemented,
            InputCapability::KeyboardKeyRed => Capability::NotImplemented,
            InputCapability::KeyboardKeyGreen => Capability::NotImplemented,
            InputCapability::KeyboardKeyYellow => Capability::NotImplemented,
            InputCapability::KeyboardKeyBlue => Capability::NotImplemented,
            InputCapability::KeyboardKeyChannelUp => Capability::NotImplemented,
            InputCapability::KeyboardKeyChannelDown => Capability::NotImplemented,
            InputCapability::KeyboardKeyFirst => Capability::NotImplemented,
            InputCapability::KeyboardKeyLast => Capability::NotImplemented,
            InputCapability::KeyboardKeyAb => Capability::NotImplemented,
            InputCapability::KeyboardKeyNext => Capability::NotImplemented,
            InputCapability::KeyboardKeyRestart => Capability::NotImplemented,
            InputCapability::KeyboardKeySlow => Capability::NotImplemented,
            InputCapability::KeyboardKeyShuffle => Capability::NotImplemented,
            InputCapability::KeyboardKeyBreak => Capability::NotImplemented,
            InputCapability::KeyboardKeyPrevious => Capability::NotImplemented,
            InputCapability::KeyboardKeyDigits => Capability::NotImplemented,
            InputCapability::KeyboardKeyTeen => Capability::NotImplemented,
            InputCapability::KeyboardKeyTwen => Capability::NotImplemented,
            InputCapability::KeyboardKeyVideoPhone => Capability::NotImplemented,
            InputCapability::KeyboardKeyGames => Capability::NotImplemented,
            InputCapability::KeyboardKeyZoomIn => Capability::NotImplemented,
            InputCapability::KeyboardKeyZoomOut => Capability::NotImplemented,
            InputCapability::KeyboardKeyZoomReset => Capability::NotImplemented,
            InputCapability::KeyboardKeyWordProcessor => Capability::NotImplemented,
            InputCapability::KeyboardKeyEditor => Capability::NotImplemented,
            InputCapability::KeyboardKeySpreadsheet => Capability::NotImplemented,
            InputCapability::KeyboardKeyGraphicsEditor => Capability::NotImplemented,
            InputCapability::KeyboardKeyPresentation => Capability::NotImplemented,
            InputCapability::KeyboardKeyDatabase => Capability::NotImplemented,
            InputCapability::KeyboardKeyNews => Capability::NotImplemented,
            InputCapability::KeyboardKeyVoicemail => Capability::NotImplemented,
            InputCapability::KeyboardKeyAddressbook => Capability::NotImplemented,
            InputCapability::KeyboardKeyMessenger => Capability::NotImplemented,
            InputCapability::KeyboardKeyDisplayToggle => Capability::NotImplemented,
            InputCapability::KeyboardKeySpellcheck => Capability::NotImplemented,
            InputCapability::KeyboardKeyLogoff => Capability::NotImplemented,
            InputCapability::KeyboardKeyDollar => Capability::NotImplemented,
            InputCapability::KeyboardKeyEuro => Capability::NotImplemented,
            InputCapability::KeyboardKeyFrameBack => Capability::NotImplemented,
            InputCapability::KeyboardKeyFrameForward => Capability::NotImplemented,
            InputCapability::KeyboardKeyContextMenu => Capability::NotImplemented,
            InputCapability::KeyboardKeyMediaRepeat => Capability::NotImplemented,
            InputCapability::KeyboardKey10ChannelsUp => Capability::NotImplemented,
            InputCapability::KeyboardKey10ChannelsDown => Capability::NotImplemented,
            InputCapability::KeyboardKeyImages => Capability::NotImplemented,
            InputCapability::KeyboardKeyNotificationCenter => Capability::NotImplemented,
            InputCapability::KeyboardKeyPickupPhone => Capability::NotImplemented,
            InputCapability::KeyboardKeyHangupPhone => Capability::NotImplemented,
            InputCapability::KeyboardKeyDelEol => Capability::NotImplemented,
            InputCapability::KeyboardKeyDelEos => Capability::NotImplemented,
            InputCapability::KeyboardKeyInsLine => Capability::NotImplemented,
            InputCapability::KeyboardKeyDelLine => Capability::NotImplemented,
            InputCapability::KeyboardKeyFn => Capability::NotImplemented,
            InputCapability::KeyboardKeyFnEsc => Capability::NotImplemented,
            InputCapability::KeyboardKeyFnF1 => Capability::NotImplemented,
            InputCapability::KeyboardKeyFnF2 => Capability::NotImplemented,
            InputCapability::KeyboardKeyFnF3 => Capability::NotImplemented,
            InputCapability::KeyboardKeyFnF4 => Capability::NotImplemented,
            InputCapability::KeyboardKeyFnF5 => Capability::NotImplemented,
            InputCapability::KeyboardKeyFnF6 => Capability::NotImplemented,
            InputCapability::KeyboardKeyFnF7 => Capability::NotImplemented,
            InputCapability::KeyboardKeyFnF8 => Capability::NotImplemented,
            InputCapability::KeyboardKeyFnF9 => Capability::NotImplemented,
            InputCapability::KeyboardKeyFnF10 => Capability::NotImplemented,
            InputCapability::KeyboardKeyFnF11 => Capability::NotImplemented,
            InputCapability::KeyboardKeyFnF12 => Capability::NotImplemented,
            InputCapability::KeyboardKeyFn1 => Capability::NotImplemented,
            InputCapability::KeyboardKeyFn2 => Capability::NotImplemented,
            InputCapability::KeyboardKeyFnD => Capability::NotImplemented,
            InputCapability::KeyboardKeyFnE => Capability::NotImplemented,
            InputCapability::KeyboardKeyFnF => Capability::NotImplemented,
            InputCapability::KeyboardKeyFnS => Capability::NotImplemented,
            InputCapability::KeyboardKeyFnB => Capability::NotImplemented,
            InputCapability::KeyboardKeyFnRightShift => Capability::NotImplemented,
            InputCapability::KeyboardKeyBrlDot1 => Capability::NotImplemented,
            InputCapability::KeyboardKeyBrlDot2 => Capability::NotImplemented,
            InputCapability::KeyboardKeyBrlDot3 => Capability::NotImplemented,
            InputCapability::KeyboardKeyBrlDot4 => Capability::NotImplemented,
            InputCapability::KeyboardKeyBrlDot5 => Capability::NotImplemented,
            InputCapability::KeyboardKeyBrlDot6 => Capability::NotImplemented,
            InputCapability::KeyboardKeyBrlDot7 => Capability::NotImplemented,
            InputCapability::KeyboardKeyBrlDot8 => Capability::NotImplemented,
            InputCapability::KeyboardKeyBrlDot9 => Capability::NotImplemented,
            InputCapability::KeyboardKeyBrlDot10 => Capability::NotImplemented,
            InputCapability::KeyboardKeyNumeric0 => Capability::NotImplemented,
            InputCapability::KeyboardKeyNumeric1 => Capability::NotImplemented,
            InputCapability::KeyboardKeyNumeric2 => Capability::NotImplemented,
            InputCapability::KeyboardKeyNumeric3 => Capability::NotImplemented,
            InputCapability::KeyboardKeyNumeric4 => Capability::NotImplemented,
            InputCapability::KeyboardKeyNumeric5 => Capability::NotImplemented,
            InputCapability::KeyboardKeyNumeric6 => Capability::NotImplemented,
            InputCapability::KeyboardKeyNumeric7 => Capability::NotImplemented,
            InputCapability::KeyboardKeyNumeric8 => Capability::NotImplemented,
            InputCapability::KeyboardKeyNumeric9 => Capability::NotImplemented,
            InputCapability::KeyboardKeyNumericStar => Capability::NotImplemented,
            InputCapability::KeyboardKeyNumericPound => Capability::NotImplemented,
            InputCapability::KeyboardKeyNumericA => Capability::NotImplemented,
            InputCapability::KeyboardKeyNumericB => Capability::NotImplemented,
            InputCapability::KeyboardKeyNumericC => Capability::NotImplemented,
            InputCapability::KeyboardKeyNumericD => Capability::NotImplemented,
            InputCapability::KeyboardKeyCameraFocus => Capability::NotImplemented,
            InputCapability::KeyboardKeyWpsButton => Capability::NotImplemented,
            InputCapability::KeyboardKeyTouchpadToggle => Capability::NotImplemented,
            InputCapability::KeyboardKeyTouchpadOn => Capability::NotImplemented,
            InputCapability::KeyboardKeyTouchpadOff => Capability::NotImplemented,
            InputCapability::KeyboardKeyCameraZoomIn => Capability::NotImplemented,
            InputCapability::KeyboardKeyCameraZoomOut => Capability::NotImplemented,
            InputCapability::KeyboardKeyCameraUp => Capability::NotImplemented,
            InputCapability::KeyboardKeyCameraDown => Capability::NotImplemented,
            InputCapability::KeyboardKeyCameraLeft => Capability::NotImplemented,
            InputCapability::KeyboardKeyCameraRight => Capability::NotImplemented,
            InputCapability::KeyboardKeyAttendantOn => Capability::NotImplemented,
            InputCapability::KeyboardKeyAttendantOff => Capability::NotImplemented,
            InputCapability::KeyboardKeyAttendantToggle => Capability::NotImplemented,
            InputCapability::KeyboardKeyLightsToggle => Capability::NotImplemented,
            InputCapability::KeyboardKeyAlsToggle => Capability::NotImplemented,
            InputCapability::KeyboardKeyRotateLockToggle => Capability::NotImplemented,
            InputCapability::KeyboardKeyRefreshRateToggle => Capability::NotImplemented,
            InputCapability::KeyboardKeyButtonConfig => Capability::NotImplemented,
            InputCapability::KeyboardKeyTaskManager => Capability::NotImplemented,
            InputCapability::KeyboardKeyJournal => Capability::NotImplemented,
            InputCapability::KeyboardKeyControlPanel => Capability::NotImplemented,
            InputCapability::KeyboardKeyAppSelect => Capability::NotImplemented,
            InputCapability::KeyboardKeyScreensaver => Capability::NotImplemented,
            InputCapability::KeyboardKeyVoiceCommand => Capability::NotImplemented,
            InputCapability::KeyboardKeyAssistant => Capability::NotImplemented,
            InputCapability::KeyboardKeyKbdLayoutNext => Capability::NotImplemented,
            InputCapability::KeyboardKeyEmojiPicker => Capability::NotImplemented,
            InputCapability::KeyboardKeyDictate => Capability::NotImplemented,
            InputCapability::KeyboardKeyCameraAccessEnable => Capability::NotImplemented,
            InputCapability::KeyboardKeyCameraAccessDisable => Capability::NotImplemented,
            InputCapability::KeyboardKeyCameraAccessToggle => Capability::NotImplemented,
            InputCapability::KeyboardKeyAccessibility => Capability::NotImplemented,
            InputCapability::KeyboardKeyDoNotDisturb => Capability::NotImplemented,
            InputCapability::KeyboardKeyBrightnessMin => Capability::NotImplemented,
            InputCapability::KeyboardKeyBrightnessMax => Capability::NotImplemented,
            InputCapability::KeyboardKeyKbdInputAssistPrev => Capability::NotImplemented,
            InputCapability::KeyboardKeyKbdInputAssistNext => Capability::NotImplemented,
            InputCapability::KeyboardKeyKbdInputAssistPrevGroup => Capability::NotImplemented,
            InputCapability::KeyboardKeyKbdInputAssistNextGroup => Capability::NotImplemented,
            InputCapability::KeyboardKeyKbdInputAssistAccept => Capability::NotImplemented,
            InputCapability::KeyboardKeyKbdInputAssistCancel => Capability::NotImplemented,
            InputCapability::KeyboardKeyRightUp => Capability::NotImplemented,
            InputCapability::KeyboardKeyRightDown => Capability::NotImplemented,
            InputCapability::KeyboardKeyLeftUp => Capability::NotImplemented,
            InputCapability::KeyboardKeyLeftDown => Capability::NotImplemented,
            InputCapability::KeyboardKeyRootMenu => Capability::NotImplemented,
            InputCapability::KeyboardKeyMediaTopMenu => Capability::NotImplemented,
            InputCapability::KeyboardKeyNumeric11 => Capability::NotImplemented,
            InputCapability::KeyboardKeyNumeric12 => Capability::NotImplemented,
            InputCapability::KeyboardKeyAudioDesc => Capability::NotImplemented,
            InputCapability::KeyboardKey3dMode => Capability::NotImplemented,
            InputCapability::KeyboardKeyNextFavorite => Capability::NotImplemented,
            InputCapability::KeyboardKeyStopRecord => Capability::NotImplemented,
            InputCapability::KeyboardKeyPauseRecord => Capability::NotImplemented,
            InputCapability::KeyboardKeyVod => Capability::NotImplemented,
            InputCapability::KeyboardKeyUnmute => Capability::NotImplemented,
            InputCapability::KeyboardKeyFastReverse => Capability::NotImplemented,
            InputCapability::KeyboardKeySlowReverse => Capability::NotImplemented,
            InputCapability::KeyboardKeyData => Capability::NotImplemented,
            InputCapability::KeyboardKeyOnscreenKeyboard => Capability::NotImplemented,
            InputCapability::KeyboardKeyPrivacyScreenToggle => Capability::NotImplemented,
            InputCapability::KeyboardKeySelectiveScreenshot => Capability::NotImplemented,
            InputCapability::KeyboardKeyNextElement => Capability::NotImplemented,
            InputCapability::KeyboardKeyPreviousElement => Capability::NotImplemented,
            InputCapability::KeyboardKeyAutopilotEngageToggle => Capability::NotImplemented,
            InputCapability::KeyboardKeyMarkWaypoint => Capability::NotImplemented,
            InputCapability::KeyboardKeySos => Capability::NotImplemented,
            InputCapability::KeyboardKeyNavChart => Capability::NotImplemented,
            InputCapability::KeyboardKeyFishingChart => Capability::NotImplemented,
            InputCapability::KeyboardKeySingleRangeRadar => Capability::NotImplemented,
            InputCapability::KeyboardKeyDualRangeRadar => Capability::NotImplemented,
            InputCapability::KeyboardKeyRadarOverlay => Capability::NotImplemented,
            InputCapability::KeyboardKeyTraditionalSonar => Capability::NotImplemented,
            InputCapability::KeyboardKeyClearvuSonar => Capability::NotImplemented,
            InputCapability::KeyboardKeySidevuSonar => Capability::NotImplemented,
            InputCapability::KeyboardKeyNavInfo => Capability::NotImplemented,
            InputCapability::KeyboardKeyBrightnessMenu => Capability::NotImplemented,
            InputCapability::KeyboardKeyMacro1 => Capability::NotImplemented,
            InputCapability::KeyboardKeyMacro2 => Capability::NotImplemented,
            InputCapability::KeyboardKeyMacro3 => Capability::NotImplemented,
            InputCapability::KeyboardKeyMacro4 => Capability::NotImplemented,
            InputCapability::KeyboardKeyMacro5 => Capability::NotImplemented,
            InputCapability::KeyboardKeyMacro6 => Capability::NotImplemented,
            InputCapability::KeyboardKeyMacro7 => Capability::NotImplemented,
            InputCapability::KeyboardKeyMacro8 => Capability::NotImplemented,
            InputCapability::KeyboardKeyMacro9 => Capability::NotImplemented,
            InputCapability::KeyboardKeyMacro10 => Capability::NotImplemented,
            InputCapability::KeyboardKeyMacro11 => Capability::NotImplemented,
            InputCapability::KeyboardKeyMacro12 => Capability::NotImplemented,
            InputCapability::KeyboardKeyMacro13 => Capability::NotImplemented,
            InputCapability::KeyboardKeyMacro14 => Capability::NotImplemented,
            InputCapability::KeyboardKeyMacro15 => Capability::NotImplemented,
            InputCapability::KeyboardKeyMacro16 => Capability::NotImplemented,
            InputCapability::KeyboardKeyMacro17 => Capability::NotImplemented,
            InputCapability::KeyboardKeyMacro18 => Capability::NotImplemented,
            InputCapability::KeyboardKeyMacro19 => Capability::NotImplemented,
            InputCapability::KeyboardKeyMacro20 => Capability::NotImplemented,
            InputCapability::KeyboardKeyMacro21 => Capability::NotImplemented,
            InputCapability::KeyboardKeyMacro22 => Capability::NotImplemented,
            InputCapability::KeyboardKeyMacro23 => Capability::NotImplemented,
            InputCapability::KeyboardKeyMacro24 => Capability::NotImplemented,
            InputCapability::KeyboardKeyMacro25 => Capability::NotImplemented,
            InputCapability::KeyboardKeyMacro26 => Capability::NotImplemented,
            InputCapability::KeyboardKeyMacro27 => Capability::NotImplemented,
            InputCapability::KeyboardKeyMacro28 => Capability::NotImplemented,
            InputCapability::KeyboardKeyMacro29 => Capability::NotImplemented,
            InputCapability::KeyboardKeyMacro30 => Capability::NotImplemented,
            InputCapability::KeyboardKeyMacroRecordStart => Capability::NotImplemented,
            InputCapability::KeyboardKeyMacroRecordStop => Capability::NotImplemented,
            InputCapability::KeyboardKeyMacroPresetCycle => Capability::NotImplemented,
            InputCapability::KeyboardKeyMacroPreset1 => Capability::NotImplemented,
            InputCapability::KeyboardKeyMacroPreset2 => Capability::NotImplemented,
            InputCapability::KeyboardKeyMacroPreset3 => Capability::NotImplemented,
            InputCapability::KeyboardKeyKbdLcdMenu1 => Capability::NotImplemented,
            InputCapability::KeyboardKeyKbdLcdMenu2 => Capability::NotImplemented,
            InputCapability::KeyboardKeyKbdLcdMenu3 => Capability::NotImplemented,
            InputCapability::KeyboardKeyKbdLcdMenu4 => Capability::NotImplemented,
            InputCapability::KeyboardKeyKbdLcdMenu5 => Capability::NotImplemented,
            InputCapability::MouseButtonLeft => Capability::NotImplemented,
            InputCapability::MouseButtonRight => Capability::NotImplemented,
            InputCapability::MouseButtonMiddle => Capability::NotImplemented,
            InputCapability::MouseButtonSide => Capability::NotImplemented,
            InputCapability::MouseButtonExtra => Capability::NotImplemented,
            InputCapability::GamepadButtonQuick => {
                Capability::Gamepad(Gamepad::Button(GamepadButton::QuickAccess))
            }
            InputCapability::GamepadButtonQuick2 => {
                Capability::Gamepad(Gamepad::Button(GamepadButton::QuickAccess2))
            }
            InputCapability::GamepadButtonSouth => {
                Capability::Gamepad(Gamepad::Button(GamepadButton::South))
            }
            InputCapability::GamepadButtonEast => {
                Capability::Gamepad(Gamepad::Button(GamepadButton::East))
            }
            InputCapability::GamepadButtonNorth => {
                Capability::Gamepad(Gamepad::Button(GamepadButton::North))
            }
            InputCapability::GamepadButtonWest => {
                Capability::Gamepad(Gamepad::Button(GamepadButton::West))
            }
            InputCapability::GamepadButtonSelect => {
                Capability::Gamepad(Gamepad::Button(GamepadButton::Select))
            }
            InputCapability::GamepadButtonStart => {
                Capability::Gamepad(Gamepad::Button(GamepadButton::Start))
            }
            InputCapability::GamepadButtonGuide => {
                Capability::Gamepad(Gamepad::Button(GamepadButton::Guide))
            }
            InputCapability::GamepadButtonDpadUp => {
                Capability::Gamepad(Gamepad::Button(GamepadButton::DPadUp))
            }
            InputCapability::GamepadButtonDpadDown => {
                Capability::Gamepad(Gamepad::Button(GamepadButton::DPadDown))
            }
            InputCapability::GamepadButtonDpadLeft => {
                Capability::Gamepad(Gamepad::Button(GamepadButton::DPadLeft))
            }
            InputCapability::GamepadButtonDpadRight => {
                Capability::Gamepad(Gamepad::Button(GamepadButton::DPadRight))
            }
            InputCapability::GamepadButtonLeftBumper => {
                Capability::Gamepad(Gamepad::Button(GamepadButton::LeftBumper))
            }
            InputCapability::GamepadButtonRightBumper => {
                Capability::Gamepad(Gamepad::Button(GamepadButton::RightBumper))
            }
            InputCapability::GamepadButtonLeftTrigger => {
                Capability::Gamepad(Gamepad::Button(GamepadButton::LeftTrigger))
            }
            InputCapability::GamepadButtonRightTrigger => {
                Capability::Gamepad(Gamepad::Button(GamepadButton::RightTrigger))
            }
            InputCapability::GamepadButtonLeftStick => {
                Capability::Gamepad(Gamepad::Button(GamepadButton::LeftStick))
            }
            InputCapability::GamepadButtonRightStick => {
                Capability::Gamepad(Gamepad::Button(GamepadButton::RightStick))
            }
            InputCapability::GamepadButtonKeyboard => {
                Capability::Gamepad(Gamepad::Button(GamepadButton::Keyboard))
            }
            InputCapability::GamepadButtonScreenshot => {
                Capability::Gamepad(Gamepad::Button(GamepadButton::Screenshot))
            }
            InputCapability::GamepadButtonMute => {
                Capability::Gamepad(Gamepad::Button(GamepadButton::Mute))
            }
            InputCapability::GamepadButtonLeftPaddle1 => {
                Capability::Gamepad(Gamepad::Button(GamepadButton::LeftPaddle1))
            }
            InputCapability::GamepadButtonLeftPaddle2 => {
                Capability::Gamepad(Gamepad::Button(GamepadButton::LeftPaddle2))
            }
            InputCapability::GamepadButtonLeftPaddle3 => {
                Capability::Gamepad(Gamepad::Button(GamepadButton::LeftPaddle3))
            }
            InputCapability::GamepadButtonRightPaddle1 => {
                Capability::Gamepad(Gamepad::Button(GamepadButton::RightPaddle1))
            }
            InputCapability::GamepadButtonRightPaddle2 => {
                Capability::Gamepad(Gamepad::Button(GamepadButton::RightPaddle2))
            }
            InputCapability::GamepadButtonRightPaddle3 => {
                Capability::Gamepad(Gamepad::Button(GamepadButton::RightPaddle3))
            }
            InputCapability::GamepadButtonLeftStickTouch => {
                Capability::Gamepad(Gamepad::Button(GamepadButton::LeftStickTouch))
            }
            InputCapability::GamepadButtonRightStickTouch => {
                Capability::Gamepad(Gamepad::Button(GamepadButton::RightStickTouch))
            }
            InputCapability::GamepadButtonLeftTop => {
                Capability::Gamepad(Gamepad::Button(GamepadButton::LeftTop))
            }
            InputCapability::GamepadButtonRightTop => {
                Capability::Gamepad(Gamepad::Button(GamepadButton::RightTop))
            }
            InputCapability::GamepadAxisLeftStick => {
                Capability::Gamepad(Gamepad::Axis(GamepadAxis::LeftStick))
            }
            InputCapability::GamepadAxisRightStick => {
                Capability::Gamepad(Gamepad::Axis(GamepadAxis::RightStick))
            }
            InputCapability::GamepadTriggerLeft => {
                Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::LeftTrigger))
            }
            InputCapability::GamepadTriggerRight => {
                Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::RightTrigger))
            }
            InputCapability::GamepadTriggerLeftTouchpadForce => {
                Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::LeftTouchpadForce))
            }
            InputCapability::GamepadTriggerLeftStickForce => {
                Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::LeftStickForce))
            }
            InputCapability::GamepadTriggerRightTouchpadForce => {
                Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::RightTouchpadForce))
            }
            InputCapability::GamepadTriggerRightStickForce => {
                Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::RightStickForce))
            }
            InputCapability::GamepadGyroCenter => Capability::Gamepad(Gamepad::Gyro),
            InputCapability::GamepadGyroLeft => Capability::Gamepad(Gamepad::Gyro),
            InputCapability::GamepadGyroRight => Capability::Gamepad(Gamepad::Gyro),
            InputCapability::GamepadAccelerometerCenter => {
                Capability::Gamepad(Gamepad::Accelerometer)
            }
            InputCapability::GamepadAccelerometerLeft => {
                Capability::Gamepad(Gamepad::Accelerometer)
            }
            InputCapability::GamepadAccelerometerRight => {
                Capability::Gamepad(Gamepad::Accelerometer)
            }
            InputCapability::TouchpadLeftMotion => {
                Capability::Touchpad(Touchpad::LeftPad(Touch::Motion))
            }
            InputCapability::TouchpadCenterMotion => {
                Capability::Touchpad(Touchpad::CenterPad(Touch::Motion))
            }
            InputCapability::TouchpadRightMotion => {
                Capability::Touchpad(Touchpad::RightPad(Touch::Motion))
            }
            InputCapability::TouchpadLeftButton => {
                Capability::Touchpad(Touchpad::LeftPad(Touch::Button(TouchButton::Press)))
            }
            InputCapability::TouchpadCenterButton => {
                Capability::Touchpad(Touchpad::CenterPad(Touch::Button(TouchButton::Press)))
            }
            InputCapability::TouchpadRightButton => {
                Capability::Touchpad(Touchpad::RightPad(Touch::Button(TouchButton::Press)))
            }
            InputCapability::TouchscreenMotion => Capability::Touchscreen(Touch::Motion),
            InputCapability::TouchscreenTopMotion => Capability::Touchscreen(Touch::Motion),
        };
        if capability == Capability::NotImplemented {
            log::warn!("Translation not implemented for: {value:?}");
        }
        capability
    }
}
