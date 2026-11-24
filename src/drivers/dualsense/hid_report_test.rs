use std::error::Error;

use packed_struct::prelude::*;

use super::driver::{accel_axis_calibration, gyro_axis_calibration};
use super::hid_report::{
    output_report_bt_crc32, BluetoothPackedOutputReport, CalibrationReport, InputState,
    SetStatePackedOutputData, OUTPUT_REPORT_BT_TAG,
};
use super::{
    DS5_ACCEL_RAW_TO_MPS2, DS5_CALIB_ACCEL_PLUS, DS5_CALIB_GYRO_PLUS, DS5_CALIB_GYRO_SPEED,
    DS5_GYRO_RAW_TO_RAD_S, OUTPUT_REPORT_BT, OUTPUT_REPORT_BT_SIZE,
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

#[test]
fn bt_output_crc_is_seeded_with_hidp_header() {
    assert_eq!(
        output_report_bt_crc32(&[0x31, 0x20, 0x10, 0xAB, 0xCD]),
        0x6D65_5711
    );
}

#[test]
fn bt_output_report_layout() -> Result<(), Box<dyn Error>> {
    let state = SetStatePackedOutputData {
        allow_led_color: true,
        led_red: 0x11,
        led_green: 0x22,
        led_blue: 0x33,
        ..Default::default()
    };
    let buf = BluetoothPackedOutputReport::new(state, 0x13).pack_with_crc()?;

    assert_eq!(buf.len(), OUTPUT_REPORT_BT_SIZE);
    assert_eq!(buf[0], OUTPUT_REPORT_BT);
    assert_eq!(buf[1], 0x30);
    assert_eq!(buf[2], OUTPUT_REPORT_BT_TAG);
    assert_eq!(&buf[3..50], &state.pack()?[..]);
    assert!(buf[50..74].iter().all(|b| *b == 0));
    let crc = output_report_bt_crc32(&buf[..74]);
    assert_eq!(&buf[74..], &crc.to_le_bytes());

    Ok(())
}

#[test]
fn bt_output_report_round_trip() -> Result<(), Box<dyn Error>> {
    let state = SetStatePackedOutputData {
        allow_right_trigger_ffb: true,
        right_trigger_ffb: [0x21, 0xFF, 0x03, 0, 0, 0, 0, 0, 0, 0, 0],
        enable_rumble_emulation: true,
        use_rumble_not_haptics: true,
        rumble_emulation_left: 90,
        ..Default::default()
    };
    let buf = BluetoothPackedOutputReport::new(state, 7).pack_with_crc()?;
    let report = BluetoothPackedOutputReport::unpack(&buf)?;

    assert_eq!(report.state, state);
    assert_eq!(report.seq_tag, 0x70);
    assert_eq!(
        report.crc32.to_primitive(),
        output_report_bt_crc32(&buf[..74])
    );

    Ok(())
}

#[test]
fn merge_keeps_sections_not_set_by_newer_state() {
    let mut merged = SetStatePackedOutputData {
        allow_led_color: true,
        led_red: 255,
        led_blue: 128,
        ..Default::default()
    };
    merged.merge(&SetStatePackedOutputData {
        use_rumble_not_haptics: true,
        enable_rumble_emulation: true,
        rumble_emulation_left: 200,
        rumble_emulation_right: 100,
        ..Default::default()
    });
    merged.merge(&SetStatePackedOutputData {
        use_rumble_not_haptics: true,
        enable_rumble_emulation: true,
        ..Default::default()
    });

    assert!(merged.allow_led_color);
    assert_eq!(
        (merged.led_red, merged.led_green, merged.led_blue),
        (255, 0, 128)
    );
    assert!(merged.enable_rumble_emulation && merged.use_rumble_not_haptics);
    assert_eq!(
        (merged.rumble_emulation_left, merged.rumble_emulation_right),
        (0, 0)
    );
    assert!(!merged.allow_right_trigger_ffb && !merged.allow_left_trigger_ffb);
}

#[test]
fn merge_replaces_trigger_effects_per_side() {
    let mut merged = SetStatePackedOutputData {
        allow_right_trigger_ffb: true,
        right_trigger_ffb: [1; 11],
        ..Default::default()
    };
    merged.merge(&SetStatePackedOutputData {
        allow_left_trigger_ffb: true,
        left_trigger_ffb: [2; 11],
        ..Default::default()
    });
    merged.merge(&SetStatePackedOutputData {
        allow_right_trigger_ffb: true,
        right_trigger_ffb: [3; 11],
        ..Default::default()
    });

    assert_eq!(merged.right_trigger_ffb, [3; 11]);
    assert_eq!(merged.left_trigger_ffb, [2; 11]);
}

#[test]
fn would_change_ignores_repeated_states() {
    let led = SetStatePackedOutputData {
        allow_led_color: true,
        led_red: 10,
        led_green: 20,
        led_blue: 30,
        ..Default::default()
    };
    let rumble = SetStatePackedOutputData {
        use_rumble_not_haptics: true,
        enable_rumble_emulation: true,
        rumble_emulation_left: 120,
        ..Default::default()
    };
    let mut device = SetStatePackedOutputData::default();
    device.merge(&led);
    device.merge(&rumble);

    assert!(!device.would_change(&led));
    assert!(!device.would_change(&rumble));
    assert!(!device.would_change(&SetStatePackedOutputData {
        host_timestamp: 1234.into(),
        ..led
    }));
    assert!(device.would_change(&SetStatePackedOutputData {
        led_blue: 31,
        ..led
    }));
    assert!(device.would_change(&SetStatePackedOutputData {
        rumble_emulation_left: 0,
        ..rumble
    }));
    assert!(device.would_change(&SetStatePackedOutputData {
        allow_right_trigger_ffb: true,
        right_trigger_ffb: [0x21, 0xFF, 0x03, 0, 0, 0, 0, 0, 0, 0, 0],
        ..Default::default()
    }));
    assert!(device.would_change(&SetStatePackedOutputData {
        reset_lights: true,
        ..Default::default()
    }));
}
