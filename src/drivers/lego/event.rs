/// Events that can be emitted by the Legion Go controller
#[derive(Clone, Debug)]
pub enum Event {
    GamepadButton(GamepadButtonEvent),
    MouseButton(MouseButtonEvent),
    TouchButton(TouchButtonEvent),
    Axis(AxisEvent),
    Trigger(TriggerEvent),
    Status(StatusEvent),
}

/// Binary input contain either pressed or unpressed
#[derive(Clone, Debug)]
pub struct BinaryInput {
    pub pressed: bool,
}

/// Axis input contain (x, y) coordinates
#[derive(Clone, Debug)]
pub struct TouchAxisInput {
    pub index: u8,
    pub is_touching: bool,
    pub x: u16,
    pub y: u16,
}

/// Axis input contain (x, y) coordinates
#[derive(Clone, Debug)]
pub struct MouseAxisInput {
    pub x: i16,
    pub y: i16,
}
/// Axis input contain (x, y) coordinates
#[derive(Clone, Debug)]
pub struct JoyAxisInput {
    pub x: u8,
    pub y: u8,
}

// Status inputs contain some value that corresponds to the current status of a device.
#[derive(Clone, Debug)]
pub struct StatusInput {
    pub value: u8,
}

/// Mouse Wheel contains negative integars
#[derive(Clone, Debug)]
pub struct MouseWheelInput {
    pub value: i8,
}

/// Trigger input contains non-negative integars
#[derive(Clone, Debug)]
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
    /// Hamburger (☰) button
    Menu(BinaryInput),
    /// Overlapping square ⧉  button
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
    /// M2 on the side of the right controller
    M2(BinaryInput),
    /// M3 on the back of the right controller
    M3(BinaryInput),
    /// Mouse wheel click on the back of the right controller
    MouseClick(BinaryInput),
}

/// Button events represend binary inputs
#[derive(Clone, Debug)]
pub enum MouseButtonEvent {
    /// Y3 on the back of the right gamepad
    Y3(BinaryInput),
    /// M1 on the side of the right controller
    M1(BinaryInput),
    /// M2 on the side of the right controller
    M2(BinaryInput),
    /// M3 on the back of the right controller
    M3(BinaryInput),
    /// Mouse wheel click on the back of the right controller
    Left(BinaryInput),
}

/// Button events represend binary inputs
#[derive(Clone, Debug)]
pub enum TouchButtonEvent {
    /// Tap to click button
    Left(BinaryInput),
}

/// Axis events are events that have (x, y) values
#[derive(Clone, Debug)]
pub enum AxisEvent {
    Touchpad(TouchAxisInput),
    LStick(JoyAxisInput),
    RStick(JoyAxisInput),
    Mouse(MouseAxisInput),
}

/// Trigger events contain values indicating how far a trigger is pulled
#[derive(Clone, Debug)]
pub enum TriggerEvent {
    ATriggerL(TriggerInput),
    ATriggerR(TriggerInput),
    MouseWheel(MouseWheelInput),
}

/// StatusEvent has data on if controllers are in FPS mode, connected, and the battery state.
#[derive(Clone, Debug)]
pub enum StatusEvent {
    LeftControllerBattery(StatusInput),
    LeftControllerMode0(StatusInput),
    LeftControllerMode1(StatusInput),
    RightControllerBattery(StatusInput),
    RightControllerMode0(StatusInput),
    RightControllerMode1(StatusInput),
}
