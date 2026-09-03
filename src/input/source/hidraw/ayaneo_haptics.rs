use std::{collections::HashMap, error::Error, ffi::CString, fmt::Debug};

use evdev::{FFEffectData, FFEffectKind};
use hidapi::HidDevice;
use packed_struct::prelude::*;

use crate::{
    input::{
        capability::Capability,
        event::native::NativeEvent,
        output_capability::OutputCapability,
        output_event::OutputEvent,
        source::{InputError, OutputError, SourceInputDevice, SourceOutputDevice},
    },
    udev::device::UdevDevice,
};

/// USB identifiers used by the AYANEO DirectInput controller.
pub const VID: u16 = 0x4001;
pub const PID: u16 = 0x0428;

#[derive(PackedStruct, Debug, Default, Copy, Clone, PartialEq)]
#[packed_struct(bit_numbering = "msb0", size_bytes = "8")]
struct AyaneoRumbleReport {
    #[packed_field(bytes = "0")]
    reserved_0: u8,
    #[packed_field(bytes = "1")]
    reserved_1: u8,
    #[packed_field(bytes = "2")]
    reserved_2: u8,
    #[packed_field(bytes = "3")]
    reserved_3: u8,
    #[packed_field(bytes = "4")]
    left_motor: u8,
    #[packed_field(bytes = "5")]
    right_motor: u8,
    #[packed_field(bytes = "6")]
    reserved_6: u8,
    #[packed_field(bytes = "7")]
    reserved_7: u8,
}

/// Output-only source for the rumble motors exposed by AYANEO DirectInput mode.
///
/// The controller accepts an eight-byte HID output report. Zero-based byte 4
/// drives the left motor and byte 5 drives the right motor; this layout was
/// verified against the physical motors on an AYANEO DirectInput controller.
pub struct AyaneoHaptics {
    device: HidDevice,
    ff_evdev_effects: HashMap<i16, FFEffectData>,
}

impl AyaneoHaptics {
    pub fn new(device_info: UdevDevice) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let path = device_info.devnode();
        let vid = device_info.id_vendor();
        let pid = device_info.id_product();
        if vid != VID || pid != PID {
            return Err(format!(
                "Device '{path}' is not an AYANEO DirectInput controller ({vid:04x}:{pid:04x})"
            )
            .into());
        }

        let path = CString::new(path)?;
        let api = hidapi::HidApi::new()?;
        let device = api.open_path(&path)?;

        Ok(Self {
            device,
            ff_evdev_effects: HashMap::new(),
        })
    }

    fn next_ff_effect_id(&self) -> i16 {
        const MAX_EFFECT_ID: i16 = 2096;
        (0..=MAX_EFFECT_ID)
            .find(|id| !self.ff_evdev_effects.contains_key(id))
            .unwrap_or(-1)
    }

    fn build_rumble_report(left_magnitude: u16, right_magnitude: u16) -> AyaneoRumbleReport {
        AyaneoRumbleReport {
            left_motor: (left_magnitude >> 8) as u8,
            right_motor: (right_magnitude >> 8) as u8,
            ..Default::default()
        }
    }

    fn rumble(
        &self,
        left_magnitude: u16,
        right_magnitude: u16,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let report = Self::build_rumble_report(left_magnitude, right_magnitude).pack()?;
        let written = self.device.write(&report)?;
        if written != report.len() {
            return Err(format!(
                "Short AYANEO haptics write: wrote {written} of {} bytes",
                report.len()
            )
            .into());
        }
        Ok(())
    }

    fn process_evdev_ff(
        &mut self,
        input_event: evdev::InputEvent,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let (code, value) =
            if let evdev::EventSummary::ForceFeedback(_, code, value) = input_event.destructure() {
                (code, value)
            } else {
                log::debug!("Unhandled AYANEO haptics evdev event: {input_event:?}");
                return Ok(());
            };

        if value == 0 {
            return self.rumble(0, 0);
        }

        let effect_id = code.0 as i16;
        let Some(effect) = self.ff_evdev_effects.get(&effect_id) else {
            log::warn!("No AYANEO haptics effect id found: {}", code.0);
            return Ok(());
        };

        if let FFEffectKind::Rumble {
            strong_magnitude,
            weak_magnitude,
        } = effect.kind
        {
            self.rumble(strong_magnitude, weak_magnitude)?;
        }

        Ok(())
    }
}

