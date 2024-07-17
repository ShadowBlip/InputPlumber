use std::error::Error;

use packed_struct::PackedStructSlice;

use crate::drivers::dualsense::hid_report::InputState;

#[tokio::test]
async fn test_ds_hid() -> Result<(), Box<dyn Error>> {
    let mut report = InputState::default();
    println!("Before Report: {}", report.touch_data.touch_finger_data[0]);
    report.touch_data.touch_finger_data[0].set_y(1068);
    report.touch_data.touch_finger_data[0].set_x(1919);
    assert_eq!(report.touch_data.touch_finger_data[0].get_y(), 1068);
    assert_eq!(report.touch_data.touch_finger_data[0].get_x(), 1919);

    println!("After Report: {}", report.touch_data.touch_finger_data[0]);
    assert_eq!(
        report.touch_data.touch_finger_data[0]
            .pack_to_vec()
            .unwrap(),
        vec![0x80, 0x7F, 0xC7, 0x42]
    );

    Ok(())
}
