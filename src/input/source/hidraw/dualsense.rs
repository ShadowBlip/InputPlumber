use std::{collections::HashMap, error::Error, thread, time::Duration};

use evdev::{FFEffectData, FFEffectKind};
use hidapi::DeviceInfo;
use tokio::sync::mpsc::{self, error::TryRecvError};

use crate::{
    drivers::dualsense::{
        self,
        driver::{Driver, DS5_EDGE_PID, DS5_PID, DS5_VID},
    },
    input::{
        capability::{
            Capability, Gamepad, GamepadAxis, GamepadButton, GamepadTrigger, Touch, TouchButton,
            Touchpad,
        },
        composite_device::client::CompositeDeviceClient,
        event::{native::NativeEvent, value::InputValue, Event},
        output_event::OutputEvent,
        source::command::SourceCommand,
    },
};

/// Vendor ID
pub const VID: u16 = DS5_VID;
/// Product IDs
pub const PIDS: [u16; 2] = [DS5_EDGE_PID, DS5_PID];
/// How long to sleep before polling for events.
const POLL_RATE: Duration = Duration::from_millis(1);

/// Sony DualSense Controller implementation of HIDRaw interface
#[derive(Debug)]
pub struct DualSenseController {
    info: DeviceInfo,
    composite_device: CompositeDeviceClient,
    rx: Option<mpsc::Receiver<SourceCommand>>,
    device_id: String,
}

impl DualSenseController {
    /// Create a new [DualSenseController] source device.
    pub fn new(
        info: DeviceInfo,
        composite_device: CompositeDeviceClient,
        rx: mpsc::Receiver<SourceCommand>,
        device_id: String,
    ) -> Self {
        Self {
            info,
            composite_device,
            rx: Some(rx),
            device_id,
        }
    }

    /// Run the source device.
    pub async fn run(&mut self) -> Result<(), Box<dyn Error>> {
        log::debug!("Starting DualSense Controller driver");
        let rx = self.rx.take().unwrap();
        let composite_device = self.composite_device.clone();
        let path = self.info.path().to_string_lossy().to_string();
        let device_path = path.clone();
        let device_id = self.device_id.clone();

        // Spawn a blocking task to read the events
        let task =
            tokio::task::spawn_blocking(move || -> Result<(), Box<dyn Error + Send + Sync>> {
                let mut output_handler = DualSenseOutput::new(rx);
                let mut driver = Driver::new(device_path.clone())?;

                loop {
                    let events = driver.poll()?;
                    let native_events = translate_events(events);
                    for event in native_events {
                        // Don't send un-implemented events
                        if matches!(event.as_capability(), Capability::NotImplemented) {
                            continue;
                        }
                        let res = composite_device
                            .blocking_process_event(device_id.clone(), Event::Native(event));
                        if let Err(e) = res {
                            return Err(e.to_string().into());
                        }
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
        Ok(())
    }
}

/// Manages handling output events and source device commands
#[derive(Debug)]
struct DualSenseOutput {
    rx: mpsc::Receiver<SourceCommand>,
    ff_evdev_effects: HashMap<i16, FFEffectData>,
}

impl DualSenseOutput {
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
                                if let Err(e) = driver.write(report) {
                                    log::error!(
                                        "Failed to process dualsense output report: {:?}",
                                        e
                                    );
                                }
                            }
                            OutputEvent::Uinput(_) => (),
                        }
                    }
                    SourceCommand::Stop => return Err("Device stopped".into()),
                    SourceCommand::GetSampleRate(_, _) => (),
                    SourceCommand::GetSampleRatesAvail(_, _) => (),
                    SourceCommand::SetSampleRate(_, _, _) => (),
                    SourceCommand::GetScale(_, _) => (),
                    SourceCommand::GetScalesAvail(_, _) => (),
                    SourceCommand::SetScale(_, _, _) => (),
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
            log::trace!("Stopping rumble");
            if let Err(e) = device.rumble(0, 0) {
                log::debug!("Failed to stop rumble: {:?}", e);
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
                // Scale the rumble values to the DS5 values
                let left_speed = (strong_magnitude as f64 / u16::MAX as f64) * u8::MAX as f64;
                let left_speed = left_speed.round() as u8;
                let right_speed = (weak_magnitude as f64 / u16::MAX as f64) * u8::MAX as f64;
                let right_speed = right_speed.round() as u8;

                // Do rumble
                if let Err(e) = device.rumble(left_speed, right_speed) {
                    let err = format!("Failed to do rumble: {:?}", e);
                    return Err(err.into());
                }
            }
        }

