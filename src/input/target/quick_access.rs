use std::time::{Duration, Instant};

use crate::input::{
    capability::{Capability, Gamepad, GamepadButton},
    event::{
        native::{NativeEvent, ScheduledNativeEvent},
        value::InputValue,
    },
};

const SOUTH_PRESS_DELAY: Duration = Duration::from_millis(160);
const SOUTH_RELEASE_DELAY: Duration = Duration::from_millis(160);
const GUIDE_RELEASE_DELAY: Duration = Duration::from_millis(240);
// The existing chord releases Guide 80 ms after South. Reuse that established
// interval as the minimum time each phase remains visible to Steam when the
// source firmware emits an almost instantaneous press/release pulse.
const MIN_BUTTON_HOLD: Duration = Duration::from_millis(80);

/// Schedules the Guide+South chord used to expose QuickAccess on targets that
/// do not have a native QuickAccess button.
#[derive(Debug, Default)]
pub(super) struct QuickAccessChord {
    pressed_at: Option<Instant>,
}

impl QuickAccessChord {
    pub(super) fn schedule(
        &mut self,
        pressed: bool,
        value: InputValue,
    ) -> [ScheduledNativeEvent; 2] {
        let guide = NativeEvent::new(
            Capability::Gamepad(Gamepad::Button(GamepadButton::Guide)),
            value.clone(),
        );
        let south = NativeEvent::new(
            Capability::Gamepad(Gamepad::Button(GamepadButton::South)),
            value,
        );

        if pressed {
            self.pressed_at = Some(Instant::now());
            return [
                ScheduledNativeEvent::new(guide, Duration::ZERO),
                ScheduledNativeEvent::new(south, SOUTH_PRESS_DELAY),
            ];
        }

        let held_for = self
            .pressed_at
            .take()
            .map(|pressed_at| pressed_at.elapsed());
        let (south_delay, guide_delay) = release_delays(held_for);
        [
            ScheduledNativeEvent::new(south, south_delay),
            ScheduledNativeEvent::new(guide, guide_delay),
        ]
    }
}

fn release_delays(held_for: Option<Duration>) -> (Duration, Duration) {
    let Some(held_for) = held_for else {
        return (SOUTH_RELEASE_DELAY, GUIDE_RELEASE_DELAY);
    };

    let earliest_south_release = SOUTH_PRESS_DELAY + MIN_BUTTON_HOLD;
    let earliest_guide_release = earliest_south_release + MIN_BUTTON_HOLD;
    (
        SOUTH_RELEASE_DELAY.max(earliest_south_release.saturating_sub(held_for)),
        GUIDE_RELEASE_DELAY.max(earliest_guide_release.saturating_sub(held_for)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_pulses_keep_each_qam_chord_button_pressed_for_a_full_interval() {
        let held_for = Duration::from_millis(1);
        let (south_release, guide_release) = release_delays(Some(held_for));

        assert_eq!(south_release, Duration::from_millis(239));
        assert_eq!(guide_release, Duration::from_millis(319));
        assert_eq!(
            held_for + south_release - SOUTH_PRESS_DELAY,
            MIN_BUTTON_HOLD
        );
        assert_eq!(guide_release - south_release, MIN_BUTTON_HOLD);
    }

    #[test]
    fn normal_presses_keep_existing_qam_chord_timing() {
        let (south_release, guide_release) = release_delays(Some(Duration::from_millis(80)));

        assert_eq!(south_release, SOUTH_RELEASE_DELAY);
        assert_eq!(guide_release, GUIDE_RELEASE_DELAY);
    }

    #[test]
    fn release_without_a_press_uses_existing_qam_chord_timing() {
        assert_eq!(
            release_delays(None),
            (SOUTH_RELEASE_DELAY, GUIDE_RELEASE_DELAY)
        );
    }
}
