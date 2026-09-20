use super::dualsense::{
    generate_mac, BusType, DualSenseHardware, ModelType, DS5_BASE_MAC, DS5_EDGE_BASE_MAC,
};

#[test]
fn fallback_increments_within_model_only() {
    let first = generate_mac(ModelType::Normal, None);
    let edge = generate_mac(ModelType::Edge, None);
    let second = generate_mac(ModelType::Normal, None);

    assert_eq!(&first[..3], &DS5_BASE_MAC);
    assert_eq!(&second[..3], &DS5_BASE_MAC);
    assert_eq!(&edge[..3], &DS5_EDGE_BASE_MAC);

    let first_index = u16::from_be_bytes([first[4], first[5]]);
    let second_index = u16::from_be_bytes([second[4], second[5]]);
    assert_eq!(second_index, first_index + 1);
}

#[test]
fn default_uses_normal_model_and_prefix() {
    let hw = DualSenseHardware::default();
    assert_eq!(hw.model, ModelType::Normal);
    assert_eq!(hw.bus_type, BusType::Usb);
    assert_eq!(&hw.mac_addr[..3], &DS5_BASE_MAC);
}

#[test]
fn persistent_id_gives_stable_address() {
    let id = "wch.cn_Legion_Go_S_BC4F5A06ABCD";
    let first = generate_mac(ModelType::Normal, Some(id));
    let second = generate_mac(ModelType::Normal, Some(id));
    assert_eq!(first, second);
    assert_eq!(&first[..3], &DS5_BASE_MAC);
}

#[test]
fn different_persistent_ids_differ() {
    let a = generate_mac(ModelType::Normal, Some("controller-a"));
    let b = generate_mac(ModelType::Normal, Some("controller-b"));
    assert_ne!(a, b);
}

#[test]
fn same_persistent_id_differs_by_model() {
    let id = "same-controller-id";
    let normal = generate_mac(ModelType::Normal, Some(id));
    let edge = generate_mac(ModelType::Edge, Some(id));
    assert_eq!(&normal[..3], &DS5_BASE_MAC);
    assert_eq!(&edge[..3], &DS5_EDGE_BASE_MAC);
    assert_ne!(normal, edge);
}

#[test]
fn empty_persistent_id_falls_back_to_counter() {
    let before = generate_mac(ModelType::Normal, None);
    let with_empty = generate_mac(ModelType::Normal, Some(""));
    let after = generate_mac(ModelType::Normal, None);

    assert_eq!(&with_empty[..3], &DS5_BASE_MAC);
    let before_index = u16::from_be_bytes([before[4], before[5]]);
    let empty_index = u16::from_be_bytes([with_empty[4], with_empty[5]]);
    let after_index = u16::from_be_bytes([after[4], after[5]]);
    assert_eq!(empty_index, before_index + 1);
    assert_eq!(after_index, empty_index + 1);
}
