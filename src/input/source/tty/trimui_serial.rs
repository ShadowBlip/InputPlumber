//! TrimUI Smart Pro S MCU source-device adapter.
//!
//! The serial transport and frame protocol live in
//! `crate::drivers::trimui_tty`; this translates decoded [`TrimuiEvent`]s
//! into native capabilities, applying per-stick adaptive calibration
//! (ported from trimui-inputrs).

use std::{error::Error, fmt::Debug};

use crate::{
    drivers::trimui_tty::{
        driver::Driver as TrimuiDriver,
        event::Event as TrimuiEvent,
        serial_report::{
            A, B, DPAD_DOWN, DPAD_LEFT, DPAD_RIGHT, DPAD_UP, L1, L2, L3, MODE, R1, R2, R3, SELECT,
            START, X, Y,
        },
        side_from_syspath, TrimuiSide,
    },
    input::{
        capability::{Capability, Gamepad, GamepadAxis, GamepadButton, GamepadTrigger},
        event::{native::NativeEvent, value::InputValue},
        source::{InputError, SourceInputDevice, SourceOutputDevice},
    },
    udev::device::UdevDevice,
};

const AXIS_MAX: f64 = 32767.0;

/// (mask, button) pairs emitted by the left MCU. L2 is handled separately
/// below: targets expose triggers as analog axes, not digital buttons.
const LEFT_BUTTONS: &[(u32, GamepadButton)] = &[
    (L1, GamepadButton::LeftBumper),
    (L3, GamepadButton::LeftStick),
    (MODE, GamepadButton::Guide),
];

/// (mask, button) pairs emitted by the right MCU. Note the X/Y quirk kept
/// from trimui-inputrs: the Smart Pro S reports these two face-button bits
/// opposite to the evdev labels on the physical controller.
const RIGHT_BUTTONS: &[(u32, GamepadButton)] = &[
    (A, GamepadButton::South),
    (B, GamepadButton::East),
    (Y, GamepadButton::North),
    (X, GamepadButton::West),
    (R1, GamepadButton::RightBumper),
    (R3, GamepadButton::RightStick),
    (START, GamepadButton::Start),
    (SELECT, GamepadButton::Select),
];

const DPAD_DIRS: &[(u32, GamepadButton)] = &[
    (DPAD_UP, GamepadButton::DPadUp),
    (DPAD_DOWN, GamepadButton::DPadDown),
    (DPAD_LEFT, GamepadButton::DPadLeft),
    (DPAD_RIGHT, GamepadButton::DPadRight),
];

// ---------------------------------------------------------------------------
// Adaptive stick calibration (verbatim port of trimui-inputrs `stick`,
// minus vendor config-file loading)
// ---------------------------------------------------------------------------

const RAW_MIN: f64 = 0.0;
const RAW_MAX: f64 = 4096.0;
const OUTPUT_MAX: f64 = 32767.0;
const BOOT_SAMPLES: usize = 31;
const FILTER_ALPHA: f64 = 0.25;
const CENTER_LEARN_BAND: f64 = 20.0;
const VEL_THRESH: f64 = 4.0;
const IDLE_REQUIRED: usize = 12;
const CENTER_ALPHA: f64 = 0.003;
const DEADZONE_NOISE_BASE: f64 = 6.0;
const NOISE_ALPHA: f64 = 0.08;
const NOISE_MULT: f64 = 6.0;
const DEADZONE_USER_MIN: f64 = 100.0;
const DEADZONE_USER_MAX: f64 = 250.0;
const START_HALF_SPAN: f64 = 900.0;
const MIN_ACTIVE_SPAN: f64 = 80.0;
const OUTER_GATE_RATIO: f64 = 0.65;
const SPAN_ALPHA_UP: f64 = 0.002;
const SPAN_ALPHA_DOWN: f64 = 0.0005;
const SPAN_MAX: f64 = 2400.0;
const KNEE_START: f64 = 0.90;
// Park must cover real mechanical rest offsets (measured ~120+ raw units
// on TMR sticks that don't return to electrical center); anything parked
// inside this band migrates the center instead of leaking.
const PARK_BAND: f64 = 300.0;
const PARK_REQUIRED: usize = 60;
const PARK_ALPHA: f64 = 0.06;

#[derive(Debug)]
pub struct StickAxis {
    boot: Vec<f64>,
    initialized: bool,
    center: f64,
    neg_span: f64,
    pos_span: f64,
    deadzone: f64,
    filt: f64,
    prev_filt: f64,
    idle_count: usize,
    noise_ema: f64,
    park_sum: f64,
    park_count: usize,
}

impl Default for StickAxis {
    fn default() -> Self {
        Self {
            boot: Vec::with_capacity(BOOT_SAMPLES),
            initialized: false,
            center: RAW_MAX / 2.0,
            neg_span: START_HALF_SPAN,
            pos_span: START_HALF_SPAN,
            deadzone: DEADZONE_USER_MIN,
            filt: RAW_MAX / 2.0,
            prev_filt: RAW_MAX / 2.0,
            idle_count: 0,
            noise_ema: 0.0,
            park_sum: 0.0,
            park_count: 0,
        }
    }
}

