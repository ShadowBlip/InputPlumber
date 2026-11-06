use std::error::Error;

use crate::udev::{device::parse_hid_id, get_device};

#[test]
fn parse_hid_id_reads_bus_vendor_and_product() {
    assert_eq!(
        parse_hid_id("0005:0000054C:00000CE6"),
        Some((0x0005, 0x054C, 0x0CE6))
    );
    assert_eq!(
        parse_hid_id("0018:00000B05:00001ABE"),
        Some((0x0018, 0x0B05, 0x1ABE))
    );
    assert_eq!(parse_hid_id(""), None);
    assert_eq!(parse_hid_id("0005:0000054C"), None);
    assert_eq!(parse_hid_id("0005:1000054C:00000CE6"), None);
}

#[tokio::test]
async fn test_get_device() -> Result<(), Box<dyn Error>> {
    let path = "/dev/hidraw0";
    if let Ok(device) = get_device(path.to_string()).await {
        println!("Parsed device: {:?}", device);
        println!("Parent: {:?}", device.get_parent());
        println!("Parent name: {:?}", device.get_parent_device_name());
        println!("Vendor ID: {:?}", device.get_vendor_id());
        println!("Product ID: {:?}", device.get_product_id());
    }

    Ok(())
}
