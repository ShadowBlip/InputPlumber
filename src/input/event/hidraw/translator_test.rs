use std::error::Error;

use packed_struct::PackedStructSlice;

use crate::{
    config::capability_map::{
        hidraw::{Endianness, HidrawConfig, ValueType},
        AxisCapability, CapabilityConfig, CapabilityMapConfig, CapabilityMapConfigV2,
        CapabilityMapping, GamepadCapability, SourceMapping, TriggerCapability,
    },
    drivers::dualsense::hid_report::InputState,
    input::{
        capability::{Capability, Gamepad, GamepadAxis, GamepadButton, GamepadTrigger},
        event::{
            hidraw::translator::HidrawEventTranslator, native::NativeEvent, value::InputValue,
        },
    },
};

#[tokio::test]
async fn test_ds_translation() -> Result<(), Box<dyn Error>> {
    let capability_map_str = r#"
version: 2
kind: CapabilityMap
name: GPD HID Type 1
id: gpd_v2_hid1
mapping:
  - name: Cross
    source_events:
      - hidraw:
          value_type: bool
          byte_start: 7
          bit_offset: 5
    target_event:
      gamepad:
        button: South
"#;
    let capability_map = CapabilityMapConfig::from_yaml(capability_map_str.into()).unwrap();
    let CapabilityMapConfig::V2(capability_map) = capability_map else {
        panic!("A v2 capability map was not used");
    };

    let mut translator = HidrawEventTranslator::new(&capability_map);
    let mut report = InputState::default();

    let report_bytes = report.pack_to_vec().unwrap();
    let events = translator.translate(&report_bytes);
    assert_eq!(events.len(), 0, "No events should be emitted");

    // Press the X button
    report.cross = true;
    let report_bytes = report.pack_to_vec().unwrap();
    let events = translator.translate(&report_bytes);
    assert_eq!(events.len(), 1, "A button down event should be emitted");

    Ok(())
}

/// One mapping of every single-value type in a 16-byte report. Byte 0 is the
/// state key (report id) and never decoded.
fn single_value_map() -> CapabilityMapConfigV2 {
    let mut map = new_map();
    add_mapping(&mut map, "u8", button_capability("DPadUp"), {
        let value_type = ValueType::UInt8;
        HidrawConfig {
            value_type,
            byte_start: 1,
            ..Default::default()
        }
    });
    add_mapping(&mut map, "i8", button_capability("DPadDown"), {
        let value_type = ValueType::Int8;
        HidrawConfig {
            value_type,
            byte_start: 2,
            ..Default::default()
        }
    });
    add_mapping(&mut map, "u16", button_capability("DPadLeft"), {
        let value_type = ValueType::UInt16;
        HidrawConfig {
            value_type,
            byte_start: 3,
            ..Default::default()
        }
    });
    add_mapping(&mut map, "i16", button_capability("DPadRight"), {
        let value_type = ValueType::Int16;
        HidrawConfig {
            value_type,
            byte_start: 5,
            ..Default::default()
        }
    });
    add_mapping(&mut map, "u32", button_capability("East"), {
        let value_type = ValueType::UInt32;
        HidrawConfig {
            value_type,
            byte_start: 7,
            ..Default::default()
        }
    });
    add_mapping(&mut map, "i32", button_capability("West"), {
        let value_type = ValueType::Int32;
        HidrawConfig {
            value_type,
            byte_start: 11,
            ..Default::default()
        }
    });
    map
}

const SINGLE_VALUE_SEED: [u8; 16] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