impl StickAxis {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update(&mut self, raw: u16) {
        let value = f64::from(raw).clamp(RAW_MIN, RAW_MAX);
        if !self.initialized {
            if self.boot.len() < BOOT_SAMPLES {
                self.boot.push(value);
                return;
            }
            let mut sorted = self.boot.clone();
            sorted.sort_by(f64::total_cmp);
            self.center = sorted[BOOT_SAMPLES / 2];
            self.filt = self.center;
            self.prev_filt = self.center;
            self.initialized = true;
            return;
        }

        self.filt += FILTER_ALPHA * (value - self.filt);
        let velocity = (self.filt - self.prev_filt).abs();
        self.prev_filt = self.filt;
        if (self.filt - self.center).abs() <= CENTER_LEARN_BAND && velocity <= VEL_THRESH {
            self.idle_count += 1;
        } else {
            self.idle_count = 0;
        }
        if self.idle_count >= IDLE_REQUIRED {
            self.center += CENTER_ALPHA * (self.filt - self.center);
            let deviation = (self.filt - self.center).abs();
            self.noise_ema += NOISE_ALPHA * (deviation - self.noise_ema);
            self.deadzone = (DEADZONE_NOISE_BASE + NOISE_MULT * self.noise_ema)
                .clamp(DEADZONE_USER_MIN, DEADZONE_USER_MAX);
        }

        if (self.filt - self.center).abs() <= PARK_BAND && velocity <= VEL_THRESH {
            self.park_sum += self.filt;
            self.park_count += 1;
            if self.park_count >= PARK_REQUIRED {
                let target = self.park_sum / self.park_count as f64;
                self.center += PARK_ALPHA * (target - self.center);
                self.park_sum = 0.0;
                self.park_count = 0;
            }
        } else {
            self.park_sum = 0.0;
            self.park_count = 0;
        }

        let delta = self.filt - self.center;
        if delta > 0.0 && delta >= self.pos_span * OUTER_GATE_RATIO {
            let alpha = if delta > self.pos_span {
                SPAN_ALPHA_UP
            } else {
                SPAN_ALPHA_DOWN
            };
            self.pos_span += alpha * (delta - self.pos_span);
        } else if delta < 0.0 && -delta >= self.neg_span * OUTER_GATE_RATIO {
            let magnitude = -delta;
            let alpha = if magnitude > self.neg_span {
                SPAN_ALPHA_UP
            } else {
                SPAN_ALPHA_DOWN
            };
            self.neg_span += alpha * (magnitude - self.neg_span);
        }
        self.neg_span = self.neg_span.clamp(MIN_ACTIVE_SPAN, SPAN_MAX);
        self.pos_span = self.pos_span.clamp(MIN_ACTIVE_SPAN, SPAN_MAX);
    }

    pub fn apply(&self, raw: u16) -> i32 {
        let raw = f64::from(raw);
        if !self.initialized {
            return 0;
        }
        let delta = raw.clamp(RAW_MIN, RAW_MAX) - self.center;
        let magnitude = delta.abs();
        if magnitude <= self.deadzone {
            return 0;
        }
        let span = if delta < 0.0 {
            self.neg_span
        } else {
            self.pos_span
        };
        let normalized = ((magnitude - self.deadzone)
            / (span.max(MIN_ACTIVE_SPAN) - self.deadzone).max(1.0))
        .clamp(0.0, 1.0);
        let eased = if normalized <= KNEE_START {
            normalized
        } else {
            let t = (normalized - KNEE_START) / (1.0 - KNEE_START);
            KNEE_START + (1.0 - KNEE_START) * (t * t * t * (t * (t * 6.0 - 15.0) + 10.0))
        };
        (if delta < 0.0 { -eased } else { eased } * OUTPUT_MAX).round() as i32
    }
}

// ---------------------------------------------------------------------------
// Source device implementation (one MCU UART)
// ---------------------------------------------------------------------------

/// [TrimuiSerial] source device implementation.
pub struct TrimuiSerial {
    driver: TrimuiDriver,
    side: TrimuiSide,
    previous_dpad: u32,
    previous_axes: Option<(i32, i32)>,
    cal_x: StickAxis,
    cal_y: StickAxis,
}

