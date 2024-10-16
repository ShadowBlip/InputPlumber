use std::os::fd::AsRawFd;
use std::time::Duration;
use std::{collections::HashMap, error::Error};

use evdev::{
    uinput::{VirtualDevice, VirtualDeviceBuilder},
    AbsInfo, AbsoluteAxisCode, AttributeSet, BusType, FFEffectCode, InputId, KeyCode,
    UinputAbsSetup,
};
use evdev::{EventSummary, FFStatusCode, InputEvent, UInputCode};
use nix::fcntl::{FcntlArg, OFlag};

use crate::input::capability::{Capability, Gamepad, GamepadAxis, GamepadButton, GamepadTrigger};
use crate::input::composite_device::client::CompositeDeviceClient;
use crate::input::event::evdev::EvdevEvent;
use crate::input::event::native::{NativeEvent, ScheduledNativeEvent};
use crate::input::output_capability::OutputCapability;
use crate::input::output_event::{OutputEvent, UinputOutputEvent};

use super::{InputError, OutputError, TargetInputDevice, TargetOutputDevice};

#[derive(Debug)]
pub struct XBox360Controller {
    device: VirtualDevice,
    axis_map: HashMap<AbsoluteAxisCode, AbsInfo>,
    queued_events: Vec<ScheduledNativeEvent>,
}

impl XBox360Controller {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let axis_map = XBox360Controller::get_abs_info();
        let device = XBox360Controller::create_virtual_device(&axis_map)?;
        Ok(Self {
            device,
            axis_map,
            queued_events: Vec::new(),
        })
    }

    /// Return a hashmap of ABS information for this virtual device. This information
    /// is used to denormalize input event values.
    fn get_abs_info() -> HashMap<AbsoluteAxisCode, AbsInfo> {
        let mut axes_info = HashMap::new();

        let joystick_setup = AbsInfo::new(0, -32768, 32767, 16, 128, 1);
        axes_info.insert(AbsoluteAxisCode::ABS_X, joystick_setup);
        axes_info.insert(AbsoluteAxisCode::ABS_Y, joystick_setup);
        axes_info.insert(AbsoluteAxisCode::ABS_RX, joystick_setup);
        axes_info.insert(AbsoluteAxisCode::ABS_RY, joystick_setup);

        let triggers_setup = AbsInfo::new(0, 0, 255, 0, 0, 1);
        axes_info.insert(AbsoluteAxisCode::ABS_Z, triggers_setup);
        axes_info.insert(AbsoluteAxisCode::ABS_RZ, triggers_setup);

        let dpad_setup = AbsInfo::new(0, -1, 1, 0, 0, 1);
        axes_info.insert(AbsoluteAxisCode::ABS_HAT0X, dpad_setup);
        axes_info.insert(AbsoluteAxisCode::ABS_HAT0Y, dpad_setup);

        axes_info
    }

    /// Create the virtual device to emulate
    fn create_virtual_device(
        axis_map: &HashMap<AbsoluteAxisCode, AbsInfo>,
    ) -> Result<VirtualDevice, Box<dyn Error>> {
        // Setup Key inputs
        let mut keys = AttributeSet::<KeyCode>::new();
        keys.insert(KeyCode::BTN_SOUTH);
        keys.insert(KeyCode::BTN_EAST);
        keys.insert(KeyCode::BTN_NORTH);
        keys.insert(KeyCode::BTN_WEST);
        keys.insert(KeyCode::BTN_TL);
        keys.insert(KeyCode::BTN_TR);
        keys.insert(KeyCode::BTN_SELECT);
        keys.insert(KeyCode::BTN_START);
        keys.insert(KeyCode::BTN_MODE);
        keys.insert(KeyCode::BTN_THUMBL);
        keys.insert(KeyCode::BTN_THUMBR);
        keys.insert(KeyCode::BTN_TRIGGER_HAPPY1);
        keys.insert(KeyCode::BTN_TRIGGER_HAPPY2);
        keys.insert(KeyCode::BTN_TRIGGER_HAPPY3);
        keys.insert(KeyCode::BTN_TRIGGER_HAPPY4);

        // Setup ABS inputs
        let Some(joystick_setup) = axis_map.get(&AbsoluteAxisCode::ABS_X) else {
            return Err("No axis information for ABS_X".to_string().into());
        };
        let abs_x = UinputAbsSetup::new(AbsoluteAxisCode::ABS_X, *joystick_setup);
        let abs_y = UinputAbsSetup::new(AbsoluteAxisCode::ABS_Y, *joystick_setup);
        let abs_rx = UinputAbsSetup::new(AbsoluteAxisCode::ABS_RX, *joystick_setup);
        let abs_ry = UinputAbsSetup::new(AbsoluteAxisCode::ABS_RY, *joystick_setup);
        let Some(triggers_setup) = axis_map.get(&AbsoluteAxisCode::ABS_Z) else {
            return Err("No axis information for ABS_Z".to_string().into());
        };
        let abs_z = UinputAbsSetup::new(AbsoluteAxisCode::ABS_Z, *triggers_setup);
        let abs_rz = UinputAbsSetup::new(AbsoluteAxisCode::ABS_RZ, *triggers_setup);
        let Some(dpad_setup) = axis_map.get(&AbsoluteAxisCode::ABS_HAT0X) else {
            return Err("No axis information for ABS_HAT0X".to_string().into());
        };
        let abs_hat0x = UinputAbsSetup::new(AbsoluteAxisCode::ABS_HAT0X, *dpad_setup);
        let abs_hat0y = UinputAbsSetup::new(AbsoluteAxisCode::ABS_HAT0Y, *dpad_setup);

        // Setup Force Feedback
        let mut ff = AttributeSet::<FFEffectCode>::new();
        ff.insert(FFEffectCode::FF_RUMBLE);
        ff.insert(FFEffectCode::FF_PERIODIC);
        ff.insert(FFEffectCode::FF_SQUARE);
        ff.insert(FFEffectCode::FF_TRIANGLE);
        ff.insert(FFEffectCode::FF_SINE);
        ff.insert(FFEffectCode::FF_GAIN);

        // Identify to the kernel as an Xbox One Elite
        let id = InputId::new(BusType(3), 0x045e, 0x028e, 0x0001);

        // Build the device
        let device = VirtualDeviceBuilder::new()?
            .name("Microsoft X-Box 360 pad")
            .input_id(id)
            .with_keys(&keys)?
            .with_absolute_axis(&abs_x)?
            .with_absolute_axis(&abs_y)?
            .with_absolute_axis(&abs_rx)?
            .with_absolute_axis(&abs_ry)?
            .with_absolute_axis(&abs_z)?
            .with_absolute_axis(&abs_rz)?
            .with_absolute_axis(&abs_hat0x)?
            .with_absolute_axis(&abs_hat0y)?
            .with_ff(&ff)?
            .with_ff_effects_max(16)
            .build()?;

        // Set the device to do non-blocking reads
        // TODO: use epoll to wake up when data is available
        // https://github.com/emberian/evdev/blob/main/examples/evtest_nonblocking.rs
        let raw_fd = device.as_raw_fd();
        nix::fcntl::fcntl(raw_fd, FcntlArg::F_SETFL(OFlag::O_NONBLOCK))?;

        Ok(device)
    }

    /// Translate the given native event into an evdev event
    fn translate_event(&self, event: NativeEvent) -> Vec<InputEvent> {
        EvdevEvent::from_native_event(event, self.axis_map.clone())
            .into_iter()
            .map(|event| event.as_input_event())
            .collect()
    }
}