impl Debug for AyaneoHaptics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AyaneoHaptics").finish()
    }
}

impl Drop for AyaneoHaptics {
    fn drop(&mut self) {
        if let Err(err) = self.rumble(0, 0) {
            log::debug!("Failed to stop AYANEO haptics during shutdown: {err:?}");
        }
    }
}

impl SourceInputDevice for AyaneoHaptics {
    fn poll(&mut self) -> Result<Vec<NativeEvent>, InputError> {
        Ok(vec![])
    }

    fn get_capabilities(&self) -> Result<Vec<Capability>, InputError> {
        Ok(vec![])
    }
}

impl SourceOutputDevice for AyaneoHaptics {
    fn get_output_capabilities(&self) -> Result<Vec<OutputCapability>, OutputError> {
        Ok(vec![OutputCapability::ForceFeedback])
    }

    fn write_event(&mut self, event: OutputEvent) -> Result<(), OutputError> {
        match event {
            OutputEvent::Evdev(input_event) => Ok(self.process_evdev_ff(input_event)?),
            OutputEvent::DualSense(report) => {
                if report.use_rumble_not_haptics || report.enable_improved_rumble_emulation {
                    self.rumble(
                        (report.rumble_emulation_left as u16) << 8,
                        (report.rumble_emulation_right as u16) << 8,
                    )?;
                }
                Ok(())
            }
            OutputEvent::SteamDeckRumble(report) => Ok(self.rumble(
                report.left_speed.to_primitive(),
                report.right_speed.to_primitive(),
            )?),
            OutputEvent::GenericRumble {
                weak_magnitude,
                strong_magnitude,
            } => Ok(self.rumble(strong_magnitude, weak_magnitude)?),
            OutputEvent::Uinput(_) | OutputEvent::SteamDeckHaptics(_) => Ok(()),
        }
    }

    fn upload_effect(&mut self, effect: FFEffectData) -> Result<i16, OutputError> {
        let id = self.next_ff_effect_id();
        if id == -1 {
            return Err("Maximum AYANEO haptics effects uploaded".into());
        }
        self.ff_evdev_effects.insert(id, effect);
        Ok(id)
    }

    fn update_effect(&mut self, effect_id: i16, effect: FFEffectData) -> Result<(), OutputError> {
        log::debug!("Updating AYANEO haptics FF effect data with id {effect_id}");
        self.ff_evdev_effects.insert(effect_id, effect);
        Ok(())
    }

    fn erase_effect(&mut self, effect_id: i16) -> Result<(), OutputError> {
        self.ff_evdev_effects.remove(&effect_id);
        Ok(())
    }

    fn stop(&mut self) -> Result<(), OutputError> {
        self.rumble(0, 0)?;
        self.ff_evdev_effects.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use packed_struct::PackedStruct;

    use super::AyaneoHaptics;

    #[test]
    fn builds_left_and_right_motor_report() {
        assert_eq!(
            AyaneoHaptics::build_rumble_report(0x8000, 0x6000)
                .pack()
                .unwrap(),
            [0x00, 0x00, 0x00, 0x00, 0x80, 0x60, 0x00, 0x00]
        );
    }

    #[test]
    fn uses_high_byte_of_each_evdev_magnitude() {
        assert_eq!(
            AyaneoHaptics::build_rumble_report(0x12ff, 0xab01)
                .pack()
                .unwrap(),
            [0x00, 0x00, 0x00, 0x00, 0x12, 0xab, 0x00, 0x00]
        );
    }

    #[test]
    fn builds_stop_report() {
        assert_eq!(
            AyaneoHaptics::build_rumble_report(0, 0).pack().unwrap(),
            [0x00; 8]
        );
    }
}
