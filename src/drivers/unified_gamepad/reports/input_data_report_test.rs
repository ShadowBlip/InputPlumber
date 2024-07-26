use std::error::Error;

use crate::drivers::unified_gamepad::{
    capability::InputCapability,
    reports::{
        input_capability_report::{InputCapabilityInfo, InputCapabilityReport},
        input_data_report::{BoolUpdate, InputDataReport, StateUpdate, ValueUpdate},
        ValueType,
    },
    value::Value,
};

#[tokio::test]
async fn test_update() -> Result<(), Box<dyn Error>> {
    // Create a capability report
    let mut capability_report = InputCapabilityReport::default();
    capability_report
        .add_capability(InputCapabilityInfo::new(
            InputCapability::GamepadButtonStart,
            ValueType::Bool,
        ))
        .expect("should add capability");
    capability_report
        .add_capability(InputCapabilityInfo::new(
            InputCapability::GamepadButtonSelect,
            ValueType::Bool,
        ))
        .expect("should add capability");

    // Create the input report
    let mut input_report = InputDataReport::default();

    // Test updates
    let update = StateUpdate {
        capability: InputCapability::GamepadButtonStart,
        value: ValueUpdate::Bool(BoolUpdate { value: true }),
    };
    input_report
        .update(&capability_report, update)
        .expect("should update the input report");
    println!("Input Report: {input_report}");
    assert_eq!(
        input_report.data[0], 1,
        "should have set the data in the first byte"
    );

    let update = StateUpdate {
        capability: InputCapability::GamepadButtonStart,
        value: ValueUpdate::Bool(BoolUpdate { value: false }),
    };
    input_report
        .update(&capability_report, update)
        .expect("should update the input report");
    println!("Input Report: {input_report}");
    assert_eq!(
        input_report.data[0], 0,
        "should have unset the data in the first byte"
    );

    Ok(())
}

#[tokio::test]
async fn test_decode() -> Result<(), Box<dyn Error>> {
    // Create a capability report
    let mut capability_report = InputCapabilityReport::default();
    capability_report
        .add_capability(InputCapabilityInfo::new(
            InputCapability::GamepadButtonStart,
            ValueType::Bool,
        ))
        .expect("should add capability");
    capability_report
        .add_capability(InputCapabilityInfo::new(
            InputCapability::GamepadButtonSelect,
            ValueType::Bool,
        ))
        .expect("should add capability");

    // Create the input report
    let mut input_report = InputDataReport::default();

    // Update the state
    let update = StateUpdate {
        capability: InputCapability::GamepadButtonStart,
        value: ValueUpdate::Bool(BoolUpdate { value: true }),
    };
    input_report
        .update(&capability_report, update)
        .expect("should update the input report");
    println!("Input Report: {input_report}");
    assert_eq!(
        input_report.data[0], 1,
        "should have set the data in the first byte"
    );

    // Test decoding the report using the capability report
    let values = capability_report
        .decode_data_report(&input_report)
        .expect("should have decoded the input report");
    println!("Decoded values: {values:?}");

    let start_value = values.first().expect("should have at least one value");
    let Value::Bool(value) = start_value else {
        panic!("start value should be a boolean");
    };
    assert!(value.value, "decoded value should be true");

    Ok(())
}
