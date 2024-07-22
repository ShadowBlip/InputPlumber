use std::error::Error;

use tokio::sync::mpsc::channel;

use crate::{
    drivers::dualsense::driver::Driver,
    input::{
        composite_device::client::CompositeDeviceClient,
        event::{native::NativeEvent, Event},
    },
};

use super::{InputError, SourceDeviceService, SourceInputDevice, SourceOutputDevice};

struct Test {
    driver: Driver,
}

impl Test {
    fn new() -> Self {
        let device_path = "/dev/hidraw0".to_string();
        let driver = Driver::new(device_path.clone()).unwrap();
        Self { driver }
    }
}

impl SourceInputDevice for Test {
    fn poll(&mut self) -> Result<Vec<NativeEvent>, InputError> {
        let events = self.driver.poll()?;
        let events = vec![];
        Ok(events)
    }
}

impl SourceOutputDevice for Test {}

//#[tokio::test]
async fn test_traits() -> Result<(), Box<dyn Error>> {
    let (tx, rx) = channel(1024);
    let composite_dev = CompositeDeviceClient::new(tx);
    let test_device = Test::new();

    let device = SourceDeviceService::new(composite_dev, test_device);

    device.run().await?;

    Ok(())
}
