#[cfg(feature = "tokio")]

use std::assert_eq;
use std::error::Error;

use tokio::sync::mpsc;
use handbus::gamepad::manager;

#[tokio::test]
async fn test_manager() -> Result<(), Box<dyn Error>> {
    let (manager_tx, manager_rx) = mpsc::channel(32);
    let (_, manager_backend) = manager::new(manager_tx, manager_rx);

    manager_backend.discover_devices();

    Ok(())
}
