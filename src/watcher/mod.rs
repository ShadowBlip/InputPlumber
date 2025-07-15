use inotify::{EventMask, Inotify, WatchMask};
use tokio::sync::mpsc::Sender;

#[derive(Debug, Clone)]
pub enum WatchEvent {
    Create { name: String, base_path: String },
    Delete { name: String, base_path: String },
}

/// Watch for filesystem changes on the given path, sending [WatchEvent]
/// to the given channel.
pub fn watch(path: String, tx: Sender<WatchEvent>) {
    let mut inotify = Inotify::init().expect("Failed to initialize inotify");

    if let Err(e) = inotify
        .watches()
        .add(path.clone(), WatchMask::CREATE | WatchMask::DELETE)
    {
        log::error!(
            "Unable to add inotify wather for path: {path}. Got error {:?}",
            e
        );
        return;
    }

    // Listen for watch events
    let mut buffer = [0u8; 4096];
    // Use the equivalent of a "do-while" loop so the initial value is
    // processed before awaiting the `changed()` future.
    loop {
        let events = inotify
            .read_events_blocking(&mut buffer)
            .expect("Failed to read inotify events");

        for event in events {
            // Send the event over our channel
            let name = String::from(event.name.unwrap().to_str().unwrap());

            if event.mask.contains(EventMask::CREATE) {
                log::debug!("inotify CREATE: {:?}", event.name);
                let value = WatchEvent::Create {
                    name,
                    base_path: path.clone(),
                };
                match tx.blocking_send(value) {
                    Ok(_) => (),
                    Err(e) => log::error!("Error sending event: {}", e),
                }
                //if event.mask.contains(EventMask::ISDIR) {
                //    println!("Directory created: {:?}", event.name);
                //} else {
                //    println!("File created: {:?}", event.name);
                //}
            } else if event.mask.contains(EventMask::DELETE) {
                log::debug!("inotify DELETE: {:?}", event.name);
                let value = WatchEvent::Delete {
                    name,
                    base_path: path.clone(),
                };
                match tx.blocking_send(value) {
                    Ok(_) => (),
                    Err(e) => log::error!("Error sending event: {}", e),
                }
            } else if event.mask.contains(EventMask::MODIFY) {
                log::trace!("inotify MODIFY: {:?}", event.name);
            }
        }
    }
}
