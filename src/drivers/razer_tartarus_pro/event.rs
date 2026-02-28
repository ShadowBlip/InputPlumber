use num_enum::FromPrimitive;

// The Tartarus Pro despite looking like a lot of things is ultimately
// just a set of buttons.

#[derive(Clone, Debug)]
pub struct Event {
    pub key: KeyCodes,
    pub pressed: bool,
}

// Complete set of key codes that a Tartarus Pro emits in standard mode
// Despite the varied interfaces, there is only one overloaded scancode (0x04)
// and fortunately it is in a fixed position in the report types and unique per
// endpoint so we only have do deal with one instance at a time.
#[derive(Clone, Debug, FromPrimitive, PartialEq)]
#[repr(u8)]
pub enum KeyCodes {
    #[num_enum(default)]
    Blank,
    ScrollUp,
    KeyTwelve = 0x04,    // A / Aux / Middle Mouse click
    KeyNineteen = 0x06,  // C
    KeyFourteen,         // D
    KeyNine,             // E
    KeyFifteen,          // F
    KeySeven = 0x14,     // Q
    KeyTen,              // R
    KeyThirteen,         // S
    KeyEight = 0x1A,     // W
    KeyEighteen,         // X
    KeySeventeen = 0x1D, // Z
    KeyOne,
    KeyTwo,
    KeyThree,
    KeyFour,
    KeyFive,
    KeySix = 0x2B,    // Tab
    KeyTwenty,        // Spacebar
    KeyEleven = 0x39, // Capslock
    Right = 0x4F,
    Left,
    Down,
    Up,
    KeySixteen = 0xE1, // LShift
    Aux = 0xFD,        // Internal label to account for overload. Never sent by HW
    MClick,            // Internal label to account for overload. Never sent by HW
    ScrollDown,
}

// Analog mode is positional - key 1 is array index 1 so translating a report
// into variant space is trivial.