        Ok(())
    }
}

/// Translate the given DualSense events into native events
fn translate_events(events: Vec<dualsense::event::Event>) -> Vec<NativeEvent> {
    events.into_iter().map(translate_event).collect()
}

/// Translate the given DualSense event into a native event
fn translate_event(event: dualsense::event::Event) -> NativeEvent {
    match event {
        dualsense::event::Event::Button(button) => match button {
            dualsense::event::ButtonEvent::Cross(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::South)),
                InputValue::Bool(value.pressed),
            ),
            dualsense::event::ButtonEvent::Circle(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::East)),
                InputValue::Bool(value.pressed),
            ),
            dualsense::event::ButtonEvent::Square(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::North)),
                InputValue::Bool(value.pressed),
            ),
            dualsense::event::ButtonEvent::Triangle(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::West)),
                InputValue::Bool(value.pressed),
            ),
            dualsense::event::ButtonEvent::Create(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::Select)),
                InputValue::Bool(value.pressed),
            ),
            dualsense::event::ButtonEvent::Options(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::Start)),
                InputValue::Bool(value.pressed),
            ),
            dualsense::event::ButtonEvent::Guide(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::Guide)),
                InputValue::Bool(value.pressed),
            ),
            dualsense::event::ButtonEvent::Mute(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::Mute)),
                InputValue::Bool(value.pressed),
            ),
            dualsense::event::ButtonEvent::DPadDown(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::DPadDown)),
                InputValue::Bool(value.pressed),
            ),
            dualsense::event::ButtonEvent::DPadUp(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::DPadUp)),
                InputValue::Bool(value.pressed),
            ),
            dualsense::event::ButtonEvent::DPadLeft(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::DPadLeft)),
                InputValue::Bool(value.pressed),
            ),
            dualsense::event::ButtonEvent::DPadRight(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::DPadRight)),
                InputValue::Bool(value.pressed),
            ),
            dualsense::event::ButtonEvent::L1(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::LeftBumper)),
                InputValue::Bool(value.pressed),
            ),
            dualsense::event::ButtonEvent::L2(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::LeftTrigger)),
                InputValue::Bool(value.pressed),
            ),
            dualsense::event::ButtonEvent::L3(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::LeftStick)),
                InputValue::Bool(value.pressed),
            ),
            dualsense::event::ButtonEvent::L4(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::LeftPaddle1)),
                InputValue::Bool(value.pressed),
            ),
            dualsense::event::ButtonEvent::L5(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::LeftPaddle2)),
                InputValue::Bool(value.pressed),
            ),
            dualsense::event::ButtonEvent::R1(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::RightBumper)),
                InputValue::Bool(value.pressed),
            ),
            dualsense::event::ButtonEvent::R2(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::RightTrigger)),
                InputValue::Bool(value.pressed),
            ),
            dualsense::event::ButtonEvent::R3(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::RightStick)),
                InputValue::Bool(value.pressed),
            ),
            dualsense::event::ButtonEvent::R4(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::RightPaddle1)),
                InputValue::Bool(value.pressed),
            ),
            dualsense::event::ButtonEvent::R5(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::RightPaddle2)),
                InputValue::Bool(value.pressed),
            ),
            dualsense::event::ButtonEvent::PadPress(value) => NativeEvent::new(
                Capability::Touchpad(Touchpad::CenterPad(Touch::Button(TouchButton::Press))),
                InputValue::Bool(value.pressed),
            ),
        },
        dualsense::event::Event::Accelerometer(accel) => match accel {
            dualsense::event::AccelerometerEvent::Accelerometer(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Accelerometer),
                InputValue::Vector3 {
                    x: Some(value.x as f64),
                    y: Some(value.y as f64),
                    z: Some(value.z as f64),
                },
            ),
            dualsense::event::AccelerometerEvent::Gyro(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Gyro),
                InputValue::Vector3 {
                    x: Some(value.x as f64),
                    y: Some(value.y as f64),
                    z: Some(value.z as f64),
                },
            ),
        },
        dualsense::event::Event::Axis(ref axis) => match axis {
            dualsense::event::AxisEvent::Pad(_) => NativeEvent::new(
                Capability::Touchpad(Touchpad::CenterPad(Touch::Motion)),
                normalize_axis_value(axis),
            ),
            dualsense::event::AxisEvent::LStick(_) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Axis(GamepadAxis::LeftStick)),
                normalize_axis_value(axis),
            ),
            dualsense::event::AxisEvent::RStick(_) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Axis(GamepadAxis::RightStick)),
                normalize_axis_value(axis),
            ),
        },
        dualsense::event::Event::Trigger(ref trigger) => match trigger {
            dualsense::event::TriggerEvent::L2(_) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::LeftTrigger)),
                normalize_trigger_value(trigger),
            ),
            dualsense::event::TriggerEvent::R2(_) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::RightTrigger)),
                normalize_trigger_value(trigger),
            ),
        },
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

