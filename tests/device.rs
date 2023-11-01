#[cfg(feature = "tokio")]

use std::assert_eq;
use std::error::Error;

use handbus::input::device;

#[tokio::test]
async fn test_get_devices() -> Result<(), Box<dyn Error>> {
    let devices = device::get_all().unwrap();
    for device in devices {
        println!("{:?}", device);
    }

    Ok(())
}
