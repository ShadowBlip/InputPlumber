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
