
#[derive(Clone, Debug)]
pub enum Event {
    Button(ButtonEvent),
}

#[derive(Clone, Debug)]
pub struct BinaryInput {
    pub pressed: bool,
}

#[derive(Clone, Debug)]
pub enum ButtonEvent {
    DPadDown(BinaryInput),
    DPadUp(BinaryInput),
    DPadLeft(BinaryInput),
    DPadRight(BinaryInput),
}