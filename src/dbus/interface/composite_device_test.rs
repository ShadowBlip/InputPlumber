use zbus::zvariant::{Structure, Value};

use crate::input::event::value::InputValue;

use super::composite_device::touch_value_from_structure;

fn structure(value: Value<'static>) -> Structure<'static> {
    let Value::Structure(structure) = value else {
        panic!("value should be a structure");
    };
    structure
}

#[test]
fn test_touch_value_from_structure() {
    let value = structure(Value::from((1u32, true, 0.5f64, 0.25f64, 0.75f64)));
    let touch = touch_value_from_structure(value).expect("should parse touch value");
    assert!(matches!(
        touch,
        InputValue::Touch {
            index: 1,
            is_touching: true,
            pressure: Some(0.5),
            x: Some(0.25),
            y: Some(0.75),
        }
    ));
}

#[test]
fn test_touch_value_from_structure_index_out_of_range() {
    let value = structure(Value::from((256u32, true, 1.0f64, 0.0f64, 0.0f64)));
    assert!(touch_value_from_structure(value).is_err());
}

#[test]
fn test_touch_value_from_structure_wrong_signature() {
    let value = structure(Value::from((0u32, true, 1.0f64, 0.0f64)));
    assert!(touch_value_from_structure(value).is_err());
}

#[test]
fn test_touch_value_from_structure_extra_field() {
    let value = structure(Value::from((0u32, true, 1.0f64, 0.0f64, 0.0f64, 0.0f64)));
    assert!(touch_value_from_structure(value).is_err());
}

#[test]
fn test_touch_value_from_structure_wrong_field_type() {
    let value = structure(Value::from((0u32, 1.0f64, 1.0f64, 0.0f64, 0.0f64)));
    assert!(touch_value_from_structure(value).is_err());
}
