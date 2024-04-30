//! The Deck implementation has been largly based off of the OpenSD project:
//! https://gitlab.com/open-sd/opensd/
use std::{
    collections::HashMap,
    error::Error,
    thread,
    time::{self, Duration},
};

use evdev::{FFEffectData, FFEffectKind};
use hidapi::DeviceInfo;
use tokio::sync::mpsc::{self, error::TryRecvError};

use crate::{
    drivers::{
        dualsense::hid_report::SetStatePackedOutputData,
        steam_deck::{
            self,
            driver::{Driver, ACCEL_SCALE},
            hid_report::LIZARD_SLEEP_SEC,
        },
    },
    input::{
        capability::{
            Capability, Gamepad, GamepadAxis, GamepadButton, GamepadTrigger, Touch, TouchButton,
            Touchpad,
        },
        composite_device::Command,
        event::{native::NativeEvent, value::InputValue, Event},
        output_event::OutputEvent,
        source::SourceCommand,
    },
};

/// Vendor ID
pub const VID: u16 = 0x28de;
/// Product ID
pub const PID: u16 = 0x1205;
/// How long to sleep before polling for events.
const POLL_RATE: Duration = Duration::from_micros(250);

/// Steam Deck Controller implementation of HIDRaw interface
#[derive(Debug)]
pub struct DeckController {
    info: DeviceInfo,
    composite_tx: mpsc::Sender<Command>,
    rx: Option<mpsc::Receiver<SourceCommand>>,
    device_id: String,
}

impl DeckController {
    pub fn new(
        info: DeviceInfo,
        composite_tx: mpsc::Sender<Command>,
        rx: mpsc::Receiver<SourceCommand>,
        device_id: String,
    ) -> Self {
        Self {
            info,
            composite_tx,
            rx: Some(rx),
            device_id,
        }
    }

    pub async fn run(&mut self) -> Result<(), Box<dyn Error>> {
        log::debug!("Starting Steam Deck Controller driver");
        let rx = self.rx.take().unwrap();
        let tx = self.composite_tx.clone();
        let path = self.info.path().to_string_lossy().to_string();
        let device_path = path.clone();
        let device_id = self.device_id.clone();

        // Spawn a blocking task to handle lizard mode
        let lizard_task =
            tokio::task::spawn_blocking(move || -> Result<(), Box<dyn Error + Send + Sync>> {
                let driver = Driver::new(path.clone())?;
                loop {
                    // Keep the lizard asleep
                    driver.handle_lizard_mode()?;

                    // Polling interval is about 4ms so we can sleep a little
                    let duration = time::Duration::from_secs(LIZARD_SLEEP_SEC as u64);
                    thread::sleep(duration);
                }
            });

        // Spawn a blocking task to read the events
        let task =
            tokio::task::spawn_blocking(move || -> Result<(), Box<dyn Error + Send + Sync>> {
                let mut output_handler = DeckOutput::new(rx);
                let mut driver = Driver::new(device_path.clone())?;
                loop {
                    let events = driver.poll()?;
                    let native_events = translate_events(events);
                    for event in native_events {
                        // Don't send un-implemented events
                        if matches!(event.as_capability(), Capability::NotImplemented) {
                            continue;
                        }
                        tx.blocking_send(Command::ProcessEvent(
                            device_id.clone(),
                            Event::Native(event),
                        ))?;
                    }

                    // Receive commands/output events
                    if let Err(e) = output_handler.receive_commands(&mut driver) {
                        log::debug!("Error receiving commands: {:?}", e);
                        break;
                    }

                    // Polling interval is about 4ms so we can sleep a little
                    thread::sleep(POLL_RATE);
                }

                Ok(())
            });

        // Wait for the task to finish
        if let Err(e) = task.await? {
            return Err(e.to_string().into());
        }
        if let Err(e) = lizard_task.await? {
            return Err(e.to_string().into());
        }

        log::debug!("Steam Deck Controller driver stopped");

        Ok(())
    }
}

/// Manages handling output events and source device commands
#[derive(Debug)]
struct DeckOutput {
    rx: mpsc::Receiver<SourceCommand>,
    ff_evdev_effects: HashMap<i16, FFEffectData>,
}

impl DeckOutput {
    pub fn new(rx: mpsc::Receiver<SourceCommand>) -> Self {
        Self {
            rx,
            ff_evdev_effects: HashMap::new(),
        }
    }

