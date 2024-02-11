//! The Deck implementation has been largly based off of the OpenSD project:
//! https://gitlab.com/open-sd/opensd/
use std::{error::Error, thread, time};

use hidapi::DeviceInfo;
use tokio::sync::broadcast;

use crate::{
    drivers::steam_deck::{self, driver::Driver},
    input::{
        capability::{Capability, Gamepad, GamepadAxis, GamepadButton, GamepadTrigger},
        composite_device::Command,
        event::{
            native::{InputValue, NativeEvent},
            Event,
        },
    },
};

pub const VID: u16 = 0x28de;
pub const PID: u16 = 0x1205;

/// Steam Deck Controller implementation of HIDRaw interface
#[derive(Debug)]
pub struct DeckController {
    info: DeviceInfo,
    composite_tx: broadcast::Sender<Command>,
}

impl DeckController {
    pub fn new(info: DeviceInfo, composite_tx: broadcast::Sender<Command>) -> Self {
        Self { info, composite_tx }
    }

    pub async fn run(&self) -> Result<(), Box<dyn Error>> {
        log::debug!("Starting Steam Deck Controller driver");
        let path = self.info.path().to_string_lossy().to_string();
        let tx = self.composite_tx.clone();

        // Spawn a blocking task to read the events
        let task =
            tokio::task::spawn_blocking(move || -> Result<(), Box<dyn Error + Send + Sync>> {
                let mut driver = Driver::new(path.clone())?;
                loop {
                    let events = driver.poll()?;
                    let native_events = translate_events(events);
                    for event in native_events {
                        tx.send(Command::ProcessEvent(Event::Native(event)))?;
                    }

                    // Polling interval is about 4ms so we can sleep a little
                    let duration = time::Duration::from_micros(250);
                    thread::sleep(duration);
                }
            });

        // Wait for the task to finish
        if let Err(e) = task.await? {
            return Err(e.to_string().into());
        }

        log::debug!("Steam Deck Controller driver stopped");

        Ok(())
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
                Capability::Gamepad(Gamepad::Button(GamepadButton::Base)),
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
                Capability::Gamepad(Gamepad::Button(GamepadButton::LeftBumper)),
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
                Capability::Gamepad(Gamepad::Button(GamepadButton::RightTouchpadTouch)),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::LPadTouch(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::LeftTouchpadTouch)),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::RPadPress(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::RightTouchpadPress)),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::LPadPress(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::LeftTouchpadPress)),
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
                Capability::NotImplemented,
                InputValue::Vector3 {
                    x: value.x as f64,
                    y: value.y as f64,
                    z: value.z as f64,
                },
            ),
            steam_deck::event::AccelerometerEvent::Attitude(value) => NativeEvent::new(
                Capability::NotImplemented,
                InputValue::Vector3 {
                    x: value.x as f64,
                    y: value.y as f64,
                    z: value.z as f64,
                },
            ),
        },
        steam_deck::event::Event::Axis(axis) => match axis {
            steam_deck::event::AxisEvent::LPad(value) => NativeEvent::new(
                Capability::NotImplemented,
                InputValue::Vector2 {
                    x: value.x as f64,
                    y: value.y as f64,
                },
            ),
            steam_deck::event::AxisEvent::RPad(value) => NativeEvent::new(
                Capability::NotImplemented,
                InputValue::Vector2 {
                    x: value.x as f64,
                    y: value.y as f64,
                },
            ),
            steam_deck::event::AxisEvent::LStick(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Axis(GamepadAxis::LeftStick)),
                InputValue::Vector2 {
                    x: value.x as f64,
                    y: value.y as f64,
                },
            ),
            steam_deck::event::AxisEvent::RStick(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Axis(GamepadAxis::RightStick)),
                InputValue::Vector2 {
                    x: value.x as f64,
                    y: value.y as f64,
                },
            ),
        },
        steam_deck::event::Event::Trigger(trigg) => match trigg {
            steam_deck::event::TriggerEvent::LTrigger(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::LeftTrigger)),
                InputValue::UInt(value.value as u32),
            ),
            steam_deck::event::TriggerEvent::RTrigger(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::RightTrigger)),
                InputValue::UInt(value.value as u32),
            ),
            steam_deck::event::TriggerEvent::LPadForce(value) => NativeEvent::new(
                Capability::NotImplemented,
                InputValue::UInt(value.value as u32),
            ),
            steam_deck::event::TriggerEvent::RPadForce(value) => NativeEvent::new(
                Capability::NotImplemented,
                InputValue::UInt(value.value as u32),
            ),
            steam_deck::event::TriggerEvent::LStickForce(value) => NativeEvent::new(
                Capability::NotImplemented,
                InputValue::UInt(value.value as u32),
            ),
            steam_deck::event::TriggerEvent::RStickForce(value) => NativeEvent::new(
                Capability::NotImplemented,
                InputValue::UInt(value.value as u32),
            ),
        },
    }
}