impl TargetInputDevice for XBox360Controller {
    fn write_event(&mut self, event: NativeEvent) -> Result<(), InputError> {
        log::trace!("Received event: {event:?}");

        // Check for QuickAccess, create chord for event.
        let cap = event.as_capability();
        if cap == Capability::Gamepad(Gamepad::Button(GamepadButton::QuickAccess)) {
            let pressed = event.pressed();
            let guide = NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::Guide)),
                event.get_value(),
            );
            let south = NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::South)),
                event.get_value(),
            );

            let (guide, south) = if pressed {
                let guide = ScheduledNativeEvent::new(guide, Duration::from_millis(0));
                let south = ScheduledNativeEvent::new(south, Duration::from_millis(160));
                (guide, south)
            } else {
                let guide = ScheduledNativeEvent::new(guide, Duration::from_millis(240));
                let south = ScheduledNativeEvent::new(south, Duration::from_millis(160));
                (guide, south)
            };

            self.queued_events.push(guide);
            self.queued_events.push(south);
            return Ok(());
        }

        let evdev_events = self.translate_event(event);
        self.device.emit(evdev_events.as_slice())?;
        Ok(())
    }

    fn get_capabilities(&self) -> Result<Vec<Capability>, InputError> {
        Ok(vec![
            Capability::Gamepad(Gamepad::Axis(GamepadAxis::LeftStick)),
            Capability::Gamepad(Gamepad::Axis(GamepadAxis::RightStick)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::DPadDown)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::DPadLeft)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::DPadRight)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::DPadUp)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::East)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::Guide)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::LeftBumper)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::LeftStick)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::LeftTrigger)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::North)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::QuickAccess)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::RightBumper)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::RightStick)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::RightTrigger)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::Select)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::South)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::Start)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::West)),
            Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::LeftTrigger)),
            Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::RightTrigger)),
        ])
    }

    /// Returns any events in the queue up to the [TargetDriver]
    fn scheduled_events(&mut self) -> Option<Vec<ScheduledNativeEvent>> {
        if self.queued_events.is_empty() {
            return None;
        }
        Some(self.queued_events.drain(..).collect())
    }
}

