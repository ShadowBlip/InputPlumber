use std::error::Error;

use packed_struct::prelude::*;

use super::driver::{accel_axis_calibration, gyro_axis_calibration};
use super::hid_report::{CalibrationReport, InputState};
use super::{
    DS5_ACCEL_RAW_TO_MPS2, DS5_CALIB_ACCEL_PLUS, DS5_CALIB_GYRO_PLUS, DS5_CALIB_GYRO_SPEED,
    DS5_GYRO_RAW_TO_RAD_S,
};

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
        vec![0xE4, 0x7F, 0xC7, 0x42]
    );

    Ok(())
}

#[test]
fn test_calibration_report_unpack() {
    // Calibration report bytes captured from a real DualSense controller.
    let buf: [u8; 41] = [
        0x05, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x83, 0x22, 0x78, 0xdd, 0x92, 0x22, 0x5f, 0xdd,
        0x95, 0x22, 0x6d, 0xdd, 0x1c, 0x02, 0x1c, 0x02, 0xf2, 0x1f, 0xed, 0xdf, 0xe3, 0x20, 0xda,
        0xe0, 0xee, 0x1f, 0xdf, 0xdf, 0x0b, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    let report = CalibrationReport::unpack(&buf).unwrap();

    assert_eq!(report.report_id, 0x05);
    assert_eq!(report.gyro_pitch_bias.to_primitive(), 0);
    assert_eq!(report.gyro_pitch_plus.to_primitive(), 8835);
    assert_eq!(report.gyro_pitch_minus.to_primitive(), -8840);
    assert_eq!(report.gyro_yaw_plus.to_primitive(), 8850);
    assert_eq!(report.gyro_yaw_minus.to_primitive(), -8865);
    assert_eq!(report.gyro_roll_plus.to_primitive(), 8853);
    assert_eq!(report.gyro_roll_minus.to_primitive(), -8851);
    assert_eq!(report.gyro_speed_plus.to_primitive(), 540);
    assert_eq!(report.gyro_speed_minus.to_primitive(), 540);
    assert_eq!(report.acc_x_plus.to_primitive(), 8178);
    assert_eq!(report.acc_x_minus.to_primitive(), -8211);
    assert_eq!(report.acc_y_plus.to_primitive(), 8419);
    assert_eq!(report.acc_y_minus.to_primitive(), -7974);
    assert_eq!(report.acc_z_plus.to_primitive(), 8174);
    assert_eq!(report.acc_z_minus.to_primitive(), -8225);
}

#[test]
fn test_calibration_report_default_round_trip() {
    let packed = CalibrationReport::default().pack().unwrap();
    assert_eq!(packed.len(), 41);
    assert_eq!(packed[0], 0x05);
    assert_eq!(
        CalibrationReport::unpack(&packed).unwrap(),
        CalibrationReport::default()
    );
}

#[test]
fn identity_calibration_matches_nominal_scale() {
    let speed_2x = (DS5_CALIB_GYRO_SPEED as i32) * 2;
    let gyro = gyro_axis_calibration(DS5_CALIB_GYRO_PLUS, -DS5_CALIB_GYRO_PLUS, 0, speed_2x);
    assert_eq!(gyro.bias, 0.0);
    assert!((gyro.scale / DS5_GYRO_RAW_TO_RAD_S - 1.0).abs() < 1e-3);

    let accel = accel_axis_calibration(DS5_CALIB_ACCEL_PLUS, -DS5_CALIB_ACCEL_PLUS);
    assert_eq!(accel.bias, 0.0);
    assert!((accel.scale / DS5_ACCEL_RAW_TO_MPS2 - 1.0).abs() < 1e-3);
}

#[test]
fn real_calibration_matches_corrected_scale() {
    let gyro = gyro_axis_calibration(8835, -8840, 0, 1080);
    assert!((gyro.scale / DS5_GYRO_RAW_TO_RAD_S - 1.0).abs() < 1e-2);
}

#[test]
fn zero_denominator_falls_back_without_panicking() {
    let gyro = gyro_axis_calibration(0, 0, 0, 0);
    assert!(gyro.scale.is_finite() && gyro.scale > 0.0);

    let accel = accel_axis_calibration(0, 0);
    assert_eq!(accel.bias, 0.0);
    assert!(accel.scale.is_finite() && accel.scale > 0.0);
}