#[test]
fn test_single_value_decoding() {
    let map = single_value_map();
    // u8 = 0xFF -> 1.0; i8 = 0x80 (-128) -> -1.0; u16 = 0xFFFF (LE) -> 1.0;
    // i16 = 0x7FFF (32767, LE) -> 1.0; u32 = 0xFFFFFFFF (LE) -> 1.0;
    // i32 = i32::MIN (LE) -> -1.0
    let mut report = SINGLE_VALUE_SEED;
    report[1] = 0xFF;
    report[2] = 0x80;
    report[3] = 0xFF;
    report[4] = 0xFF;
    report[5] = 0xFF;
    report[6] = 0x7F;
    report[7] = 0xFF;
    report[8] = 0xFF;
    report[9] = 0xFF;
    report[10] = 0xFF;
    report[11] = 0x00;
    report[12] = 0x00;
    report[13] = 0x00;
    report[14] = 0x80;
    let events = translate(&map, &SINGLE_VALUE_SEED, &report);

    assert_eq!(
        event_value(
            &events,
            &Capability::Gamepad(Gamepad::Button(GamepadButton::DPadUp)),
            0
        ),
        Some(InputValue::Float(1.0))
    );
    assert_eq!(
        event_value(
            &events,
            &Capability::Gamepad(Gamepad::Button(GamepadButton::DPadDown)),
            0
        ),
        Some(InputValue::Float(-1.0))
    );
    assert_eq!(
        event_value(
            &events,
            &Capability::Gamepad(Gamepad::Button(GamepadButton::DPadLeft)),
            0
        ),
        Some(InputValue::Float(1.0))
    );
    assert_eq!(
        event_value(
            &events,
            &Capability::Gamepad(Gamepad::Button(GamepadButton::DPadRight)),
            0
        ),
        Some(InputValue::Float(1.0))
    );
    assert_eq!(
        event_value(
            &events,
            &Capability::Gamepad(Gamepad::Button(GamepadButton::East)),
            0
        ),
        Some(InputValue::Float(1.0))
    );
    assert_eq!(
        event_value(
            &events,
            &Capability::Gamepad(Gamepad::Button(GamepadButton::West)),
            0
        ),
        Some(InputValue::Float(-1.0))
    );
}

#[test]
fn test_unsigned_midrange() {
    let map = single_value_map();
    let mut report = SINGLE_VALUE_SEED;
    report[1] = 0x80;
    let events = translate(&map, &SINGLE_VALUE_SEED, &report);

    assert_eq!(
        event_value(
            &events,
            &Capability::Gamepad(Gamepad::Button(GamepadButton::DPadUp)),
            0
        ),
        Some(InputValue::Float(128.0 / 255.0))
    );
    // DPadDown (i8) is still 0 in both reports -> no event
    assert!(events.iter().all(|event| event.as_capability()
        != Capability::Gamepad(Gamepad::Button(GamepadButton::DPadDown))));
}

#[test]
fn test_int8_vector2_decoding() {
    let mut map = new_map();
    add_mapping(&mut map, "axis", axis_capability("LeftStick"), {
        let value_type = ValueType::Int8Vector2;
        HidrawConfig {
            value_type,
            byte_start: 1,
            ..Default::default()
        }
    });
    // x = 127 at the top of the [-128, 127] range -> 1.0, y = -128 -> -1.0
    let events = translate(&map, &[0u8; 3], &[0, 127, 0x80]);
    assert_eq!(
        event_value(
            &events,
            &Capability::Gamepad(Gamepad::Axis(GamepadAxis::LeftStick)),
            0
        ),
        Some(InputValue::Vector2 {
            x: Some(1.0),
            y: Some(-1.0),
        })
    );
}

#[test]
fn test_uint16_vector2_decoding() {
    let mut map = new_map();
    add_mapping(&mut map, "axis", axis_capability("LeftStick"), {
        let value_type = ValueType::UInt16Vector2;
        HidrawConfig {
            value_type,
            byte_start: 1,
            ..Default::default()
        }
    });
    // LE: x = 0x0100 = 256 -> 256/65535, y = 0xFFFF -> 1.0
    let events = translate(&map, &[0u8; 5], &[0, 0x00, 0x01, 0xFF, 0xFF]);
    assert_eq!(
        event_value(
            &events,
            &Capability::Gamepad(Gamepad::Axis(GamepadAxis::LeftStick)),
            0
        ),
        Some(InputValue::Vector2 {
            x: Some(256.0 / 65535.0),
            y: Some(1.0),
        })
    );
}

