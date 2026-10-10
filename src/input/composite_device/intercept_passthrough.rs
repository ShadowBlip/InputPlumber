use std::{
    collections::{HashSet, VecDeque},
    future::pending,
};

use tokio::time::{sleep_until, Duration, Instant};

use crate::input::{
    capability::{Capability, Gamepad, Mouse},
    event::native::NativeEvent,
};

/// Replay withheld activation chords without delaying unrelated input.
#[derive(Debug, Default)]
pub(super) struct InterceptPassthrough {
    events: VecDeque<(Instant, NativeEvent)>,
    inputs: HashSet<Capability>,
    delay: Duration,
    release_after: Option<Instant>,
}

// Preserve the existing Guide-before-button timing used by Steam shortcuts.
const CHORD_DELAY: Duration = Duration::from_millis(80);
// Give a replayed press time to appear in a target report before its release.
const MIN_PRESS_TIME: Duration = Duration::from_millis(16);

impl InterceptPassthrough {
    pub fn start(&mut self, events: Vec<NativeEvent>, now: Instant) {
        let mut at = self
            .events
            .back()
            .map(|(at, _)| *at + MIN_PRESS_TIME)
            .unwrap_or(now)
            .max(now);
        for event in events {
            self.inputs.insert(event.as_capability());
            self.events.push_back((at, event));
            at += CHORD_DELAY;
        }
        if let Some((at, event)) = self.events.back() {
            self.delay = *at - now
                + if event.pressed() {
                    MIN_PRESS_TIME
                } else {
                    Duration::ZERO
                };
        }
    }

    /// Defer matching transitions by the setup latency, not 80 ms per event.
    pub fn defer(&mut self, event: &NativeEvent, now: Instant) -> bool {
        let cap = event.as_capability();
        // A new button must not see a modifier that is already physically released.
        let follows_release = self.events.iter().any(|(_, event)| !event.pressed())
            && self.inputs.iter().any(|input| {
                matches!(
                    (&cap, input),
                    (Capability::Keyboard(_), Capability::Keyboard(_))
                        | (
                            Capability::Gamepad(Gamepad::Button(_)),
                            Capability::Gamepad(Gamepad::Button(_))
                        )
                        | (
                            Capability::Mouse(Mouse::Button(_)),
                            Capability::Mouse(Mouse::Button(_))
                        )
                )
            });
        if !self.inputs.contains(&cap) && !follows_release {
            return false;
        }
        let at = match self.events.back() {
            Some((at, _)) => (*at).max(now + self.delay),
            None => match self.release_after {
                Some(at) if now < at => at,
                _ => return false,
            },
        };
        self.inputs.insert(cap);
        self.events.push_back((at, event.clone()));
        true
    }

    pub fn pop_ready(&mut self, now: Instant) -> Option<NativeEvent> {
        let Some((at, _)) = self.events.front() else {
            if self.release_after.is_some_and(|at| now >= at) {
                self.clear();
            }
            return None;
        };
        if now < *at {
            return None;
        }
        let (at, event) = self.events.pop_front().unwrap();
        // If emission runs late, preserve the remaining relative timing too.
        let late = now - at;
        for (at, _) in &mut self.events {
            *at += late;
        }
        self.delay += late;
        if event.pressed() {
            self.release_after = Some(now + MIN_PRESS_TIME);
        }
        if self.events.is_empty() && self.release_after.is_none_or(|at| now >= at) {
            self.clear();
        }
        Some(event)
    }

    pub async fn wait(&self) {
        match self
            .events
            .front()
            .map(|(at, _)| *at)
            .or(self.release_after)
        {
            Some(at) => sleep_until(at).await,
            None => pending().await,
        }
    }