impl TrimuiSerial {
    /// Create a new [TrimuiSerial] source device for the given udev tty.
    /// The pad half is resolved from the parent UART address, never the
    /// `ttyAS*` sysname.
    pub fn new(device: UdevDevice) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let side = side_from_syspath(device.syspath().as_str())
            .ok_or("TrimUI UART syspath not recognized")?;
        let driver = TrimuiDriver::new(device.devnode().as_str(), side)?;
        Ok(Self {
            driver,
            side,
            previous_dpad: 0,
            previous_axes: None,
            cal_x: StickAxis::new(),
            cal_y: StickAxis::new(),
        })
    }

    fn button_table(&self) -> &'static [(u32, GamepadButton)] {
        match self.side {
            TrimuiSide::Left => LEFT_BUTTONS,
            TrimuiSide::Right => RIGHT_BUTTONS,
        }
    }

    fn axis(&self) -> GamepadAxis {
        match self.side {
            TrimuiSide::Left => GamepadAxis::LeftStick,
            TrimuiSide::Right => GamepadAxis::RightStick,
        }
    }

    fn translate(&mut self, event: TrimuiEvent, events: &mut Vec<NativeEvent>) {
        match event {
            TrimuiEvent::Buttons { buttons, changed } => {
                for (mask, button) in self.button_table() {
                    if changed & mask != 0 {
                        events.push(NativeEvent::new(
                            Capability::Gamepad(Gamepad::Button(button.clone())),
                            InputValue::Bool(buttons & mask != 0),
                        ));
                    }
                }
                // L2/R2 are digital on the wire, but targets expose triggers
                // as analog axes: emit full-scale Float edges instead.
                let (trigger_mask, trigger) = match self.side {
                    TrimuiSide::Left => (L2, GamepadTrigger::LeftTrigger),
                    TrimuiSide::Right => (R2, GamepadTrigger::RightTrigger),
                };
                if changed & trigger_mask != 0 {
                    events.push(NativeEvent::new(
                        Capability::Gamepad(Gamepad::Trigger(trigger)),
                        InputValue::Float(if buttons & trigger_mask != 0 {
                            1.0
                        } else {
                            0.0
                        }),
                    ));
                }
                // D-pad lives on the left MCU; emit per-direction edges.
                if self.side == TrimuiSide::Left {
                    for (mask, button) in DPAD_DIRS {
                        let pressed = buttons & mask != 0;
                        let was = self.previous_dpad & mask != 0;
                        if pressed != was {
                            events.push(NativeEvent::new(
                                Capability::Gamepad(Gamepad::Button(button.clone())),
                                InputValue::Bool(pressed),
                            ));
                        }
                    }
                    self.previous_dpad = buttons;
                }
            }
            TrimuiEvent::Stick { x, y } => {
                self.cal_x.update(x);
                self.cal_y.update(y);
                // Match trimui-inputrs polarity: Y is flipped.
                let output = (self.cal_x.apply(x), -self.cal_y.apply(y));
                if self.previous_axes != Some(output) {
                    events.push(NativeEvent::new(
                        Capability::Gamepad(Gamepad::Axis(self.axis())),
                        InputValue::Vector2 {
                            x: Some((output.0 as f64 / AXIS_MAX).clamp(-1.0, 1.0)),
                            y: Some((output.1 as f64 / AXIS_MAX).clamp(-1.0, 1.0)),
                        },
                    ));
                    self.previous_axes = Some(output);
                }
            }
        }
    }
}

impl Debug for TrimuiSerial {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TrimuiSerial")
            .field("side", &self.side)
            .finish()
    }
}

impl SourceInputDevice for TrimuiSerial {
    fn poll(&mut self) -> Result<Vec<NativeEvent>, InputError> {
        let events = self
            .driver
            .poll()
            .map_err(|e| InputError::DeviceError(e.to_string()))?;
        let mut native = Vec::with_capacity(events.len());
        for event in events {
            self.translate(event, &mut native);
        }
        Ok(native)
    }

    fn get_capabilities(&self) -> Result<Vec<Capability>, InputError> {
        Ok(CAPABILITIES.into())
    }
}

impl SourceOutputDevice for TrimuiSerial {}

/// List of all capabilities that [TrimuiSerial] implements (superset of
/// both halves; each instance only ever emits its own side).
pub const CAPABILITIES: &[Capability] = &[
    Capability::Gamepad(Gamepad::Button(GamepadButton::South)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::East)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::North)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::West)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::Start)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::Select)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::Guide)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::DPadUp)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::DPadDown)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::DPadLeft)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::DPadRight)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftBumper)),
    Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::LeftTrigger)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftStick)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::RightBumper)),
    Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::RightTrigger)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::RightStick)),
    Capability::Gamepad(Gamepad::Axis(GamepadAxis::LeftStick)),
    Capability::Gamepad(Gamepad::Axis(GamepadAxis::RightStick)),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn x_and_y_bits_keep_upstream_quirk() {
        // Physical X/Y are reported swapped in the bitmap; the table
        // compensates so X -> West and Y -> North.
        let x = RIGHT_BUTTONS.iter().find(|(m, _)| *m == X).unwrap();
        let y = RIGHT_BUTTONS.iter().find(|(m, _)| *m == Y).unwrap();
        assert_eq!(x.1, GamepadButton::West);
        assert_eq!(y.1, GamepadButton::North);
    }

    #[test]
    fn learned_calibration_matches_c_boot_count() {
        let mut axis = StickAxis::new();
        for _ in 0..BOOT_SAMPLES {
            axis.update(2048);
            assert_eq!(axis.apply(4096), 0);
        }
        axis.update(2048);
        assert!(axis.apply(4096) > 0);
    }
}
