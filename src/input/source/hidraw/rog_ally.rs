use std::{collections::HashMap, error::Error, fmt::Debug, time::Instant};

use evdev::{FFEffectData, FFEffectKind};
use packed_struct::types::{Integer, SizedInteger};

use crate::{
    drivers::{
        rog_ally::{driver::Driver, event, GAMEPAD_WRITE_TIMEOUT},
        steam_deck::hid_report::{CommandType, PackedTriggerHapticPulse, PadSide},
    },
    input::{
        capability::Capability,
        event::native::NativeEvent,
        output_capability::{Haptic, OutputCapability},
        output_event::OutputEvent,
        source::{
            haptic_duration, haptic_gain_level, HapticPulseSeries, InputError, OutputError,
            SourceInputDevice, SourceOutputDevice, HAPTIC_NOMINAL_PERCENT,
        },
    },
    udev::device::UdevDevice,
};

/// RogAlly source device implementation
pub struct RogAlly {
    driver: Driver,
    ff_evdev_effects: HashMap<i16, FFEffectData>,
    /// Main motor levels, 0-255 scale
    main_strong: u8,
    main_weak: u8,
    /// Scheduled stop for one-shot main motor haptics
    main_stop_at: Option<Instant>,
    /// Active trigger haptic pulse series
    pulse: Option<HapticPulseSeries>,
    /// Until when the current trigger tick must stay on
    pulse_on_until: Option<Instant>,
    /// Trigger actuator levels, 0-255 scale
    trigger_lt: u8,
    trigger_rt: u8,
}

impl RogAlly {
    /// Create a new source device with the given udev
    /// device information
    pub fn new(device_info: UdevDevice) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let driver = Driver::new(device_info)?;
        Ok(Self {
            driver,
            ff_evdev_effects: HashMap::new(),
            main_strong: 0,
            main_weak: 0,
            main_stop_at: None,
            pulse: None,
            pulse_on_until: None,
            trigger_lt: 0,
            trigger_rt: 0,
        })
    }

    /// Returns the next available evdev effect id
    fn next_ff_effect_id(&self) -> i16 {
        const MAX: i16 = 2096;
        let mut i = 0;
        loop {
            if !self.ff_evdev_effects.contains_key(&i) {
                return i;
            }
            i += 1;
            if i > MAX {
                return -1;
            }
        }
    }

    /// Write the current actuator percentages to the driver
    fn write_state(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        let e = event::OutputEvent::RumbleExtended {
            left_trigger: self.trigger_lt,
            right_trigger: self.trigger_rt,
            left_motor: self.main_strong,
            right_motor: self.main_weak,
        };
        self.driver.write_event(&e)
    }

    /// Process the given evdev force feedback event
    fn process_evdev_ff(
        &mut self,
        input_event: evdev::InputEvent,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        // Get the code (effect id) and value of the event
        let (code, value) =
            if let evdev::EventSummary::ForceFeedback(_, code, value) = input_event.destructure() {
                (code, value)
            } else {
                log::debug!("Unhandled evdev output event: {:?}", input_event);
                return Ok(());
            };

        // Find the effect data for this event
        let effect_id = code.0 as i16;
        let Some(effect_data) = self.ff_evdev_effects.get(&effect_id) else {
            log::warn!("No effect id found: {}", code.0);
            return Ok(());
        };

        // The value determines if the effect should be playing or not
        if value == 0 {
            self.main_strong = 0;
            self.main_weak = 0;
            self.main_stop_at = None;
            return self.write_state();
        }

        // Perform the rumble based on the effect
        // TODO: handle effect duration, etc.
        match effect_data.kind {
            FFEffectKind::Damper => Ok(()),
            FFEffectKind::Inertia => Ok(()),
            FFEffectKind::Constant {
                level: _,
                envelope: _,
            } => Ok(()),
            FFEffectKind::Ramp {
                start_level: _,
                end_level: _,
                envelope: _,
            } => Ok(()),
            FFEffectKind::Periodic {
                waveform: _,
                period: _,
                magnitude: _,
                offset: _,
                phase: _,
                envelope: _,
            } => Ok(()),
            FFEffectKind::Spring { condition: _ } => Ok(()),
            FFEffectKind::Friction { condition: _ } => Ok(()),
            FFEffectKind::Rumble {
                strong_magnitude,
                weak_magnitude,
            } => {
                // Scale the rumble values to a percentage
                let strong = ((strong_magnitude as f64 / u16::MAX as f64) * 100.0).round() as u8;
                let weak = ((weak_magnitude as f64 / u16::MAX as f64) * 100.0).round() as u8;
                self.main_strong = strong;
                self.main_weak = weak;
                self.main_stop_at = None;
                self.write_state()
            }
        }
    }

    /// Process Steam Deck style trackpad haptic reports by emulating them on
    /// the main motors
    fn process_haptic(
        &mut self,
        report: PackedTriggerHapticPulse,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        if report.cmd_type == CommandType::Off {
            self.main_strong = 0;
            self.main_weak = 0;
            self.main_stop_at = None;
            return self.write_state();
        }

        let level = haptic_gain_level(report.gain, 100.0, HAPTIC_NOMINAL_PERCENT) as u8;

        // strong drives the left motor and weak the right
        let (strong, weak) = match report.side {
            PadSide::Left => (level, 0),
            PadSide::Right => (0, level),
            PadSide::Both => (level, level),
        };
        self.main_strong = strong;
        self.main_weak = weak;
        self.main_stop_at = None;
        self.write_state()?;

        // Level based motors need a scheduled stop
        self.main_stop_at = Some(Instant::now() + haptic_duration(&report));

        Ok(())
    }
}

