//! The Deck implementation has been largly based off of the OpenSD project:
//! https://gitlab.com/open-sd/opensd/
use std::error::Error;

use hidapi::DeviceInfo;
use tokio::sync::broadcast;

use crate::{
    drivers::steam_deck::{self, driver::Driver},
    input::{
        composite_device::Command,
        event::{native::NativeEvent, Event},
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
            steam_deck::event::ButtonEvent::A(value) => todo!(),
            steam_deck::event::ButtonEvent::X(_) => todo!(),
            steam_deck::event::ButtonEvent::B(_) => todo!(),
            steam_deck::event::ButtonEvent::Y(_) => todo!(),
            steam_deck::event::ButtonEvent::Menu(_) => todo!(),
            steam_deck::event::ButtonEvent::Options(_) => todo!(),
            steam_deck::event::ButtonEvent::Steam(_) => todo!(),
            steam_deck::event::ButtonEvent::QuickAccess(_) => todo!(),
            steam_deck::event::ButtonEvent::DPadDown(_) => todo!(),
            steam_deck::event::ButtonEvent::DPadUp(_) => todo!(),
            steam_deck::event::ButtonEvent::DPadLeft(_) => todo!(),
            steam_deck::event::ButtonEvent::DPadRight(_) => todo!(),
            steam_deck::event::ButtonEvent::L1(_) => todo!(),
            steam_deck::event::ButtonEvent::L2(_) => todo!(),
            steam_deck::event::ButtonEvent::L3(_) => todo!(),
            steam_deck::event::ButtonEvent::L4(_) => todo!(),
            steam_deck::event::ButtonEvent::L5(_) => todo!(),
            steam_deck::event::ButtonEvent::R1(_) => todo!(),
            steam_deck::event::ButtonEvent::R2(_) => todo!(),
            steam_deck::event::ButtonEvent::R3(_) => todo!(),
            steam_deck::event::ButtonEvent::R4(_) => todo!(),
            steam_deck::event::ButtonEvent::R5(_) => todo!(),
            steam_deck::event::ButtonEvent::RPadTouch(_) => todo!(),
            steam_deck::event::ButtonEvent::LPadTouch(_) => todo!(),
            steam_deck::event::ButtonEvent::RPadPress(_) => todo!(),
            steam_deck::event::ButtonEvent::LPadPress(_) => todo!(),
            steam_deck::event::ButtonEvent::RStickTouch(_) => todo!(),
            steam_deck::event::ButtonEvent::LStickTouch(_) => todo!(),
        },
        steam_deck::event::Event::Accelerometer(accel) => match accel {
            steam_deck::event::AccelerometerEvent::Accelerometer(_) => todo!(),
            steam_deck::event::AccelerometerEvent::Attitude(_) => todo!(),
        },
        steam_deck::event::Event::Axis(axis) => match axis {
            steam_deck::event::AxisEvent::LPad(_) => todo!(),
            steam_deck::event::AxisEvent::RPad(_) => todo!(),
            steam_deck::event::AxisEvent::LStick(_) => todo!(),
            steam_deck::event::AxisEvent::RStick(_) => todo!(),
        },
        steam_deck::event::Event::Trigger(trigg) => match trigg {
            steam_deck::event::TriggerEvent::LTrigger(_) => todo!(),
            steam_deck::event::TriggerEvent::RTrigger(_) => todo!(),
            steam_deck::event::TriggerEvent::LPadForce(_) => todo!(),
            steam_deck::event::TriggerEvent::RPadForce(_) => todo!(),
            steam_deck::event::TriggerEvent::LStickForce(_) => todo!(),
            steam_deck::event::TriggerEvent::RStickForce(_) => todo!(),
        },
    }
}