#[test]
fn test_int16_vector3_decoding() {
    let mut map = new_map();
    add_mapping(&mut map, "axis", axis_capability("LeftStick"), {
        let value_type = ValueType::Int16Vector3;
        HidrawConfig {
            value_type,
            byte_start: 1,
            ..Default::default()
        }
    });
    // LE: x = i16::MIN -> -1.0, y = 32767 -> 1.0, z = 0 -> 0.5/32767.5
    // (signed normalization is midpoint-based, so 0 is slightly above -1.0..1.0
    // midpoint, not exactly 0.0)
    let events = translate(&map, &[0u8; 7], &[0, 0x00, 0x80, 0xFF, 0x7F, 0x00, 0x00]);
    assert_eq!(
        event_value(
            &events,
            &Capability::Gamepad(Gamepad::Axis(GamepadAxis::LeftStick)),
            0
        ),
        Some(InputValue::Vector3 {
            x: Some(-1.0),
            y: Some(1.0),
            z: Some(0.5 / 32767.5),
        })
    );
}

#[test]
fn test_trigger_value() {
    let mut map = new_map();
    add_mapping(&mut map, "ltrigger", trigger_capability("LeftTrigger"), {
        let value_type = ValueType::UInt16;
        HidrawConfig {
            value_type,
            byte_start: 1,
            ..Default::default()
        }
    });
    let events = translate(&map, &[0u8; 3], &[0, 0xFF, 0xFF]);
    assert_eq!(
        event_value(
            &events,
            &Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::LeftTrigger)),
            0
        ),
        Some(InputValue::Float(1.0))
    );
}

#[test]
fn test_msb_endianness() {
    let mut map = new_map();
    let mut config = {
        let value_type = ValueType::UInt16;
        HidrawConfig {
            value_type,
            byte_start: 1,
            ..Default::default()
        }
    };
    config.endian = Some(Endianness::Msb);
    add_mapping(&mut map, "axis", trigger_capability("LeftTrigger"), config);

    // MSB: [0xFF, 0x00] -> 0xFF00 = 65280 -> 65280/65535
    let events = translate(&map, &[0u8; 3], &[0, 0xFF, 0x00]);
    assert_eq!(
        event_value(
            &events,
            &Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::LeftTrigger)),
            0
        ),
        Some(InputValue::Float(0xFF00 as f64 / 65535.0))
    );
}

#[test]
fn test_unsigned_custom_max() {
    let mut map = new_map();
    let mut config = {
        let value_type = ValueType::UInt8;
        HidrawConfig {
            value_type,
            byte_start: 1,
            ..Default::default()
        }
    };
    config.max_value = Some(200);
    add_mapping(&mut map, "u8", button_capability("East"), config);

    // 100 in an explicit [0, 200] range normalizes to 0.5
    let events = translate(&map, &[0u8; 2], &[0, 100]);
    assert_eq!(
        event_value(
            &events,
            &Capability::Gamepad(Gamepad::Button(GamepadButton::East)),
            0
        ),
        Some(InputValue::Float(0.5))
    );
}

#[test]
fn test_signed_custom_min_max() {
    let mut map = new_map();
    let mut config = {
        let value_type = ValueType::Int16;
        HidrawConfig {
            value_type,
            byte_start: 1,
            ..Default::default()
        }
    };
    config.min_value = Some(-100);
    config.max_value = Some(100);
    add_mapping(&mut map, "axis", axis_capability("LeftStick"), config);

    // -28 (LE 0xFFE4, bytes [0xE4, 0xFF]) in a [-100, 100] range normalizes to -0.28
    let events = translate(&map, &[0u8; 3], &[0, 0xE4, 0xFF]);
    assert_eq!(
        event_value(
            &events,
            &Capability::Gamepad(Gamepad::Axis(GamepadAxis::LeftStick)),
            0
        ),
        Some(InputValue::Float(-28.0 / 100.0))
    );
}

