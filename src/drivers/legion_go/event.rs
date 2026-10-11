use std::mem;

/// Events that can be emitted by the Legion Go controller
#[derive(Clone, Debug)]
pub enum Event {
    GamepadButton(GamepadButtonEvent),
    TouchButton(TouchButtonEvent),
    Axis(AxisEvent),
    Trigger(TriggerEvent),
}

impl Event {
    /// Whether two events are the same type, ignoring payload data
    pub fn variant_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Event::GamepadButton(a), Event::GamepadButton(b)) => a.variant_eq(b),
            (Event::TouchButton(a), Event::TouchButton(b)) => a.variant_eq(b),
            (Event::Axis(a), Event::Axis(b)) => a.variant_eq(b),
            (Event::Trigger(a), Event::Trigger(b)) => a.variant_eq(b),
            _ => false,
        }
    }
}

/// Binary input contain either pressed or unpressed
#[derive(Clone, Debug, Default)]
pub struct BinaryInput {
    pub pressed: bool,
}

/// Axis input contain (x, y) coordinates
#[derive(Clone, Debug, Default)]
pub struct TouchAxisInput {
    pub index: u8,
    pub is_touching: bool,
    pub x: u16,
    pub y: u16,
}

/// Axis input contain (x, y) coordinates
#[derive(Clone, Debug, Default)]
pub struct JoyAxisInput {
    pub x: u8,
    pub y: u8,
}
/// IMU Axis input contain (x, y, z) coordinates
#[derive(Clone, Debug, Default)]
pub struct ImuAxisInput {
    pub pitch: i16,
    pub roll: i16,
    pub yaw: i16,
}

/// Mouse Wheel contains negative integars
#[derive(Clone, Debug, Default)]
pub struct MouseWheelInput {
    pub value: i8,
}

/// Trigger input contains non-negative integars
#[derive(Clone, Debug, Default)]
pub struct TriggerInput {
    pub value: u8,
}

/// Button events represend binary inputs
#[derive(Clone, Debug)]
pub enum GamepadButtonEvent {
    /// A Button
    A(BinaryInput),
    /// X Button
    X(BinaryInput),
    /// B Button
    B(BinaryInput),
    /// Y Button
    Y(BinaryInput),
    /// Hamburger (☰) button (Right controller Legion Go 2)
    Menu(BinaryInput),
    /// Overlapping square ⧉  button (Left controller Legion Go 2)
    View(BinaryInput),
    /// Legion button on left controller
    Legion(BinaryInput),
    /// Quick Access button on right controller
    QuickAccess(BinaryInput),
    /// DPad down
    DPadDown(BinaryInput),
    /// DPad up
    DPadUp(BinaryInput),
    /// DPad left
    DPadLeft(BinaryInput),
    /// DPad right
    DPadRight(BinaryInput),
    /// Left shoulder button
    LB(BinaryInput),
    /// Binary sensor for left analog trigger
    DTriggerL(BinaryInput),
    /// Z-axis button on the left stick
    ThumbL(BinaryInput),
    /// Y1 on the back of the left gamepad
    Y1(BinaryInput),
    /// Y2 on the back of the left gamepad
    Y2(BinaryInput),
    /// Y3 on the back of the right gamepad
    Y3(BinaryInput),
    /// Right shoulder button
    RB(BinaryInput),
    /// Binary sensor for right analog trigger
    DTriggerR(BinaryInput),
    /// Z-axis button on the right stick
    ThumbR(BinaryInput),
    /// M1 on the side of the right controller
    M1(BinaryInput),
    /// M2 on the side of the right controller
    M2(BinaryInput),
    /// M3 on the back of the right controller
    M3(BinaryInput),
    /// Mouse wheel click on the back of the right controller
    MouseClick(BinaryInput),
    // Legion Go 2 Only
    /// Show Desktop, 11 small squares icon immedialy below dpad.
    ShowDesktop(BinaryInput),
    /// Alt Tab, squares and rectangles icon below show desktop button.
    AltTab(BinaryInput),
}

/// Button events represend binary inputs
#[derive(Clone, Debug)]
pub enum TouchButtonEvent {
    /// Tap to click button
    Left(BinaryInput),
}

impl GamepadButtonEvent {
    /// Whether two events are the same button, ignoring payload data
    pub fn variant_eq(&self, other: &Self) -> bool {
        mem::discriminant(self) == mem::discriminant(other)
    }

    /// Whether the button is pressed
    pub fn pressed(&self) -> bool {
        match self {
            Self::A(input)
            | Self::X(input)
            | Self::B(input)
            | Self::Y(input)
            | Self::Menu(input)
            | Self::View(input)
            | Self::Legion(input)
            | Self::QuickAccess(input)
            | Self::DPadDown(input)
            | Self::DPadUp(input)
            | Self::DPadLeft(input)
            | Self::DPadRight(input)
            | Self::LB(input)
            | Self::DTriggerL(input)
            | Self::ThumbL(input)
            | Self::Y1(input)
            | Self::Y2(input)
            | Self::Y3(input)
            | Self::RB(input)
            | Self::DTriggerR(input)
            | Self::ThumbR(input)
            | Self::M1(input)
            | Self::M2(input)
            | Self::M3(input)
            | Self::MouseClick(input)
            | Self::ShowDesktop(input)
            | Self::AltTab(input) => input.pressed,
        }
    }
}

/// Axis events are events that have (x, y) values
#[derive(Clone, Debug)]
pub enum AxisEvent {
    Touchpad(TouchAxisInput),
    LStick(JoyAxisInput),
    RStick(JoyAxisInput),
    LeftAccel(ImuAxisInput),
    LeftGyro(ImuAxisInput),
    RightAccel(ImuAxisInput),
    RightGyro(ImuAxisInput),
    MultiAccel(ImuAxisInput),
    MultiGyro(ImuAxisInput),
}

impl AxisEvent {
    /// Whether two events are the same axis, ignoring payload data
    pub fn variant_eq(&self, other: &Self) -> bool {
        mem::discriminant(self) == mem::discriminant(other)
    }
}

/// Trigger events contain values indicating how far a trigger is pulled
#[derive(Clone, Debug)]
pub enum TriggerEvent {
    ATriggerL(TriggerInput),
    ATriggerR(TriggerInput),
    MouseWheel(MouseWheelInput),
    RpadForce(TriggerInput),
}

impl TriggerEvent {
    /// Whether two events are the same trigger, ignoring payload data
    pub fn variant_eq(&self, other: &Self) -> bool {
        mem::discriminant(self) == mem::discriminant(other)
    }
}

impl TouchButtonEvent {
    /// Whether two events are the same button, ignoring payload data
    pub fn variant_eq(&self, other: &Self) -> bool {
        mem::discriminant(self) == mem::discriminant(other)
    }

    /// Whether the button is pressed
    pub fn pressed(&self) -> bool {
        match self {
            Self::Left(input) => input.pressed,
        }
    }
}
