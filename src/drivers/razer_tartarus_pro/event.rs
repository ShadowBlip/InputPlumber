use num_enum::FromPrimitive;

#[derive(Clone, Debug)]
pub struct Event {
    pub key: KeyCodes,
    pub pressed: bool,
}

/// Keycodes implemented for the Razer Tartarus Pro. Names not prefixed with
/// Phantom are emitted by hardware. Phantom keys are are used to implement
/// dual-function or overloaded bindings.
/// There are 22 phantom keys, 20 for the analog keys and 2 to account for
/// the additional definitions of 0x04 across the USB endpoints.
#[derive(Clone, Debug, FromPrimitive, PartialEq)]
#[repr(u8)]
pub enum KeyCodes {
    #[num_enum(default)]
    PhantomBlank,
    ScrollUp,
    KeyTwelve = 0x04, // A / Aux / Middle Mouse click
    PhantomOne,
    KeyNineteen, // C
    KeyFourteen, // D
    KeyNine,     // E
    KeyFifteen,  // F
    PhantomTwo,
    PhantomThree,
    PhantomFour,
    PhantomFive,
    PhantomSix,
    PhantomSeven,
    PhantomEight,
    PhantomNine,
    PhantomTen,
    PhantomEleven,
    KeySeven,    // Q
    KeyTen,      // R
    KeyThirteen, // S
    PhantomTwelve,
    PhantomThirteen,
    PhantomFourteen,
    KeyEight,    // W
    KeyEighteen, // X
    PhantomFifteen,
    KeySeventeen, // Z
    KeyOne,
    KeyTwo,
    KeyThree,
    KeyFour,
    KeyFive,
    PhantomSixteen,
    PhantomSeventeen,
    PhantomEighteen,
    PhantomNineteen,
    PhantomTwenty,
    KeySix = 0x2B,    // Tab
    KeyTwenty,        // Spacebar
    KeyEleven = 0x39, // Capslock
    Right = 0x4F,
    Left,
    Down,
    Up,
    KeySixteen = 0xE1, // LShift
    PhantomAux = 0xFD,
    PhantomMClick,
    ScrollDown,
}

pub static ANALOG_KEY_CODES: [(KeyCodes, KeyCodes); 20] = [
    (KeyCodes::KeyOne, KeyCodes::PhantomOne),
    (KeyCodes::KeyTwo, KeyCodes::PhantomTwo),
    (KeyCodes::KeyThree, KeyCodes::PhantomThree),
    (KeyCodes::KeyFour, KeyCodes::PhantomFour),
    (KeyCodes::KeyFive, KeyCodes::PhantomFive),
    (KeyCodes::KeySix, KeyCodes::PhantomSix),
    (KeyCodes::KeySeven, KeyCodes::PhantomSeven),
    (KeyCodes::KeyEight, KeyCodes::PhantomEight),
    (KeyCodes::KeyNine, KeyCodes::PhantomNine),
    (KeyCodes::KeyTen, KeyCodes::PhantomTen),
    (KeyCodes::KeyEleven, KeyCodes::PhantomEleven),
    (KeyCodes::KeyTwelve, KeyCodes::PhantomTwelve),
    (KeyCodes::KeyThirteen, KeyCodes::PhantomThirteen),
    (KeyCodes::KeyFourteen, KeyCodes::PhantomFourteen),
    (KeyCodes::KeyFifteen, KeyCodes::PhantomFifteen),
    (KeyCodes::KeySixteen, KeyCodes::PhantomSixteen),
    (KeyCodes::KeySeventeen, KeyCodes::PhantomSeventeen),
    (KeyCodes::KeyEighteen, KeyCodes::PhantomEighteen),
    (KeyCodes::KeyNineteen, KeyCodes::PhantomNineteen),
    (KeyCodes::KeyTwenty, KeyCodes::PhantomTwenty),
];
