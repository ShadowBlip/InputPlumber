use tokio::sync::watch::Sender;
use tokio::time::{Duration, sleep};

/// Time in milliseconds to poll for gamepad changes
const POLL_DURATION: u64 = 1000;


/// The type of watch event that occurred
pub enum WatchEventType {
    Added,
    Removed,
}

/// Emitted when an input device has changed
pub struct WatchEvent {
    pub path: String,
    pub kind: WatchEventType,
}

impl WatchEvent {
    /// Returns a new WatchEvent
    pub fn new(path: String, kind: WatchEventType) -> WatchEvent {
        WatchEvent { path, kind }
    }
}

impl std::fmt::Display for WatchEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.path)
    }
}

/// Watches for connecting and disconnecting gamepads and sends a signal with
/// the connecting or disconnecting device.
pub struct Watcher<'sender> {
    channel: Sender<&'sender WatchEvent>,
}

impl Watcher<'_> {
    /// Return a new instance of [Watcher] using the given sender-side of a 
    /// watch channel. The watcher will send messages when gamepads are added
    /// or removed.
    pub fn new(channel: Sender<&WatchEvent>) -> Watcher {
        Watcher{channel}
    }

    /// Start watching for gamepad devices
    pub async fn watch(&self) {
        loop {
            let path = String::from("/dev/input/event1");
            let event = WatchEvent::new(path, WatchEventType::Added);
            let _ = self.channel.send(&event);
            let _ = sleep(Duration::from_millis(POLL_DURATION)).await;
        }
    }
}
