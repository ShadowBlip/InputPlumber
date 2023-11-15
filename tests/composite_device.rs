#[cfg(feature = "tokio")]
use std::assert_eq;
use std::error::Error;

use handbus::gamepad::composite_device;

#[tokio::test]
async fn test_load_device_yaml() -> Result<(), Box<dyn Error>> {
    let device = composite_device::CompositeDevice::from_yaml_file(String::from(
        "rootfs/usr/share/handbus/devices/onexplayer_mini_a07.yaml",
    ))?;
    println!("{:?}", device);
    assert_eq!(device.name, "OneXPlayer mini A07");
    assert_eq!(device.kind, "CompositeDevice");
    assert_eq!(device.version, 1);
    assert_eq!(device.matches.iter().len(), 1);
    assert_eq!(device.source_devices.iter().len(), 2);
    assert_eq!(device.event_map_id, "oxp3");
    Ok(())
}
