use std::error::Error;

use packed_struct::{
    types::{Integer, SizedInteger},
    PackedStruct,
};

use crate::drivers::fts3528::hid_report::PackedInputDataReport;

// Example packet:
// # ReportID: 1 / Tip Switch: 1 | # | # | Contact Id:    0 | X:    442 ,    442 | Y:    381 ,    381 | Width:     48 | Height:     48
// #             | Tip Switch: 0 | # | # | Contact Id:   15 | X:   4095 ,   4095 | Y:   4095 ,   4095 | Width:     48 | Height:     48
// #             | Tip Switch: 0 | # | # | Contact Id:   15 | X:   4095 ,   4095 | Y:   4095 ,   4095 | Width:     48 | Height:     48
// #             | Tip Switch: 0 | # | # | Contact Id:   15 | X:   4095 ,   4095 | Y:   4095 ,   4095 | Width:     48 | Height:     48 | Scan Time:  53800 | Contact Count:    1
// E: 000788.304103 60 01 01 00 ba 01 ba 01 7d 01 7d 01 30 00 30 00 00 0f ff 0f ff 0f ff 0f ff 0f 30 00 30 00 00 0f ff 0f ff 0f ff 0f ff 0f 30 00 30 00 00 0f ff 0f ff 0f ff 0f ff 0f 30 00 30 00 28 d2 01

#[tokio::test]
async fn test_opi_hid() -> Result<(), Box<dyn Error>> {
    let mut report = PackedInputDataReport::default();
    println!("Before Report: {}", report);
    report.touch1.tip_switch = 1;
    report.touch1.contact_id = 0;
    report.touch1.set_x(442);
    report.touch1.set_x2(442);
    report.touch1.set_y(381);
    report.touch1.set_y2(381);
    report.scan_time = Integer::from_primitive(53800);
    report.contact_count = 1;

    println!("After Report: {}", report);

    let expected: [u8; 60] = [
        0x01, 0x01, 0x00, 0xba, 0x01, 0xba, 0x01, 0x7d, 0x01, 0x7d, 0x01, 0x30, 0x00, 0x30, 0x00,
        0x00, 0x0f, 0xff, 0x0f, 0xff, 0x0f, 0xff, 0x0f, 0xff, 0x0f, 0x30, 0x00, 0x30, 0x00, 0x00,
        0x0f, 0xff, 0x0f, 0xff, 0x0f, 0xff, 0x0f, 0xff, 0x0f, 0x30, 0x00, 0x30, 0x00, 0x00, 0x0f,
        0xff, 0x0f, 0xff, 0x0f, 0xff, 0x0f, 0xff, 0x0f, 0x30, 0x00, 0x30, 0x00, 0x28, 0xd2, 0x01,
    ];
    let packed = report.pack().unwrap();

    assert_eq!(expected, packed);

    Ok(())
}
