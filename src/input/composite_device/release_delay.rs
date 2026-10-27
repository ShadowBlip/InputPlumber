use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use crate::input::capability::Capability;

use super::MIN_EVENT_TIME;

/// Delays DBus releases that come less than [MIN_EVENT_TIME] after their press.
#[derive(Debug, Default)]
pub struct ReleaseDelay {
    pressed_at: HashMap<Capability, Instant>,
}

impl ReleaseDelay {
    /// Returns how long to delay the event, if at all.
    pub fn delay(&mut self, cap: &Capability, pressed: bool, now: Instant) -> Option<Duration> {
        if pressed {
            // Only the first press starts the timer, so touch motion doesn't reset it
            self.pressed_at.entry(cap.clone()).or_insert(now);
            return None;
        }
        let pressed_at = self.pressed_at.remove(cap)?;
        let remaining = MIN_EVENT_TIME.saturating_sub(now.duration_since(pressed_at));
        (!remaining.is_zero()).then_some(remaining)
    }
}