impl SourceInputDevice for RogAlly {
    /// Poll the given input device for input events
    fn poll(&mut self) -> Result<Vec<NativeEvent>, InputError> {
        let mut changed = false;

        if let Some(series) = &mut self.pulse {
            let now = Instant::now();
            // Hold each tick for its minimum width before letting the
            // series advance
            let held = self.pulse_on_until.is_some_and(|until| now < until);
            if !held {
                if let Some((weak, strong)) = series.advance(now) {
                    // strong drives the left trigger and weak the right
                    self.trigger_lt = strong;
                    self.trigger_rt = weak;
                    self.pulse_on_until =
                        (weak != 0 || strong != 0).then(|| now + GAMEPAD_WRITE_TIMEOUT);
                    changed = true;
                }
            }
            if series.is_finished() {
                self.pulse = None;
                self.pulse_on_until = None;
            }
        }

        if let Some(stop_at) = self.main_stop_at {
            if Instant::now() >= stop_at {
                self.main_stop_at = None;
                if self.main_strong != 0 || self.main_weak != 0 {
                    self.main_strong = 0;
                    self.main_weak = 0;
                    changed = true;
                }
            }
        }

        if changed {
            let _ = self.write_state();
        }

        Ok(vec![])
    }

    /// Returns the possible input events this device is capable of emitting
    fn get_capabilities(&self) -> Result<Vec<Capability>, InputError> {
        Ok(vec![])
    }
}

impl SourceOutputDevice for RogAlly {
    /// Returns the possible output events this device is capable of (e.g.
    /// force feedback, LED, etc.)
    fn get_output_capabilities(&self) -> Result<Vec<OutputCapability>, OutputError> {
        if self.driver.hid_device.is_none() {
            return Ok(vec![]);
        }

        Ok(vec![
            OutputCapability::ForceFeedback,
            OutputCapability::ForceFeedbackUpload,
            OutputCapability::ForceFeedbackErase,
            OutputCapability::Haptics(Haptic::TrackpadLeft),
            OutputCapability::Haptics(Haptic::TrackpadRight),
        ])
    }