    /// Read commands sent to this device from the channel until it is
    /// empty.
    fn receive_commands(&mut self, driver: &mut Driver) -> Result<(), Box<dyn Error>> {
        const MAX_COMMANDS: u8 = 64;
        let mut commands_processed = 0;
        loop {
            match self.rx.try_recv() {
                Ok(cmd) => match cmd {
                    SourceCommand::UploadEffect(data, composite_dev) => {
                        self.upload_ff_effect(data, composite_dev);
                    }
                    SourceCommand::UpdateEffect(id, data) => {
                        self.update_ff_effect(id, data);
                    }
                    SourceCommand::EraseEffect(id, composite_dev) => {
                        self.erase_ff_effect(id, composite_dev);
                    }
                    SourceCommand::WriteEvent(event) => {
                        log::trace!("Received output event: {:?}", event);
                        match event {
                            OutputEvent::Evdev(input_event) => {
                                if let Err(e) = self.process_evdev_ff(driver, input_event) {
                                    log::error!("Failed to write output event: {:?}", e);
                                }
                            }
                            OutputEvent::DualSense(report) => {
                                log::debug!("Received DualSense output report");
                                if report.use_rumble_not_haptics
                                    || report.enable_improved_rumble_emulation
                                {
                                    if let Err(e) = self.process_dualsense_ff(driver, report) {
                                        log::error!(
                                            "Failed to process dualsense output report: {:?}",
                                            e
                                        );
                                    }
                                }
                            }
                            OutputEvent::Uinput(_) => (),
                        }
                    }
                    SourceCommand::Stop => return Err("Device stopped".into()),
                },
                Err(e) => match e {
                    TryRecvError::Empty => return Ok(()),
                    TryRecvError::Disconnected => {
                        log::debug!("Receive channel disconnected");
                        return Err("Receive channel disconnected".into());
                    }
                },
            };

            commands_processed += 1;
            if commands_processed >= MAX_COMMANDS {
                return Ok(());
            }
        }
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

    /// Upload the given effect data to the device and send the result to
    /// the composite device.
    fn upload_ff_effect(
        &mut self,
        data: evdev::FFEffectData,
        composite_dev: std::sync::mpsc::Sender<Result<i16, Box<dyn Error + Send + Sync>>>,
    ) {
        log::debug!("Uploading FF effect data");
        let id = self.next_ff_effect_id();
        if id == -1 {
            if let Err(e) = composite_dev.send(Err("Maximum FF effects uploaded".into())) {
                log::error!("Failed to send upload result: {:?}", e);
            }
            return;
        }

        self.ff_evdev_effects.insert(id, data);
        if let Err(e) = composite_dev.send(Ok(id)) {
            log::error!("Failed to send upload result: {:?}", e);
        }
    }

    /// Update the effect with the given id using the given effect data.
    fn update_ff_effect(&mut self, id: i16, data: FFEffectData) {
        log::debug!("Updating FF effect data with id {id}");
        self.ff_evdev_effects.insert(id, data);
    }

    /// Erase the effect from the device with the given effect id and send the
    /// result to the composite device.
    fn erase_ff_effect(
        &mut self,
        id: i16,
        composite_dev: std::sync::mpsc::Sender<Result<(), Box<dyn Error + Send + Sync>>>,
    ) {
        log::debug!("Erasing FF effect data");
        self.ff_evdev_effects.remove(&id);
        if let Err(err) = composite_dev.send(Ok(())) {
            log::error!("Failed to send erase result: {:?}", err);
        }
    }

    /// Process evdev force feedback events. Evdev events will send events with
    /// the effect id set in the 'code' field.
    fn process_evdev_ff(
        &self,
        device: &mut Driver,
        input_event: evdev::InputEvent,
    ) -> Result<(), Box<dyn Error>> {
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

        // The value determines if the effect should be playing or not.
        if value == 0 {
            log::trace!("Stopping haptic rumble");
            if let Err(e) = device.haptic_rumble(0, 0, 0, 0, 0) {
                log::debug!("Failed to stop haptic rumble: {:?}", e);
                return Ok(());
            }
            return Ok(());
        }

        // Perform the rumble based on the effect
        // TODO: handle effect duration, etc.
        match effect_data.kind {
            FFEffectKind::Damper => (),
            FFEffectKind::Inertia => (),
            FFEffectKind::Constant {
                level: _,
                envelope: _,
            } => (),
            FFEffectKind::Ramp {
                start_level: _,
                end_level: _,
                envelope: _,
            } => (),
            FFEffectKind::Periodic {
                waveform: _,
                period: _,
                magnitude: _,
                offset: _,
                phase: _,
                envelope: _,
            } => (),
            FFEffectKind::Spring { condition: _ } => (),
            FFEffectKind::Friction { condition: _ } => (),
            FFEffectKind::Rumble {
                strong_magnitude,
                weak_magnitude,
            } => {
                // Set rumble values based on the effect data
                let intensity = 0;
                let left_speed = strong_magnitude;
                let right_speed = weak_magnitude;
                let mut left_gain = 130;
                let mut right_gain = 130;
                if left_speed == 0 {
                    left_gain = 0;
                }
                if right_speed == 0 {
                    right_gain = 0;
                }

                // Do rumble
                if let Err(e) =
                    device.haptic_rumble(intensity, left_speed, right_speed, left_gain, right_gain)
                {
                    let err = format!("Failed to do haptic rumble: {:?}", e);
                    return Err(err.into());
                }
            }
        }

        Ok(())
    }

    /// Process dualsense force feedback output reports
    fn process_dualsense_ff(
        &mut self,
        driver: &mut Driver,
        report: SetStatePackedOutputData,
    ) -> Result<(), Box<dyn Error>> {
        // Set the rumble values based on the DualSense output report
        let intensity = 0;
        let left_speed = report.rumble_emulation_left as u16 * 256;
        let right_speed = report.rumble_emulation_right as u16 * 256;
        let mut left_gain = 130;
        let mut right_gain = 130;
        if left_speed == 0 {
            left_gain = 0;
        }
        if right_speed == 0 {
            right_gain = 0;
        }

        if let Err(e) =
            driver.haptic_rumble(intensity, left_speed, right_speed, left_gain, right_gain)
        {
            let err = format!("Failed to do haptic rumble: {:?}", e);
            return Err(err.into());
        }

        Ok(())
    }
}

/// Returns a value between -1.0 and 1.0 based on the given value with its
/// minimum and maximum values.
fn normalize_signed_value(raw_value: f64, min: f64, max: f64) -> f64 {
    let mid = (max + min) / 2.0;
    let event_value = raw_value - mid;

    // Normalize the value
    if event_value >= 0.0 {
        let maximum = max - mid;
        event_value / maximum
    } else {
        let minimum = min - mid;
        let value = event_value / minimum;
        -value
    }
}

// Returns a value between 0.0 and 1.0 based on the given value with its
// maximum.
fn normalize_unsigned_value(raw_value: f64, max: f64) -> f64 {
    raw_value / max
}

/// Normalize the value to something between -1.0 and 1.0 based on the Deck's
/// minimum and maximum axis ranges.
fn normalize_axis_value(event: steam_deck::event::AxisEvent) -> InputValue {
    match event {
        steam_deck::event::AxisEvent::LPad(value) => {
            let min = steam_deck::hid_report::PAD_X_MIN;
            let max = steam_deck::hid_report::PAD_X_MAX;
            let x = normalize_signed_value(value.x as f64, min, max);
            let x = Some(x);

            let min = steam_deck::hid_report::PAD_Y_MAX; // uses inverted Y-axis
            let max = steam_deck::hid_report::PAD_Y_MIN;
            let y = normalize_signed_value(value.y as f64, min, max);
            let y = Some(-y); // Y-Axis is inverted

            InputValue::Touch {
                index: value.index as u8,
                is_touching: value.is_touching,
                x,
                y,
            }
        }
        steam_deck::event::AxisEvent::RPad(value) => {
            let min = steam_deck::hid_report::PAD_X_MIN;
            let max = steam_deck::hid_report::PAD_X_MAX;
            let x = normalize_signed_value(value.x as f64, min, max);
            let x = Some(x);

            let min = steam_deck::hid_report::PAD_Y_MAX; // uses inverted Y-axis
            let max = steam_deck::hid_report::PAD_Y_MIN;
            let y = normalize_signed_value(value.y as f64, min, max);
            let y = Some(-y); // Y-Axis is inverted

            InputValue::Touch {
                index: value.index as u8,
                is_touching: value.is_touching,
                x,
                y,
            }
        }
        steam_deck::event::AxisEvent::LStick(value) => {
            let min = steam_deck::hid_report::STICK_X_MIN;
            let max = steam_deck::hid_report::STICK_X_MAX;
            let x = normalize_signed_value(value.x as f64, min, max);
            let x = Some(x);

            let min = steam_deck::hid_report::STICK_Y_MAX; // uses inverted Y-axis
            let max = steam_deck::hid_report::STICK_Y_MIN;
            let y = normalize_signed_value(value.y as f64, min, max);
            let y = Some(-y); // Y-Axis is inverted

            InputValue::Vector2 { x, y }
        }
        steam_deck::event::AxisEvent::RStick(value) => {
            let min = steam_deck::hid_report::STICK_X_MIN;
            let max = steam_deck::hid_report::STICK_X_MAX;
            let x = normalize_signed_value(value.x as f64, min, max);
            let x = Some(x);

            let min = steam_deck::hid_report::STICK_Y_MAX; // uses inverted Y-axis
            let max = steam_deck::hid_report::STICK_Y_MIN;
            let y = normalize_signed_value(value.y as f64, min, max);
            let y = Some(-y); // Y-Axis is inverted

            InputValue::Vector2 { x, y }
        }
    }
}

/// Normalize the trigger value to something between 0.0 and 1.0 based on the Deck's
/// maximum axis ranges.
fn normalize_trigger_value(event: steam_deck::event::TriggerEvent) -> InputValue {
    match event {
        steam_deck::event::TriggerEvent::LTrigger(value) => {
            let max = steam_deck::hid_report::TRIGG_MAX;
            InputValue::Float(normalize_unsigned_value(value.value as f64, max))
        }
        steam_deck::event::TriggerEvent::RTrigger(value) => {
            let max = steam_deck::hid_report::TRIGG_MAX;
            InputValue::Float(normalize_unsigned_value(value.value as f64, max))
        }
        steam_deck::event::TriggerEvent::LPadForce(value) => {
            let max = steam_deck::hid_report::PAD_FORCE_MAX;
            InputValue::Float(normalize_unsigned_value(value.value as f64, max))
        }
        steam_deck::event::TriggerEvent::RPadForce(value) => {
            let max = steam_deck::hid_report::PAD_FORCE_MAX;
            InputValue::Float(normalize_unsigned_value(value.value as f64, max))
        }
        steam_deck::event::TriggerEvent::LStickForce(value) => {
            let max = steam_deck::hid_report::STICK_FORCE_MAX;
            InputValue::Float(normalize_unsigned_value(value.value as f64, max))
        }
        steam_deck::event::TriggerEvent::RStickForce(value) => {
            let max = steam_deck::hid_report::STICK_FORCE_MAX;
            InputValue::Float(normalize_unsigned_value(value.value as f64, max))
        }
    }
}

/// Translate the given Steam Deck events into native events
fn translate_events(events: Vec<steam_deck::event::Event>) -> Vec<NativeEvent> {
    events.into_iter().map(translate_event).collect()
}

/// Translate the given Steam Deck event into a native event
fn translate_event(event: steam_deck::event::Event) -> NativeEvent {
    match event {
        steam_deck::event::Event::Button(button) => match button {
            steam_deck::event::ButtonEvent::A(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::South)),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::X(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::North)),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::B(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::East)),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::Y(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::West)),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::Menu(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::Start)),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::Options(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::Select)),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::Steam(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::Guide)),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::QuickAccess(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::QuickAccess)),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::DPadDown(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::DPadDown)),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::DPadUp(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::DPadUp)),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::DPadLeft(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::DPadLeft)),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::DPadRight(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::DPadRight)),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::L1(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::LeftBumper)),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::L2(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::LeftTrigger)),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::L3(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::LeftStick)),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::L4(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::LeftPaddle1)),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::L5(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::LeftPaddle2)),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::R1(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::RightBumper)),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::R2(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::RightTrigger)),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::R3(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::RightStick)),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::R4(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::RightPaddle1)),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::R5(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::RightPaddle2)),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::RPadTouch(value) => NativeEvent::new(
                Capability::Touchpad(Touchpad::RightPad(Touch::Button(TouchButton::Touch))),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::LPadTouch(value) => NativeEvent::new(
                Capability::Touchpad(Touchpad::LeftPad(Touch::Button(TouchButton::Touch))),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::RPadPress(value) => NativeEvent::new(
                Capability::Touchpad(Touchpad::RightPad(Touch::Button(TouchButton::Press))),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::LPadPress(value) => NativeEvent::new(
                Capability::Touchpad(Touchpad::LeftPad(Touch::Button(TouchButton::Press))),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::RStickTouch(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::RightStickTouch)),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::LStickTouch(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::LeftStickTouch)),
                InputValue::Bool(value.pressed),
            ),
        },
        steam_deck::event::Event::Accelerometer(accel) => match accel {
            steam_deck::event::AccelerometerEvent::Accelerometer(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Accelerometer),
                InputValue::Vector3 {
                    x: Some(value.x as f64 * ACCEL_SCALE),
                    y: Some(value.y as f64 * ACCEL_SCALE),
                    z: Some(value.z as f64 * ACCEL_SCALE),
                },
            ),
            steam_deck::event::AccelerometerEvent::Attitude(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Gyro),
                InputValue::Vector3 {
                    x: Some(value.x as f64),
                    y: Some(value.y as f64),
                    z: Some(value.z as f64),
                },
            ),
        },
        steam_deck::event::Event::Axis(axis) => match axis.clone() {
            steam_deck::event::AxisEvent::LPad(_) => NativeEvent::new(
                Capability::Touchpad(Touchpad::LeftPad(Touch::Motion)),
                normalize_axis_value(axis),
            ),
            steam_deck::event::AxisEvent::RPad(_) => NativeEvent::new(
                Capability::Touchpad(Touchpad::RightPad(Touch::Motion)),
                normalize_axis_value(axis),
            ),
            steam_deck::event::AxisEvent::LStick(_) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Axis(GamepadAxis::LeftStick)),
                normalize_axis_value(axis),
            ),
            steam_deck::event::AxisEvent::RStick(_) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Axis(GamepadAxis::RightStick)),
                normalize_axis_value(axis),
            ),
        },
        steam_deck::event::Event::Trigger(trigg) => match trigg.clone() {
            steam_deck::event::TriggerEvent::LTrigger(_) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::LeftTrigger)),
                normalize_trigger_value(trigg),
            ),
            steam_deck::event::TriggerEvent::RTrigger(_) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::RightTrigger)),
                normalize_trigger_value(trigg),
            ),
            steam_deck::event::TriggerEvent::LPadForce(_) => {
                NativeEvent::new(Capability::NotImplemented, normalize_trigger_value(trigg))
            }
            steam_deck::event::TriggerEvent::RPadForce(_) => {
                NativeEvent::new(Capability::NotImplemented, normalize_trigger_value(trigg))
            }
            steam_deck::event::TriggerEvent::LStickForce(_) => {
                NativeEvent::new(Capability::NotImplemented, normalize_trigger_value(trigg))
            }
            steam_deck::event::TriggerEvent::RStickForce(_) => {
                NativeEvent::new(Capability::NotImplemented, normalize_trigger_value(trigg))
            }
        },
    }
}

