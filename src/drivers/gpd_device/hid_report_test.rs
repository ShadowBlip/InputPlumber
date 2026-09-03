use packed_struct::PackedStruct;

use crate::drivers::gpd_device::hid_report::TouchpadDataReport2024;

#[test]
fn test_parse_touch_report() {
    // Two finger touch captured from a GPD Win Mini (2024) touchpad.
    // Finger 0: confidence=1, tip=1, cid=0, x=0x0812=2066, y=0x02a8=680
    // Finger 1: confidence=1, tip=1, cid=1, x=0x03f8=1016, y=0x03b9=953
    let buf: [u8; 29] = [
        0x04, 0x03, 0x12, 0x08, 0xa8, 0x02, 0x07, 0xf8, 0x03, 0xb9, 0x03, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xe8, 0xfe, 0x02, 0x03, 0x00, 0x00, 0x40,
        0x4c,
    ];
    let report = TouchpadDataReport2024::unpack(&buf).unwrap();

    assert_eq!(report.report_id, 0x04);
    assert_eq!(report.confidence0, true);
    assert_eq!(report.tip_switch0, true);
    assert_eq!(report.contact_id0, 0);
    assert_eq!(report.touch_x0, 2066);
    assert_eq!(report.touch_y0, 680);

    assert_eq!(report.confidence1, true);
    assert_eq!(report.tip_switch1, true);
    assert_eq!(report.contact_id1, 1);
    assert_eq!(report.touch_x1, 1016);
    assert_eq!(report.touch_y1, 953);

    assert_eq!(report.contact_count, 2);
    assert_eq!(report.scan_time, 0xfee8);

    // Release report: confidence=1, tip=0 for both fingers
    let buf: [u8; 29] = [
        0x04, 0x01, 0x32, 0x08, 0xa6, 0x02, 0x05, 0xf5, 0x03, 0xc2, 0x03, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x68, 0x01, 0x02, 0x04, 0x00, 0x00, 0x40,
        0x41,
    ];
    let report = TouchpadDataReport2024::unpack(&buf).unwrap();
    assert_eq!(report.tip_switch0, false);
    assert_eq!(report.confidence0, true);
    assert_eq!(report.touch_x0, 2098);
    assert_eq!(report.touch_y0, 678);
}
