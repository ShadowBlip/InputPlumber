#[cfg(feature = "tokio")]
use std::assert_eq;
use std::error::Error;

use handbus::gamepad::manager;
use tokio::sync::mpsc;

#[tokio::test]
async fn test_manager() -> Result<(), Box<dyn Error>> {
    let (manager_tx, manager_rx) = mpsc::channel(32);
    let (_, manager_backend) = manager::new(manager_tx, manager_rx);

    manager_backend.discover_devices();

    Ok(())
}

#[tokio::test]
async fn test_load_composite_devices() -> Result<(), Box<dyn Error>> {
    let devices = manager::Manager::load_composite_devices();
    println!("{:?}", devices);
    assert!(!devices.is_empty());

    Ok(())
}