/// List of all capabilities that the Steam Deck driver implements
pub const CAPABILITIES: &[Capability] = &[
    Capability::Gamepad(Gamepad::Accelerometer),
    Capability::Gamepad(Gamepad::Axis(GamepadAxis::LeftStick)),
    Capability::Gamepad(Gamepad::Axis(GamepadAxis::RightStick)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::DPadDown)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::DPadLeft)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::DPadRight)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::DPadUp)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::East)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::Guide)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftBumper)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftPaddle1)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftPaddle2)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftStick)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftStickTouch)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftTrigger)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::North)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::QuickAccess)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::RightBumper)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::RightPaddle1)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::RightPaddle2)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::RightStick)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::RightStickTouch)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::RightTrigger)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::Select)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::South)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::Start)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::West)),
    Capability::Gamepad(Gamepad::Gyro),
    Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::LeftTrigger)),
    Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::RightTrigger)),
    Capability::Touchpad(Touchpad::LeftPad(Touch::Button(TouchButton::Press))),
    Capability::Touchpad(Touchpad::LeftPad(Touch::Button(TouchButton::Touch))),
    Capability::Touchpad(Touchpad::LeftPad(Touch::Motion)),
    Capability::Touchpad(Touchpad::RightPad(Touch::Button(TouchButton::Press))),
    Capability::Touchpad(Touchpad::RightPad(Touch::Button(TouchButton::Touch))),
    Capability::Touchpad(Touchpad::RightPad(Touch::Motion)),
];
