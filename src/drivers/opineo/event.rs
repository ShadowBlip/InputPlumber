use std::mem;

/// Events that can be emitted by the OrangePi Neo touchpad
#[derive(Clone, Debug)]
pub enum Event {
    TouchAxis(TouchAxisEvent),
    TouchButton(TouchButtonEvent),
    Trigger(TriggerEvent),
}

impl Event {
    /// Whether two events are the same type, ignoring payload data
    pub fn variant_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Event::TouchAxis(_), Event::TouchAxis(_)) => true,
            (Event::TouchButton(a), Event::TouchButton(b)) => a.variant_eq(b),
            (Event::Trigger(a), Event::Trigger(b)) => a.variant_eq(b),
            _ => false,
        }
    }
}

/// Axis input contain (x, y) coordinates
#[derive(Clone, Debug, Default)]
pub struct TouchAxisEvent {
    pub index: u8,
    pub is_touching: bool,
    pub x: u16,
    pub y: u16,
}

/// Binary input contain either pressed or unpressed
#[derive(Clone, Debug, Default)]
pub struct BinaryInput {
    pub pressed: bool,
}

/// Trigger input contains non-negative integers
#[derive(Clone, Debug, Default)]
pub struct TriggerInput {
    pub value: u8,
}

/// Button events represend binary inputs
#[derive(Clone, Debug)]
pub enum TouchButtonEvent {
    /// Tap to click button
    Left(BinaryInput),
}

impl TouchButtonEvent {
    /// Whether two events are the same type, ignoring payload data
    pub fn variant_eq(&self, other: &Self) -> bool {
        mem::discriminant(self) == mem::discriminant(other)
    }
}

/// Trigger events contain values indicating how far a trigger is pulled
#[derive(Clone, Debug)]
pub enum TriggerEvent {
    PadForce(TriggerInput),
}

impl TriggerEvent {
    /// Whether two events are the same type, ignoring payload data
    pub fn variant_eq(&self, other: &Self) -> bool {
        mem::discriminant(self) == mem::discriminant(other)
    }
}
