use std::mem;

use super::hid_report::TouchData;

#[derive(Clone, Debug)]
pub enum Event {
    Touch(TouchAxisInput),
}

impl Event {
    /// Whether two events are the same type, ignoring payload data
    pub fn variant_eq(&self, other: &Self) -> bool {
        mem::discriminant(self) == mem::discriminant(other)
    }
}

/// Axis input contain (x, y) coordinates
#[derive(Clone, Debug, Default)]
pub struct TouchAxisInput {
    pub index: u8,
    pub is_touching: bool,
    pub x: u16,
    pub y: u16,
}

impl From<TouchData> for TouchAxisInput {
    fn from(touch_data: TouchData) -> Self {
        let index = touch_data.contact_id;
        let is_touching = touch_data.is_touching();
        let (x, y) = match is_touching {
            true => (touch_data.get_x(), touch_data.get_y()),
            false => (0, 0),
        };
        Self {
            index,
            is_touching,
            x,
            y,
        }
    }
}
