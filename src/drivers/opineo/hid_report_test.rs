use std::error::Error;

use packed_struct::types::{Integer, SizedInteger};

use crate::drivers::opineo::hid_report::TouchpadDataReport;

#[tokio::test]
async fn test_opi_hid() -> Result<(), Box<dyn Error>> {
    let mut report = TouchpadDataReport::default();
    println!("Before Report: {}", report);
    report.touch_x = Integer::from_primitive(467);
    report.touch_y = Integer::from_primitive(512);

    println!("After Report: {}", report);
    assert_eq!(report.touch_x.to_primitive(), 467);
    assert_eq!(report.touch_y.to_primitive(), 512);

    Ok(())
}