impl TargetOutputDevice for XBox360Controller {
    /// Process force feedback events from the device
    fn poll(
        &mut self,
        composite_device: &Option<CompositeDeviceClient>,
    ) -> Result<Vec<OutputEvent>, OutputError> {
        // Fetch any force feedback events from the device
        let events: Vec<InputEvent> = match self.device.fetch_events() {
            Ok(events) => events.collect(),
            Err(e) => match e.kind() {
                std::io::ErrorKind::WouldBlock => vec![],
                _ => {
                    return Err(e.to_string().into());
                }
            },
        };

        const STOPPED: i32 = FFStatusCode::FF_STATUS_STOPPED.0 as i32;
        const PLAYING: i32 = FFStatusCode::FF_STATUS_PLAYING.0 as i32;

        // Process the events
        let mut output_events = vec![];
        for event in events {
            match event.destructure() {
                EventSummary::UInput(event, UInputCode::UI_FF_UPLOAD, ..) => {
                    log::debug!("Got FF upload event");
                    // Claim ownership of the FF upload and convert it to a FF_UPLOAD
                    // event
                    let mut event = self.device.process_ff_upload(event)?;
                    let effect_id = event.effect_id();

                    log::debug!("Upload effect: {:?} with id {}", event.effect(), effect_id);
                    let Some(composite_device) = composite_device else {
                        log::debug!("No composite device to upload effect to!");
                        event.set_retval(-1);
                        continue;
                    };

                    // Send the effect data to be uploaded to the device and wait
                    // for an effect ID to be generated.
                    let (tx, rx) = std::sync::mpsc::channel::<Option<i16>>();
                    let upload = OutputEvent::Uinput(UinputOutputEvent::FFUpload(
                        effect_id,
                        event.effect(),
                        tx,
                    ));
                    if let Err(e) = composite_device.blocking_process_output_event(upload) {
                        event.set_retval(-1);
                        return Err(e.to_string().into());
                    }
                    let effect_id = match rx.recv_timeout(Duration::from_secs(1)) {
                        Ok(id) => id,
                        Err(e) => {
                            event.set_retval(-1);
                            log::error!("Failed to receive FF upload response: {e:?}");
                            continue;
                        }
                    };

                    // Set the effect ID for the FF effect
                    if let Some(id) = effect_id {
                        event.set_effect_id(id);
                        event.set_retval(0);
                    } else {
                        log::warn!("Failed to get effect ID to upload FF effect");
                        event.set_retval(-1);
                    }
                }
                EventSummary::UInput(event, UInputCode::UI_FF_ERASE, ..) => {
                    log::debug!("Got FF erase event");
                    // Claim ownership of the FF erase event and convert it to a FF_ERASE
                    // event.
                    let event = self.device.process_ff_erase(event)?;
                    log::debug!("Erase effect: {:?}", event.effect_id());

                    let erase = OutputEvent::Uinput(UinputOutputEvent::FFErase(event.effect_id()));
                    output_events.push(erase);
                }
                EventSummary::ForceFeedback(.., effect_id, STOPPED) => {
                    log::debug!("Stopped effect ID: {}", effect_id.0);
                    log::debug!("Stopping event: {:?}", event);
                    output_events.push(OutputEvent::Evdev(event));
                }
                EventSummary::ForceFeedback(.., effect_id, PLAYING) => {
                    log::debug!("Playing effect ID: {}", effect_id.0);
                    log::debug!("Playing event: {:?}", event);
                    output_events.push(OutputEvent::Evdev(event));
                }
                _ => {
                    log::debug!("Unhandled event: {:?}", event);
                }
            }
        }

        Ok(output_events)
    }

    fn get_output_capabilities(&self) -> Result<Vec<OutputCapability>, OutputError> {
        Ok(vec![OutputCapability::ForceFeedback])
    }
}