#[test]
fn test_bool_with_bit_offset() {
    let mut map = new_map();
    let mut config = {
        let value_type = ValueType::Bool;
        HidrawConfig {
            value_type,
            byte_start: 1,
            ..Default::default()
        }
    };
    config.bit_offset = Some(2);
    add_mapping(&mut map, "u8", button_capability("East"), config);

    // bit 2 of 0x04 is set -> true
    let events = translate(&map, &[0u8, 0x00], &[0, 0x04]);
    assert_eq!(
        event_value(
            &events,
            &Capability::Gamepad(Gamepad::Button(GamepadButton::East)),
            0
        ),
        Some(InputValue::Bool(true))
    );

    // clearing bit 2 -> back to false
    let events = translate(&map, &[0u8, 0x04], &[0, 0x00]);
    assert_eq!(
        event_value(
            &events,
            &Capability::Gamepad(Gamepad::Button(GamepadButton::East)),
            0
        ),
        Some(InputValue::Bool(false))
    );
}

#[test]
fn test_bool_without_bit_offset() {
    let mut map = new_map();
    let config = {
        let value_type = ValueType::Bool;
        HidrawConfig {
            value_type,
            byte_start: 1,
            ..Default::default()
        }
    };
    add_mapping(&mut map, "u8", button_capability("East"), config);

    // nonzero byte -> true
    let events = translate(&map, &[0u8, 0x00], &[0, 0x01]);
    assert_eq!(
        event_value(
            &events,
            &Capability::Gamepad(Gamepad::Button(GamepadButton::East)),
            0
        ),
        Some(InputValue::Bool(true))
    );
}

/// A report whose first byte doesn't match the mapping's report_id is skipped
/// and must not update the per-report-id last state.
#[test]
fn test_report_id_mismatch_is_skipped() {
    let mut map = new_map();
    let mut config = {
        let value_type = ValueType::UInt8;
        HidrawConfig {
            value_type,
            byte_start: 1,
            ..Default::default()
        }
    };
    config.report_id = Some(1);
    add_mapping(&mut map, "u8", button_capability("East"), config);

    let mut translator = HidrawEventTranslator::new(&map);
    // Seed the report-id-1 state with the value at byte 1 = 0
    assert_eq!(translator.translate(&[1u8, 0x00]).len(), 0);
    // A report with a mismatching report id is ignored entirely and does not
    // disturb the report-id-1 state
    let events = translator.translate(&[2u8, 0xFF]);
    assert_eq!(events.len(), 0);
    // The report-id-1 state is unchanged, so a change to it still works
    let events = translator.translate(&[1u8, 0xFF]);
    assert_eq!(
        event_value(
            &events,
            &Capability::Gamepad(Gamepad::Button(GamepadButton::East)),
            0
        ),
        Some(InputValue::Float(1.0))
    );
}

/// Reports with distinct report ids must keep independent last-states: changing
/// one report id's contents must only emit events for that id's mappings.
#[test]
fn test_separate_state_per_report_id() {
    let mut map = new_map();
    let mut config_a = {
        let value_type = ValueType::UInt8;
        HidrawConfig {
            value_type,
            byte_start: 1,
            ..Default::default()
        }
    };
    config_a.report_id = Some(1);
    add_mapping(&mut map, "a", button_capability("East"), config_a);
    let mut config_b = {
        let value_type = ValueType::UInt8;
        HidrawConfig {
            value_type,
            byte_start: 1,
            ..Default::default()
        }
    };
    config_b.report_id = Some(2);
    add_mapping(&mut map, "b", button_capability("West"), config_b);

    let mut translator = HidrawEventTranslator::new(&map);
    assert_eq!(translator.translate(&[1u8, 0xFF]).len(), 0);
    assert_eq!(translator.translate(&[2u8, 0xFF]).len(), 0);

    // Changing report id 2's value emits West only
    let events = translator.translate(&[2u8, 0x00]);
    assert_eq!(
        event_value(
            &events,
            &Capability::Gamepad(Gamepad::Button(GamepadButton::West)),
            0
        ),
        Some(InputValue::Float(0.0))
    );
    assert!(events
        .iter()
        .all(|event| event.as_capability()
            != Capability::Gamepad(Gamepad::Button(GamepadButton::East))));

    // Changing report id 1's value emits East only
    let events = translator.translate(&[1u8, 0x00]);
    assert_eq!(
        event_value(
            &events,
            &Capability::Gamepad(Gamepad::Button(GamepadButton::East)),
            0
        ),
        Some(InputValue::Float(0.0))
    );
    assert!(events
        .iter()
        .all(|event| event.as_capability()
            != Capability::Gamepad(Gamepad::Button(GamepadButton::West))));
}