/// Normalize the value to something between -1.0 and 1.0 based on the DualSense's
/// minimum and maximum axis ranges.
fn normalize_axis_value(event: &dualsense::event::AxisEvent) -> InputValue {
    match event {
        dualsense::event::AxisEvent::Pad(value) => {
            let min = 0.0;
            let max = dualsense::driver::DS5_TOUCHPAD_WIDTH;
            let x = normalize_signed_value(value.x as f64, min, max);
            let x = Some(x);

            let min = 0.0;
            let max = dualsense::driver::DS5_TOUCHPAD_HEIGHT;
            let y = normalize_signed_value(value.y as f64, min, max);
            let y = Some(y);

            InputValue::Touch {
                index: value.index,
                is_touching: value.is_touching,
                pressure: Some(1.0),
                x,
                y,
            }
        }
        dualsense::event::AxisEvent::LStick(value) => {
            let min = dualsense::driver::STICK_X_MIN;
            let max = dualsense::driver::STICK_X_MAX;
            let x = normalize_signed_value(value.x as f64, min, max);
            let x = Some(x);

            let min = dualsense::driver::STICK_Y_MIN;
            let max = dualsense::driver::STICK_Y_MAX;
            let y = normalize_signed_value(value.y as f64, min, max);
            let y = Some(y);

            InputValue::Vector2 { x, y }
        }
        dualsense::event::AxisEvent::RStick(value) => {
            let min = dualsense::driver::STICK_X_MIN;
            let max = dualsense::driver::STICK_X_MAX;
            let x = normalize_signed_value(value.x as f64, min, max);
            let x = Some(x);

            let min = dualsense::driver::STICK_Y_MIN;
            let max = dualsense::driver::STICK_Y_MAX;
            let y = normalize_signed_value(value.y as f64, min, max);
            let y = Some(y);

            InputValue::Vector2 { x, y }
        }
    }
}

/// Normalize the trigger value to something between 0.0 and 1.0 based on the Deck's
/// maximum axis ranges.
fn normalize_trigger_value(event: &dualsense::event::TriggerEvent) -> InputValue {
    match event {
        dualsense::event::TriggerEvent::L2(value) => {
            let max = dualsense::driver::TRIGGER_MAX;
            InputValue::Float(normalize_unsigned_value(value.value as f64, max))
        }
        dualsense::event::TriggerEvent::R2(value) => {
            let max = dualsense::driver::TRIGGER_MAX;
            InputValue::Float(normalize_unsigned_value(value.value as f64, max))
        }
    }
}

/// List of all capabilities that the DualSense driver implements
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
    Capability::Touchpad(Touchpad::CenterPad(Touch::Button(TouchButton::Press))),
    Capability::Touchpad(Touchpad::CenterPad(Touch::Button(TouchButton::Touch))),
    Capability::Touchpad(Touchpad::CenterPad(Touch::Motion)),
];
