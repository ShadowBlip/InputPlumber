use std::error::Error;

use crate::drivers::dualsense::hid_report::USBPackedInputDataReport;

#[tokio::test]
async fn test_ds_hid() -> Result<(), Box<dyn Error>> {
    let mut report = USBPackedInputDataReport::default();
    println!("Before Report: {}", report.touch_data.touch_finger_data[0]);
    report.touch_data.touch_finger_data[0].set_y(1068);
    report.touch_data.touch_finger_data[0].set_x(1919);

    println!("After Report: {}", report.touch_data.touch_finger_data[0]);
    println!("Expected: 80  87 43");

    Ok(())
}
