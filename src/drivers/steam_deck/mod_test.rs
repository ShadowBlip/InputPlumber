use super::{generate_board_serial, generate_serial};

#[test]
fn stable_for_same_persistent_id() {
    let id = "wch.cn_Legion_Go_S_BC4F5A06ABCD";
    let first = generate_serial(Some(id));
    let second = generate_serial(Some(id));
    assert_eq!(first, second);
    assert_eq!(first.len(), 10);
    assert!(first.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_lowercase()));
}

#[test]
fn differs_for_different_persistent_ids() {
    let a = generate_serial(Some("controller-a"));
    let b = generate_serial(Some("controller-b"));
    assert_ne!(a, b);
}

#[test]
fn falls_back_to_fixed_value_without_persistent_id() {
    let without = generate_serial(None);
    let with_empty = generate_serial(Some(""));
    assert_eq!(without, with_empty);
    assert_eq!(without, "0001AE1C0B");
}

#[test]
fn board_serial_is_valid() {
    let id = "wch.cn_Legion_Go_S_BC4F5A06ABCD";
    let first = generate_board_serial(Some(id));
    let second = generate_board_serial(Some(id));
    let chars: Vec<char> = first.chars().collect();
    assert_eq!(first, second);
    assert_eq!(first.len(), 10);
    assert_eq!(chars[0], 'M');
    assert!(['B', 'C', 'D'].contains(&chars[2]));
    assert!(['A', 'B', 'U'].contains(&chars[3]));
}

#[test]
fn board_serial_differs_from_unit_serial() {
    let id = "wch.cn_Legion_Go_S_BC4F5A06ABCD";
    assert_ne!(generate_serial(Some(id)), generate_board_serial(Some(id)));
}
