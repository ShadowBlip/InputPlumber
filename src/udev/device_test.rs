use std::error::Error;

use crate::udev::get_device;

#[tokio::test]
async fn test_get_device() -> Result<(), Box<dyn Error>> {
    let path = "/dev/input/event21";
    let device = get_device(path.to_string()).await?;
    println!("Parsed device: {:?}", device);
    println!("Parent: {:?}", device.get_parent());
    println!("Parent name: {:?}", device.get_parent_device_name());
    println!("Vendor ID: {:?}", device.get_vendor_id());
    println!("Product ID: {:?}", device.get_product_id());

    Ok(())
}
