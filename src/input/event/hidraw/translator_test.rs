use std::error::Error;

use packed_struct::PackedStructSlice;

use crate::{
    config::capability_map::CapabilityMapConfig, drivers::dualsense::hid_report::InputState,
    input::event::hidraw::translator::HidrawEventTranslator,
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
