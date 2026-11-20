//use std::error::Error;
//
//use packed_struct::types::{Integer, SizedInteger};
//
//use crate::drivers::xpad_uhid::hid_report::DataReport;
//
//#[tokio::test]
//async fn test_opi_hid() -> Result<(), Box<dyn Error>> {
//    let mut report = DataReport::default();
//    println!("Before Report: {}", report);
//    report.l_stick_x = Integer::from_primitive(467);
//    report.r_stick_y = Integer::from_primitive(512);
//
//    println!("After Report: {}", report);
//    assert_eq!(report.l_stick_x.to_primitive(), 467);
//    assert_eq!(report.r_stick_y.to_primitive(), 512);
//
//    Ok(())
//}

use packed_struct::PackedStruct;

use crate::drivers::xpad_uhid::hid_report::XBoxSeriesInputDataReport;

fn elite_report(profile: u8, paddles: u8) -> [u8; 20] {
    [
        0x01, 0x00, 0x80, 0x00, 0x80, 0x00, 0x80, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, profile, 0x00, paddles,
    ]
}

#[test]
fn test_elite_paddles() {
    // Paddle bits observed per paddle: upper right, lower right, upper left, lower left
    let cases = [
        (0x01, [false, false, true, false]),
        (0x02, [false, false, false, true]),
        (0x04, [true, false, false, false]),
        (0x08, [false, true, false, false]),
        (0x00, [false, false, false, false]),
    ];
    for (byte, expected) in cases {
        let report = XBoxSeriesInputDataReport::unpack(&elite_report(0x00, byte)).unwrap();
        let paddles = report.active_paddles();
        assert_eq!(
            [paddles.l4, paddles.l5, paddles.r4, paddles.r5],
            expected,
            "paddle byte {byte:#04x}"
        );
    }
}

#[test]
fn test_elite_paddles_ignored_with_profile_active() {
    for profile in 1..=3 {
        let report = XBoxSeriesInputDataReport::unpack(&elite_report(profile, 0x01)).unwrap();
        assert_eq!(
            report.active_paddles(),
            Default::default(),
            "profile {profile}"
        );
    }
}
