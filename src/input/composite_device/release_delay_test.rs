use std::time::{Duration, Instant};

use crate::input::capability::{Capability, Gamepad, GamepadButton, Touch, Touchpad};

use super::{release_delay::ReleaseDelay, MIN_EVENT_TIME};

fn button() -> Capability {
    Capability::Gamepad(Gamepad::Button(GamepadButton::South))
}

fn touch() -> Capability {
    Capability::Touchpad(Touchpad::RightPad(Touch::Motion))
}

fn ms(value: u64) -> Duration {
    Duration::from_millis(value)
}

#[test]
fn test_press_is_not_delayed() {
    let mut delay = ReleaseDelay::default();
    assert_eq!(delay.delay(&button(), true, Instant::now()), None);
}

#[test]
fn test_release_right_after_press_is_delayed() {
    let mut delay = ReleaseDelay::default();
    let t0 = Instant::now();
    delay.delay(&button(), true, t0);
    assert_eq!(delay.delay(&button(), false, t0), Some(MIN_EVENT_TIME));
}

#[test]
fn test_release_is_delayed_by_remaining_time() {
    let mut delay = ReleaseDelay::default();
    let t0 = Instant::now();
    delay.delay(&button(), true, t0);
    assert_eq!(
        delay.delay(&button(), false, t0 + ms(10)),
        Some(MIN_EVENT_TIME - ms(10))
    );
}

#[test]
fn test_release_after_min_event_time_is_not_delayed() {
    let mut delay = ReleaseDelay::default();
    let t0 = Instant::now();
    delay.delay(&button(), true, t0);
    assert_eq!(delay.delay(&button(), false, t0 + MIN_EVENT_TIME), None);
}

#[test]
fn test_touch_motion_does_not_delay_release() {
    let mut delay = ReleaseDelay::default();
    let t0 = Instant::now();
    // Down, motion every 8ms, lift 8ms after the last motion
    for i in 0..=12 {
        assert_eq!(delay.delay(&touch(), true, t0 + ms(i * 8)), None);
    }
    assert_eq!(delay.delay(&touch(), false, t0 + ms(104)), None);
}

#[test]
fn test_quick_touch_tap_is_delayed() {
    let mut delay = ReleaseDelay::default();
    let t0 = Instant::now();
    delay.delay(&touch(), true, t0);
    assert_eq!(
        delay.delay(&touch(), false, t0 + ms(5)),
        Some(MIN_EVENT_TIME - ms(5))
    );
}

#[test]
fn test_release_without_press_is_not_delayed() {
    let mut delay = ReleaseDelay::default();
    assert_eq!(delay.delay(&button(), false, Instant::now()), None);
}

#[test]
fn test_new_press_restarts_timer() {
    let mut delay = ReleaseDelay::default();
    let t0 = Instant::now();
    delay.delay(&button(), true, t0);
    assert_eq!(delay.delay(&button(), false, t0 + ms(20)), None);
    delay.delay(&button(), true, t0 + ms(30));
    assert_eq!(
        delay.delay(&button(), false, t0 + ms(35)),
        Some(MIN_EVENT_TIME - ms(5))
    );
}

#[test]
fn test_capabilities_are_independent() {
    let mut delay = ReleaseDelay::default();
    let t0 = Instant::now();
    delay.delay(&button(), true, t0);
    delay.delay(&touch(), true, t0 + ms(20));
    assert_eq!(delay.delay(&button(), false, t0 + ms(25)), None);
    assert_eq!(
        delay.delay(&touch(), false, t0 + ms(25)),
        Some(MIN_EVENT_TIME - ms(5))
    );
}