    /// Write the given output event to the source device
    fn write_event(&mut self, event: OutputEvent) -> Result<(), OutputError> {
        log::trace!("Received output event: {:?}", event);
        match event {
            OutputEvent::Evdev(input_event) => Ok(self.process_evdev_ff(input_event)?),
            OutputEvent::DualSense(report) => {
                if report.use_rumble_not_haptics || report.enable_improved_rumble_emulation {
                    // strong drives the left motor and weak the right
                    self.main_strong = report.rumble_emulation_left;
                    self.main_weak = report.rumble_emulation_right;
                    self.main_stop_at = None;
                    self.write_state()?;
                }
                Ok(())
            }
            OutputEvent::Uinput(_) => Ok(()),
            OutputEvent::TriggerHapticPulse(report) => Ok(self.process_haptic(report)?),
            OutputEvent::TriggerRepeatedHapticPulse(mut report) => {
                // The duration field of the rumble report is in 10 ms units,
                // so round the pulse duration up to a whole step
                report.duration_us =
                    Integer::from_primitive(step_up_us(report.duration_us.to_primitive()));
                let series = HapticPulseSeries::new(&report, 100);
                let (weak, strong) = series.magnitudes();
                // strong drives the left trigger and weak the right
                self.trigger_lt = strong;
                self.trigger_rt = weak;
                self.write_state()?;
                self.pulse_on_until =
                    (weak != 0 || strong != 0).then(|| Instant::now() + GAMEPAD_WRITE_TIMEOUT);
                self.pulse = Some(series);
                Ok(())
            }
            OutputEvent::TriggerVibration(report) => {
                self.main_strong = to_magnitude(report.left_speed.to_primitive(), report.left_gain);
                self.main_weak = to_magnitude(report.right_speed.to_primitive(), report.right_gain);
                self.main_stop_at = None;
                Ok(self.write_state()?)
            }
            OutputEvent::TriggerVibrationExtended(report) => {
                self.main_strong = to_magnitude(report.left_speed.to_primitive(), report.left_gain);
                self.main_weak = to_magnitude(report.right_speed.to_primitive(), report.right_gain);
                self.trigger_lt =
                    to_magnitude(report.left_trigger_speed.to_primitive(), report.left_gain);
                self.trigger_rt =
                    to_magnitude(report.right_trigger_speed.to_primitive(), report.right_gain);
                self.main_stop_at = None;
                Ok(self.write_state()?)
            }
            OutputEvent::GenericRumble {
                weak_magnitude,
                strong_magnitude,
            } => {
                let strong = ((strong_magnitude as f64 / u16::MAX as f64) * 100.0).round() as u8;
                let weak = ((weak_magnitude as f64 / u16::MAX as f64) * 100.0).round() as u8;
                self.main_strong = strong;
                self.main_weak = weak;
                self.main_stop_at = None;
                Ok(self.write_state()?)
            }
        }
    }

    /// Upload the given force feedback effect data to the source device.
    /// Returns a device-specific id of the uploaded effect if it is
    /// successful.
    fn upload_effect(&mut self, effect: FFEffectData) -> Result<i16, OutputError> {
        log::debug!("Uploading FF effect data");
        let id = self.next_ff_effect_id();
        if id == -1 {
            return Err("Maximum FF effects uploaded".into());
        }
        self.ff_evdev_effects.insert(id, effect);
        Ok(id)
    }

    /// Update the effect with the given id using the given effect data.
    fn update_effect(&mut self, effect_id: i16, effect: FFEffectData) -> Result<(), OutputError> {
        log::debug!("Updating FF effect data with id {effect_id}");
        self.ff_evdev_effects.insert(effect_id, effect);
        Ok(())
    }

    /// Erase the effect with the given id from the source device.
    fn erase_effect(&mut self, effect_id: i16) -> Result<(), OutputError> {
        log::debug!("Erasing FF effect data");
        self.ff_evdev_effects.remove(&effect_id);
        Ok(())
    }
}

impl Debug for RogAlly {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RogAlly").finish()
    }
}

/// Calculates rumble magnitude from the given speed and gain and returns as a percentage
fn to_magnitude(speed: u16, gain: i8) -> u8 {
    let percent = speed as f64 * 100.0 / u16::MAX as f64;
    haptic_gain_level(gain, 100.0, percent) as u8
}

/// Round a microsecond duration up to a whole 10 ms step
fn step_up_us(us: u16) -> u16 {
    if us == 0 {
        return 0;
    }
    let stepped = (us as u32).div_ceil(10_000) * 10_000;
    stepped.min(u16::MAX as u32) as u16
}