/// Only capabilities whose decoded value actually changed between reports
/// should emit events.
#[test]
fn test_no_event_on_unchanged_capability() {
    let map = single_value_map();
    // Seed and report differ only in byte 1 (u8 -> DPadUp); byte 2 (i8 ->
    // DPadDown) is 1 in both reports, so it must not emit.
    let seed = [0u8, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    let mut report = seed;
    report[1] = 42;
    let events = translate(&map, &seed, &report);

    assert_eq!(events.len(), 1);
    assert_eq!(
        event_value(
            &events,
            &Capability::Gamepad(Gamepad::Button(GamepadButton::DPadUp)),
            0
        ),
        Some(InputValue::Float(42.0 / 255.0))
    );
}

/// A `byte_start` past the end of the report must not panic; the mapping is
/// skipped with a logged warning instead.
#[test]
fn test_byte_start_exceeds_report() {
    let mut map = new_map();
    let config = {
        let value_type = ValueType::UInt8;
        HidrawConfig {
            value_type,
            byte_start: 100,
            ..Default::default()
        }
    };
    add_mapping(&mut map, "u8", button_capability("East"), config);

    let mut translator = HidrawEventTranslator::new(&map);
    assert_eq!(translator.translate(&[0u8; 4]).len(), 0);
    let events = translator.translate(&[1, 2, 3, 4]);
    assert_eq!(events.len(), 0);
}

/// Seed the translator's per-report-id state, then translate a different report.
/// Events are only emitted on state change, so the seed report's first byte
/// (the report id used as the state key) must match the test report's.
fn translate(map: &CapabilityMapConfigV2, seed: &[u8], report: &[u8]) -> Vec<NativeEvent> {
    assert_eq!(seed[0], report[0]);
    let mut translator = HidrawEventTranslator::new(map);
    assert_eq!(
        translator.translate(seed).len(),
        0,
        "the seed report should never emit events"
    );
    translator.translate(report)
}

fn button_capability(button: &str) -> CapabilityConfig {
    CapabilityConfig {
        gamepad: Some(GamepadCapability {
            button: Some(button.to_string()),
            ..Default::default()
        }),
        ..Default::default()
    }
}

fn axis_capability(name: &str) -> CapabilityConfig {
    CapabilityConfig {
        gamepad: Some(GamepadCapability {
            axis: Some(AxisCapability {
                name: name.to_string(),
                ..Default::default()
            }),
            ..Default::default()
        }),
        ..Default::default()
    }
}

fn trigger_capability(name: &str) -> CapabilityConfig {
    CapabilityConfig {
        gamepad: Some(GamepadCapability {
            trigger: Some(TriggerCapability {
                name: name.to_string(),
                ..Default::default()
            }),
            ..Default::default()
        }),
        ..Default::default()
    }
}

fn add_mapping(
    map: &mut CapabilityMapConfigV2,
    name: &str,
    target: CapabilityConfig,
    hidraw: HidrawConfig,
) {
    map.mapping.push(CapabilityMapping {
        name: name.to_string(),
        mapping_type: None,
        source_events: vec![SourceMapping {
            evdev: None,
            hidraw: Some(hidraw),
            capability: None,
        }],
        target_event: target,
    });
}

fn new_map() -> CapabilityMapConfigV2 {
    CapabilityMapConfigV2 {
        version: 2,
        kind: "generic".to_string(),
        name: "test".to_string(),
        id: "test".to_string(),
        mapping: vec![],
    }
}

/// Get the value of the `index`-th event (0-based) for the given capability.
fn event_value(
    events: &[NativeEvent],
    capability: &Capability,
    index: usize,
) -> Option<InputValue> {
    events
        .iter()
        .filter(|event| &event.as_capability() == capability)
        .nth(index)
        .map(|event| event.get_value())
}
