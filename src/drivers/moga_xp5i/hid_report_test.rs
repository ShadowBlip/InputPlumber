use super::hid_report::MogaReport;

#[test]
fn idle_report_is_neutral() {
    // RT rests around 0x0b on this unit; it must be treated as released.
    let report = MogaReport::from_bytes(&[0, 0, 0, 0, 0, 0x0b, 0, 0]).unwrap();
    assert_eq!(report, MogaReport::default());
}

#[test]
fn parses_face_buttons() {
    let a = MogaReport::from_bytes(&[0, 0, 0, 0, 0, 0, 0x10, 0]).unwrap();
    assert!(a.a);
    assert!(!a.b && !a.x && !a.y);

    let b = MogaReport::from_bytes(&[0, 0, 0, 0, 0, 0, 0x20, 0]).unwrap();
    assert!(b.b);

    let x = MogaReport::from_bytes(&[0, 0, 0, 0, 0, 0, 0x40, 0]).unwrap();
    assert!(x.x);

    let y = MogaReport::from_bytes(&[0, 0, 0, 0, 0, 0, 0x80, 0]).unwrap();
    assert!(y.y);
}

#[test]
fn parses_dpad() {
    let up = MogaReport::from_bytes(&[0, 0, 0, 0, 0, 0, 0x01, 0]).unwrap();
    assert!(up.dpad_up);
    assert!(!up.dpad_right && !up.dpad_down && !up.dpad_left);

    let right = MogaReport::from_bytes(&[0, 0, 0, 0, 0, 0, 0x02, 0]).unwrap();
    assert!(right.dpad_right);
    assert!(!right.dpad_up && !right.dpad_down && !right.dpad_left);

    let down = MogaReport::from_bytes(&[0, 0, 0, 0, 0, 0, 0x04, 0]).unwrap();
    assert!(down.dpad_down);
    assert!(!down.dpad_up && !down.dpad_right && !down.dpad_left);

    let left = MogaReport::from_bytes(&[0, 0, 0, 0, 0, 0, 0x08, 0]).unwrap();
    assert!(left.dpad_left);
    assert!(!left.dpad_up && !left.dpad_right && !left.dpad_down);
}

#[test]
fn parses_aux_buttons() {
    let report = MogaReport::from_bytes(&[0, 0, 0, 0, 0, 0, 0, 0x7f]).unwrap();
    assert!(report.lb);
    assert!(report.rb);
    assert!(report.l3);
    assert!(report.r3);
    assert!(report.guide);
    assert!(report.select);
    assert!(report.start);
}

#[test]
fn applies_stick_deadzone_and_rescales() {
    let report = MogaReport::from_bytes(&[16, 17, 129, 127, 0, 0, 0, 0]).unwrap();
    assert_eq!(report.left_stick_x, 0);
    assert_eq!(report.left_stick_y, 1);
    assert_eq!(report.right_stick_x, -127);
    assert_eq!(report.right_stick_y, 127);
}

#[test]
fn applies_trigger_deadzone_and_rescales() {
    let report = MogaReport::from_bytes(&[0, 0, 0, 0, 24, 25, 0, 0]).unwrap();
    assert_eq!(report.left_trigger, 0);
    assert_eq!(report.right_trigger, 1);

    let report = MogaReport::from_bytes(&[0, 0, 0, 0, 255, 100, 0, 0]).unwrap();
    assert_eq!(report.left_trigger, 255);
    assert_eq!(report.right_trigger, 83);
}

#[test]
fn rejects_short_reports() {
    assert_eq!(MogaReport::from_bytes(&[0, 0, 0]), None);
}
