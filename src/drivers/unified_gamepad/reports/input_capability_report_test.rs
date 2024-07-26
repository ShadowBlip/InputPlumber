use std::error::Error;

use crate::drivers::unified_gamepad::{
    capability::InputCapability,
    reports::{
        input_capability_report::{InputCapabilityInfo, InputCapabilityReport},
        ValueType,
    },
};

#[tokio::test]
async fn test_packing() -> Result<(), Box<dyn Error>> {
    let mut report = InputCapabilityReport::default();
    report
        .add_capability(InputCapabilityInfo::new(
            InputCapability::GamepadButtonStart,
            ValueType::Bool,
        ))
        .expect("should add capability");
    report
        .add_capability(InputCapabilityInfo::new(
            InputCapability::GamepadButtonSelect,
            ValueType::Bool,
        ))
        .expect("should add capability");
    report
        .get_capability(InputCapability::GamepadButtonSelect)
        .expect("should have added the capability");

    // Pack the report
    let bytes = report.pack_to_vec().expect("should pack to bytes");
    println!("Got bytes: {bytes:?}");

    // Unpack the report
    let unpacked_report =
        InputCapabilityReport::unpack(bytes.as_slice()).expect("should have unpacked");

    println!("Got unpacked report: {unpacked_report}");

    assert_eq!(
        format!("{report}"),
        format!("{unpacked_report}"),
        "unpacked report should match original"
    );

    Ok(())
}