    /// Finish the old sequence before a fresh immediate input changes its modifier state.
    pub fn finish_before(&mut self, cap: &Capability) -> Vec<NativeEvent> {
        if self.inputs.contains(cap) {
            self.drain()
        } else {
            Vec::new()
        }
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn drain(&mut self) -> Vec<NativeEvent> {
        let events = self.events.drain(..).map(|(_, event)| event).collect();
        self.clear();
        events
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{
        capability::{Gamepad, GamepadAxis, GamepadButton, Keyboard, MouseButton},
        event::value::InputValue,
    };

    fn button(cap: GamepadButton, pressed: bool) -> NativeEvent {
        NativeEvent::new(
            Capability::Gamepad(Gamepad::Button(cap)),
            InputValue::Bool(pressed),
        )
    }

    fn after(now: Instant, millis: u64) -> Instant {
        now + Duration::from_millis(millis)
    }

    fn chord(now: Instant, other: GamepadButton, pressed: bool) -> InterceptPassthrough {
        let mut replay = InterceptPassthrough::default();
        replay.start(
            vec![button(GamepadButton::Guide, true), button(other, pressed)],
            now,
        );
        replay
    }

    fn assert_event(event: Option<NativeEvent>, expected: GamepadButton, pressed: bool) {
        let event = event.expect("missing button transition");
        assert_eq!(
            event.as_capability(),
            button(expected, pressed).as_capability()
        );
        assert_eq!(event.pressed(), pressed);
    }

    #[test]
    fn quick_chords_keep_both_release_orders_after_the_presses() {
        for other in [
            GamepadButton::South,
            GamepadButton::West,
            GamepadButton::North,
            GamepadButton::RightBumper,
        ] {
            for guide_first in [false, true] {
                let now = Instant::now();
                let mut replay = chord(now, other.clone(), true);
                assert_event(replay.pop_ready(now), GamepadButton::Guide, true);
                let releases = if guide_first {
                    [GamepadButton::Guide, other.clone()]
                } else {
                    [other.clone(), GamepadButton::Guide]
                };
                for (i, cap) in releases.iter().enumerate() {
                    assert!(replay.defer(&button(cap.clone(), false), after(now, 10 + i as u64)));
                }
                assert_event(replay.pop_ready(after(now, 80)), other.clone(), true);
                assert!(replay.pop_ready(after(now, 96)).is_none());
                for (i, cap) in releases.into_iter().enumerate() {
                    assert_event(replay.pop_ready(after(now, 106 + i as u64)), cap, false);
                }
                assert!(replay.inputs.is_empty());
            }
        }
    }

    #[test]
    fn unrelated_buttons_and_analog_input_are_not_deferred() {
        let now = Instant::now();
        let mut replay = chord(now, GamepadButton::South, true);
        assert!(!replay.defer(&button(GamepadButton::North, true), now));
        assert!(replay.defer(&button(GamepadButton::Guide, false), after(now, 10)));
        for cap in [
            Capability::Keyboard(Keyboard::KeyA),
            Capability::Mouse(Mouse::Button(MouseButton::Left)),
        ] {
            assert!(!replay.defer(
                &NativeEvent::new(cap, InputValue::Bool(true)),
                after(now, 20)
            ));
        }
        let stick = NativeEvent::new(
            Capability::Gamepad(Gamepad::Axis(GamepadAxis::RightStick)),
            InputValue::Vector2 {
                x: Some(0.5),
                y: None,
            },
        );
        assert!(!replay.defer(&stick, now));
    }

    #[test]
    fn a_new_button_waits_for_an_already_released_modifier() {
        let now = Instant::now();
        let mut replay = chord(now, GamepadButton::South, true);
        replay.pop_ready(now).unwrap();
        assert!(replay.defer(&button(GamepadButton::Guide, false), after(now, 10)));
        assert!(replay.defer(&button(GamepadButton::North, true), after(now, 20)));
        assert!(replay.defer(&button(GamepadButton::North, false), after(now, 50)));
        assert_event(replay.pop_ready(after(now, 80)), GamepadButton::South, true);
        assert_event(
            replay.pop_ready(after(now, 106)),
            GamepadButton::Guide,
            false,
        );
        assert_event(
            replay.pop_ready(after(now, 116)),
            GamepadButton::North,
            true,
        );
        assert_event(
            replay.pop_ready(after(now, 146)),
            GamepadButton::North,
            false,
        );
    }

    #[test]
    fn repeated_taps_do_not_accumulate_an_80_ms_delay_per_transition() {
        let now = Instant::now();
        let mut replay = chord(now, GamepadButton::South, true);
        replay.pop_ready(now).unwrap();
        let mut received = 2;
        let mut emitted = 1;
        for ms in 1..=1000 {
            let at = after(now, ms);
            while replay.pop_ready(at).is_some() {
                emitted += 1;
            }
            if ms % 100 == 10 || ms % 100 == 50 {
                received += 1;
                if !replay.defer(&button(GamepadButton::South, ms % 100 == 50), at) {
                    emitted += 1;
                }
            }
        }
        for cap in [GamepadButton::South, GamepadButton::Guide] {
            assert!(replay.defer(&button(cap, false), after(now, 1000)));
            received += 1;
        }
        let mut guide_release_at = None;
        for ms in 1001..=1200 {
            while let Some(event) = replay.pop_ready(after(now, ms)) {
                emitted += 1;
                if event.as_capability() == button(GamepadButton::Guide, false).as_capability() {
                    guide_release_at = Some(ms);
                }
            }
        }
        assert_eq!(received, emitted);
        assert_eq!(guide_release_at, Some(1096));
        assert!(replay.inputs.is_empty());
    }

    #[test]
    fn release_after_setup_only_waits_for_the_remaining_press_time() {
        let now = Instant::now();
        let mut replay = chord(now, GamepadButton::South, true);
        replay.pop_ready(now).unwrap();
        replay.pop_ready(after(now, 80)).unwrap();
        assert!(replay.defer(&button(GamepadButton::South, false), after(now, 81)));
        assert_event(
            replay.pop_ready(after(now, 96)),
            GamepadButton::South,
            false,
        );
        assert!(!replay.defer(&button(GamepadButton::South, true), after(now, 100)));
    }

    #[test]
    fn completed_guide_tap_has_no_empty_cooldown() {
        let now = Instant::now();
        let mut replay = chord(now, GamepadButton::Guide, false);
        replay.pop_ready(now).unwrap();
        replay.pop_ready(after(now, 80)).unwrap();
        assert!(!replay.defer(&button(GamepadButton::Guide, true), after(now, 90)));
        assert!(replay.inputs.is_empty());
    }

    #[test]
    fn immediate_guide_write_finishes_older_input_before_repress() {
        let now = Instant::now();
        let mut replay = chord(now, GamepadButton::South, true);
        replay.pop_ready(now).unwrap();
        for (cap, pressed, millis) in [
            (GamepadButton::South, false, 5),
            (GamepadButton::Guide, false, 10),
            (GamepadButton::North, true, 20),
            (GamepadButton::North, false, 25),
        ] {
            assert!(replay.defer(&button(cap, pressed), after(now, millis)));
        }
        // Guide is re-pressed and immediately forwarded before analog input.
        // Old X taps must finish before that new modifier, not fire underneath it.
        let pending = replay.finish_before(&button(GamepadButton::Guide, true).as_capability());
        let expected = [
            (GamepadButton::South, true),
            (GamepadButton::South, false),
            (GamepadButton::Guide, false),
            (GamepadButton::North, true),
            (GamepadButton::North, false),
        ];
        assert_eq!(pending.len(), expected.len());
        for (event, (cap, pressed)) in pending.into_iter().zip(expected) {
            assert_event(Some(event), cap, pressed);
        }
        assert!(replay.pop_ready(after(now, 160)).is_none());
        assert!(!replay.defer(&button(GamepadButton::Guide, false), after(now, 161)));
    }

    #[test]
    fn late_emission_preserves_the_remaining_spacing() {
        let now = Instant::now();
        let mut replay = chord(now, GamepadButton::Guide, false);
        let late = after(now, 1000);
        assert_event(replay.pop_ready(late), GamepadButton::Guide, true);
        assert!(replay.pop_ready(late).is_none());
        assert_event(
            replay.pop_ready(after(late, 80)),
            GamepadButton::Guide,
            false,
        );
    }

    #[test]
    fn a_new_chord_cannot_overtake_an_older_guide_tap() {
        let now = Instant::now();
        let mut replay = chord(now, GamepadButton::Guide, false);
        replay.pop_ready(now).unwrap();
        replay.start(
            vec![
                button(GamepadButton::Guide, true),
                button(GamepadButton::South, true),
            ],
            after(now, 10),
        );
        assert_event(
            replay.pop_ready(after(now, 80)),
            GamepadButton::Guide,
            false,
        );
        assert_event(replay.pop_ready(after(now, 96)), GamepadButton::Guide, true);
        assert_event(
            replay.pop_ready(after(now, 176)),
            GamepadButton::South,
            true,
        );
    }

    #[test]
    fn reset_discards_events_but_mode_change_can_drain_them() {
        let now = Instant::now();
        let mut replay = chord(now, GamepadButton::Guide, false);
        replay.pop_ready(now).unwrap();
        let remaining = replay.drain();
        assert_eq!(remaining.len(), 1);
        assert_event(remaining.into_iter().next(), GamepadButton::Guide, false);
        replay.start(vec![button(GamepadButton::Guide, true)], now);
        replay.clear();
        assert!(replay.inputs.is_empty());
        assert!(replay.pop_ready(after(now, 1000)).is_none());
    }
}
