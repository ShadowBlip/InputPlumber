use super::*;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

#[derive(Clone, Default)]
struct TestClock(Arc<AtomicU64>);
impl Clock for TestClock {
    fn now(&self) -> Duration {
        Duration::from_millis(self.0.load(Ordering::SeqCst))
    }
}
impl TestClock {
    fn advance(&self, ms: u64) {
        self.0.fetch_add(ms, Ordering::SeqCst);
    }
}
type RecordedFrame = (LedConfig, [u8; 3], bool);

#[derive(Clone, Default)]
struct TestDevice {
    frames: Arc<Mutex<Vec<RecordedFrame>>>,
    failed: Arc<AtomicBool>,
}
impl Transport for TestDevice {
    fn effects(&self) -> Vec<String> {
        ["off", "solid", "breathing", "cycle"]
            .map(str::to_owned)
            .to_vec()
    }
    fn write(&mut self, config: &LedConfig, color: [u8; 3], transition: bool) -> Result<()> {
        if self.failed.load(Ordering::SeqCst) {
            return Err("injected transport failure".into());
        }
        self.frames
            .lock()
            .unwrap()
            .push((config.clone(), color, transition));
        Ok(())
    }
}
#[derive(Clone, Default)]
struct TestStorage {
    config: Arc<Mutex<Option<LedConfig>>>,
    failed: Arc<AtomicBool>,
    writes: Arc<AtomicUsize>,
}
impl Storage for TestStorage {
    fn load(&mut self) -> Result<Option<LedConfig>> {
        Ok(self.config.lock().unwrap().clone())
    }
    fn save(&mut self, config: &LedConfig) -> Result<SaveOutcome> {
        if self.failed.load(Ordering::SeqCst) {
            return Err("injected storage failure".into());
        }
        *self.config.lock().unwrap() = Some(config.clone());
        self.writes.fetch_add(1, Ordering::SeqCst);
        Ok(SaveOutcome::Durable)
    }
}
fn setup() -> (Engine, TestDevice, TestStorage, TestClock) {
    let device = TestDevice::default();
    let storage = TestStorage::default();
    let clock = TestClock::default();
    (
        Engine::new(
            "ayaneo-3-joystick-rings".into(),
            Box::new(device.clone()),
            Box::new(storage.clone()),
            Box::new(clock.clone()),
        ),
        device,
        storage,
        clock,
    )
}
fn config(effect: &str) -> LedConfig {
    LedConfig {
        effect: effect.into(),
        ..LedConfig::default()
    }
}

#[test]
fn invalid_requests_are_atomic_and_never_touch_storage_or_hardware() {
    let (mut engine, device, storage, _) = setup();
    let before = engine.snapshot.clone();
    let mut inputs = vec![config("unknown")];
    for rgb in [vec![], vec![0], vec![0, 1], vec![0, 1, 2, 3]] {
        inputs.push(LedConfig {
            color: rgb,
            ..config("solid")
        });
    }
    inputs.push(LedConfig {
        brightness: 101,
        ..config("solid")
    });
    for period in [0, 1999, 30001, u32::MAX] {
        inputs.push(LedConfig {
            cycle_period_ms: period,
            ..config("cycle")
        });
    }
    for input in inputs {
        assert!(engine.apply(input).is_err());
        assert_eq!(engine.snapshot, before);
    }
    assert_eq!(storage.writes.load(Ordering::SeqCst), 0);
    assert!(device.frames.lock().unwrap().is_empty());
}
#[test]
fn every_transition_preserves_saved_color_and_brightness() {
    for from in ["off", "solid", "breathing", "cycle"] {
        for to in ["off", "solid", "breathing", "cycle"] {
            let (mut engine, device, storage, clock) = setup();
            engine.apply(config(from)).unwrap();
            engine.tick();
            clock.advance(100);
            let next = LedConfig {
                color: vec![10, 20, 30],
                brightness: 25,
                ..config(to)
            };
            let revision = engine.apply(next.clone()).unwrap();
            assert_eq!(revision, 2);
            assert_eq!(engine.snapshot.state.status, "pending");
            assert_eq!(*storage.config.lock().unwrap(), Some(next.clone()));
            engine.tick();
            assert_eq!(engine.snapshot.state.status, "applied");
            let frames = device.frames.lock().unwrap();
            assert_eq!(frames.len(), 2);
            assert_eq!(frames[1].0, next);
            assert!(frames[1].2);
        }
    }
}
#[test]
fn frames_are_bounded_and_stale_frames_are_discarded() {
    let (mut engine, device, _, clock) = setup();
    engine.apply(config("cycle")).unwrap();
    engine.tick();
    for _ in 0..99 {
        clock.advance(1);
        engine.tick();
    }
    assert_eq!(device.frames.lock().unwrap().len(), 1);
    clock.advance(1);
    engine.tick();
    assert_eq!(device.frames.lock().unwrap().len(), 2);
    clock.advance(3900);
    engine.tick();
    let frames = device.frames.lock().unwrap();
    assert_eq!(frames.len(), 3);
    assert_eq!(frames[2].1, [0, 255, 255]);
    assert_eq!(cycle_color(Duration::from_millis(8000), 8000), [255, 0, 0]);
}
#[test]
fn rapid_configuration_changes_coalesce_to_latest_accepted_frame() {
    let (mut engine, device, _, clock) = setup();
    engine.tick();
    for n in 0..30 {
        engine
            .apply(LedConfig {
                brightness: n,
                ..config("solid")
            })
            .unwrap();
        engine.tick();
    }
    assert_eq!(device.frames.lock().unwrap().len(), 1);
    clock.advance(100);
    engine.tick();
    assert_eq!(
        device.frames.lock().unwrap().last().unwrap().0.brightness,
        29
    );
}
#[test]
fn transport_failure_stops_animation_until_explicit_retry() {
    let (mut engine, device, storage, clock) = setup();
    engine.apply(config("cycle")).unwrap();
    device.failed.store(true, Ordering::SeqCst);
    engine.tick();
    assert_eq!(engine.snapshot.state.status, "failed");
    device.failed.store(false, Ordering::SeqCst);
    clock.advance(5000);
    engine.tick();
    assert!(device.frames.lock().unwrap().is_empty());
    assert_eq!(
        storage.config.lock().unwrap().as_ref().unwrap().effect,
        "cycle"
    );
    engine.apply(config("solid")).unwrap();
    engine.tick();
    assert_eq!(engine.snapshot.state.status, "applied");
}
#[test]
fn storage_failure_keeps_previous_saved_state_and_output() {
    let (mut engine, device, storage, clock) = setup();
    engine.apply(config("solid")).unwrap();
    engine.tick();
    let before = engine.snapshot.clone();
    storage.failed.store(true, Ordering::SeqCst);
    assert!(engine.apply(config("breathing")).is_err());
    assert_eq!(engine.snapshot, before);
    clock.advance(100);
    engine.tick();
    assert_eq!(device.frames.lock().unwrap().len(), 1);
    assert_eq!(
        storage.config.lock().unwrap().as_ref().unwrap().effect,
        "solid"
    );
}
#[test]
fn suspend_saves_nothing_and_resume_restores_latest_configuration() {
    let (mut engine, device, storage, clock) = setup();
    engine.apply(config("breathing")).unwrap();
    engine.tick();
    engine.suspend().unwrap();
    assert_eq!(
        device.frames.lock().unwrap().last().unwrap().0.effect,
        "off"
    );
    assert_eq!(storage.writes.load(Ordering::SeqCst), 1);
    engine.apply(config("cycle")).unwrap();
    clock.advance(10000);
    engine.tick();
    assert_eq!(device.frames.lock().unwrap().len(), 2);
    assert_eq!(engine.snapshot.state.status, "unavailable");
    engine.resume();
    engine.tick();
    assert_eq!(
        device.frames.lock().unwrap().last().unwrap().0.effect,
        "cycle"
    );
}
#[test]
fn new_enumeration_and_process_restart_restore_profile_identity() {
    let (mut first, device, storage, clock) = setup();
    let saved = LedConfig {
        color: vec![99, 10, 9],
        ..config("solid")
    };
    first.apply(saved.clone()).unwrap();
    first.tick();
    first.suspend().unwrap();
    let mut restarted = Engine::new(
        "ayaneo-3-joystick-rings".into(),
        Box::new(device.clone()),
        Box::new(storage),
        Box::new(clock),
    );
    restarted.tick();
    assert_eq!(restarted.snapshot.state.config, saved);
    assert_eq!(restarted.snapshot.state.status, "applied");
}

fn temp_dir(label: &str) -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let path = std::env::temp_dir().join(format!(
        "inputplumber-led-{label}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::SeqCst)
    ));
    fs::create_dir(&path).unwrap();
    path
}
#[test]
fn persistence_is_versioned_and_uses_a_stable_safe_path() {
    let path = temp_dir("storage");
    let mut store = FileStorage::new(&path, "ayaneo-3-joystick-rings").unwrap();
    assert!(store.load().unwrap().is_none());
    store.save(&config("cycle")).unwrap();
    assert_eq!(store.load().unwrap(), Some(config("cycle")));
    assert_eq!(fs::read_dir(&path).unwrap().count(), 1);
    assert!(FileStorage::new(&path, "../escape").is_err());
    assert!(FileStorage::new(&path, "").is_err());
    let file = path.join("ayaneo-3-joystick-rings.json");
    fs::write(&file, r#"{"version":9,"config":{"effect":"off","color":[255,255,255],"brightness":30,"cycle_period_ms":8000}}"#).unwrap();
    assert!(store.load().is_err());
    fs::write(&file, "broken").unwrap();
    assert!(store.load().is_err());
    fs::remove_dir_all(path).unwrap();
}
#[test]
fn sysfs_conversion_honors_channel_order_range_and_hardware_breathing() {
    let path = temp_dir("sysfs");
    for (name, value) in [
        ("max_brightness", "1023"),
        ("multi_index", "blue red green"),
        ("trigger", "[none] pattern"),
        ("brightness", "0"),
        ("multi_intensity", "0 0 0"),
        ("hw_pattern", ""),
    ] {
        fs::write(path.join(name), value).unwrap();
    }
    let mut device = SysfsTransport::open(path.clone()).unwrap();
    let setting = LedConfig {
        brightness: 50,
        ..config("solid")
    };
    device.write(&setting, [255, 128, 0], true).unwrap();
    assert_eq!(
        fs::read_to_string(path.join("multi_intensity")).unwrap(),
        "0 1023 514"
    );
    assert_eq!(fs::read_to_string(path.join("brightness")).unwrap(), "512");
    device.write(&config("breathing"), [255; 3], true).unwrap();
    assert_eq!(
        fs::read_to_string(path.join("hw_pattern")).unwrap(),
        "0 1000 307 1000"
    );
    device.write(&config("off"), [255; 3], true).unwrap();
    assert_eq!(fs::read_to_string(path.join("trigger")).unwrap(), "none");
    assert_eq!(fs::read_to_string(path.join("brightness")).unwrap(), "0");
    fs::remove_file(path.join("multi_intensity")).unwrap();
    assert!(device.write(&config("solid"), [255; 3], true).is_err());
    assert!(!path.join("multi_intensity").exists());
    fs::remove_dir_all(path).unwrap();
}
#[test]
fn unsupported_and_malformed_sysfs_are_rejected_without_panics() {
    let path = temp_dir("invalid-sysfs");
    for maximum in ["broken", "0", "4294967296"] {
        fs::write(path.join("max_brightness"), maximum).unwrap();
        assert!(SysfsTransport::open(path.clone()).is_err());
    }
    fs::write(path.join("max_brightness"), "255").unwrap();
    for channels in ["", "red green", "red green red", "red green blue white"] {
        fs::write(path.join("multi_index"), channels).unwrap();
        assert!(SysfsTransport::open(path.clone()).is_err());
    }
    fs::remove_dir_all(path).unwrap();
}

#[tokio::test]
async fn worker_concurrent_clients_are_serialized_and_notifications_show_saved_state() {
    let (engine, _, storage, _) = setup();
    let handle = LedHandle::default();
    handle.start(engine).unwrap();
    let mut changes = handle.subscribe();
    let (a, b) = tokio::join!(
        handle.apply(config("solid")),
        handle.apply(config("breathing"))
    );
    let (a, b) = (a.unwrap(), b.unwrap());
    assert_ne!(a, b);
    changes.changed().await.unwrap();
    let snapshot = handle.snapshot();
    assert_eq!(snapshot.state.revision, 2);
    assert_eq!(*storage.config.lock().unwrap(), Some(snapshot.state.config));
    handle.set_suspended(true).await.unwrap();
    handle.stop();
}

#[derive(Clone)]
struct TestAuthority {
    mode: Arc<AtomicUsize>,
    callers: Arc<Mutex<Vec<String>>>,
}
#[derive(Serialize, Deserialize, Type)]
struct AuthorizationResult(bool, bool, HashMap<String, String>);
#[zbus_macros::interface(name = "org.freedesktop.PolicyKit1.Authority")]
impl TestAuthority {
    fn check_authorization(
        &self,
        subject: (String, HashMap<String, zbus::zvariant::OwnedValue>),
        action_id: &str,
        _details: HashMap<String, String>,
        _flags: u32,
        _cancellation_id: &str,
    ) -> AuthorizationResult {
        assert_eq!(subject.0, "system-bus-name");
        let name = String::try_from(subject.1.get("name").unwrap().try_clone().unwrap()).unwrap();
        assert!(name.starts_with(':'));
        self.callers.lock().unwrap().push(name);
        let mode = if action_id.ends_with(".Id") {
            0
        } else {
            self.mode.load(Ordering::SeqCst)
        };
        AuthorizationResult(mode == 0, mode == 2, HashMap::new())
    }
}

#[tokio::test]
async fn private_bus_exercises_serialization_authorization_notifications_and_two_clients() {
    use crate::dbus::interface::source::led::SourceLedInterface;
    use crate::udev::device::UdevDevice;
    use futures::StreamExt;
    use tokio::io::{AsyncBufReadExt, BufReader};
    use zbus::{connection::Builder, fdo::PropertiesProxy, Proxy};
    // This test owns its daemon and explicitly addresses it. It never connects
    // to the user's session bus, host system bus, real Polkit, or LED hardware.
    let mut daemon = tokio::process::Command::new("dbus-daemon")
        .args(["--session", "--nofork", "--print-address=1"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let mut address = String::new();
    tokio::time::timeout(
        Duration::from_secs(3),
        BufReader::new(daemon.stdout.take().unwrap()).read_line(&mut address),
    )
    .await
    .unwrap()
    .unwrap();
    let address = address.trim();
    let authority = TestAuthority {
        mode: Arc::new(AtomicUsize::new(0)),
        callers: Arc::new(Mutex::new(vec![])),
    };
    let _polkit = Builder::address(address)
        .unwrap()
        .name("org.freedesktop.PolicyKit1")
        .unwrap()
        .serve_at("/org/freedesktop/PolicyKit1/Authority", authority.clone())
        .unwrap()
        .build()
        .await
        .unwrap();
    let connection = Builder::address(address)
        .unwrap()
        .name("org.shadowblip.InputPlumber")
        .unwrap()
        .build()
        .await
        .unwrap();
    let (engine, _, storage, _) = setup();
    let handle = LedHandle::default();
    handle.start(engine).unwrap();
    let path = "/org/shadowblip/InputPlumber/devices/source/test_rings";
    let interface = SourceLedInterface::new(
        UdevDevice::from_devnode("/nonexistent", "test_rings"),
        handle.clone(),
    );
    interface.watch(connection.clone(), path.into());
    connection
        .object_server()
        .at("/org/shadowblip/InputPlumber", zbus::fdo::ObjectManager {})
        .await
        .unwrap();
    connection
        .object_server()
        .at(path, interface)
        .await
        .unwrap();
    let output = tokio::process::Command::new("busctl")
        .args([
            &format!("--address={address}"),
            "--json=short",
            "call",
            "org.shadowblip.InputPlumber",
            "/org/shadowblip/InputPlumber",
            "org.freedesktop.DBus.ObjectManager",
            "GetManagedObjects",
        ])
        .output()
        .await
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let objects: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    println!("BUSCTL_OBJECTS={objects}");
    let first = Builder::address(address).unwrap().build().await.unwrap();
    let second = Builder::address(address).unwrap().build().await.unwrap();
    let name = "org.shadowblip.Input.Source.LEDDevice";
    let a = Proxy::new(&first, "org.shadowblip.InputPlumber", path, name)
        .await
        .unwrap();
    let b = Proxy::new(&second, "org.shadowblip.InputPlumber", path, name)
        .await
        .unwrap();
    let caps: Capabilities = a.get_property("Capabilities").await.unwrap();
    assert_eq!(caps.api_version, 1);
    assert_eq!(caps.persistent_id, "ayaneo-3-joystick-rings");
    assert_eq!(a.get_property::<String>("Id").await.unwrap(), "test_rings");
    let properties = PropertiesProxy::builder(&first)
        .destination("org.shadowblip.InputPlumber")
        .unwrap()
        .path(path)
        .unwrap()
        .build()
        .await
        .unwrap();
    let mut changed = properties.receive_properties_changed().await.unwrap();
    let first_config = (config("solid"),);
    let second_config = (config("cycle"),);
    let (x, y): (zbus::Result<u64>, zbus::Result<u64>) = tokio::join!(
        a.call("SetConfig", &first_config),
        b.call("SetConfig", &second_config)
    );
    let (x, y) = (x.unwrap(), y.unwrap());
    assert_ne!(x, y);
    let state: LedState = properties
        .get(name.try_into().unwrap(), "State")
        .await
        .unwrap()
        .try_into()
        .unwrap();
    assert_eq!(state.revision, 2);
    assert_eq!(state.config.effect, if x > y { "solid" } else { "cycle" });
    assert_eq!(*storage.config.lock().unwrap(), Some(state.config));
    tokio::time::timeout(Duration::from_secs(3), async {
        while let Some(signal) = changed.next().await {
            if signal
                .args()
                .unwrap()
                .changed_properties()
                .contains_key("State")
            {
                return;
            }
        }
        panic!("property stream ended before State changed");
    })
    .await
    .unwrap();
    assert!(authority
        .callers
        .lock()
        .unwrap()
        .contains(&first.unique_name().unwrap().to_string()));
    assert!(authority
        .callers
        .lock()
        .unwrap()
        .contains(&second.unique_name().unwrap().to_string()));
    let invalid: zbus::Result<u64> = a
        .call(
            "SetConfig",
            &(LedConfig {
                color: vec![1, 2],
                ..config("solid")
            },),
        )
        .await;
    assert!(invalid.unwrap_err().to_string().contains("InvalidArgs"));
    for mode in [1, 2] {
        authority.mode.store(mode, Ordering::SeqCst);
        let rejected: zbus::Result<u64> = a.call("SetConfig", &(config("off"),)).await;
        let error = rejected.unwrap_err().to_string();
        assert!(
            error.contains(if mode == 2 {
                "InteractiveAuthorizationRequired"
            } else {
                "Not authorized"
            }),
            "{error}"
        );
    }
    assert_eq!(storage.writes.load(Ordering::SeqCst), 2);
    let xml: String = Proxy::new(
        &first,
        "org.shadowblip.InputPlumber",
        path,
        "org.freedesktop.DBus.Introspectable",
    )
    .await
    .unwrap()
    .call("Introspect", &())
    .await
    .unwrap();
    for signature in ["(sayuu)", "(usasuu)", "(t(sayuu)ss)"] {
        assert!(xml.contains(signature), "{xml}");
    }
    handle.set_suspended(true).await.unwrap();
    handle.stop();
}

#[tokio::test]
async fn bounded_queue_rejects_excess_work_and_stop_has_priority() {
    struct BlockingDevice {
        entered: mpsc::SyncSender<()>,
        release: mpsc::Receiver<()>,
        frames: Arc<AtomicUsize>,
    }
    impl Transport for BlockingDevice {
        fn effects(&self) -> Vec<String> {
            vec!["off".into(), "solid".into()]
        }
        fn write(&mut self, _config: &LedConfig, _color: [u8; 3], _transition: bool) -> Result<()> {
            if self.frames.fetch_add(1, Ordering::SeqCst) == 0 {
                self.entered.send(()).unwrap();
                self.release.recv_timeout(Duration::from_secs(3)).unwrap();
            }
            Ok(())
        }
    }
    let (entered_tx, entered_rx) = mpsc::sync_channel(1);
    let (release_tx, release_rx) = mpsc::sync_channel(1);
    let frames = Arc::new(AtomicUsize::new(0));
    let engine = Engine::new(
        "test-rings".into(),
        Box::new(BlockingDevice {
            entered: entered_tx,
            release: release_rx,
            frames: frames.clone(),
        }),
        Box::new(TestStorage::default()),
        Box::new(SystemClock(Instant::now())),
    );
    let handle = LedHandle::default();
    handle.start(engine).unwrap();
    tokio::task::spawn_blocking(move || entered_rx.recv_timeout(Duration::from_secs(2)).unwrap())
        .await
        .unwrap();
    let mut replies = vec![];
    for _ in 0..COMMAND_CAPACITY {
        let (tx, rx) = oneshot::channel();
        handle.send(Command::Apply(config("solid"), tx)).unwrap();
        replies.push(rx);
    }
    let (tx, _rx) = oneshot::channel();
    assert!(handle.send(Command::Apply(config("solid"), tx)).is_err());
    handle.stop();
    release_tx.send(()).unwrap();
    handle.wait_stopped().await.unwrap();
    assert_eq!(frames.load(Ordering::SeqCst), 2); // Initial frame, then orderly Off only.
    assert_eq!(handle.snapshot().state.status, "unavailable");
    for reply in replies {
        assert!(reply.await.is_err());
    }
    assert!(handle.apply(config("solid")).await.is_err());
}
#[test]
fn unavailable_sysfs_reports_failure_and_recovers_on_explicit_apply() {
    let root = temp_dir("recovery");
    let mut engine = Engine::new(
        "test-rings".into(),
        Box::new(RecoveringSysfsTransport {
            root: root.clone(),
            device: None,
            hardware_cycle_only: false,
        }),
        Box::new(TestStorage::default()),
        Box::new(TestClock::default()),
    );
    engine.apply(config("solid")).unwrap();
    engine.tick();
    assert_eq!(engine.snapshot.state.status, "failed");
    for (name, value) in [
        ("max_brightness", "255"),
        ("multi_index", "red green blue"),
        ("trigger", "[none]"),
        ("multi_intensity", "0 0 0"),
        ("brightness", "0"),
    ] {
        fs::write(root.join(name), value).unwrap();
    }
    engine.resume();
    engine.tick();
    assert_eq!(engine.snapshot.state.status, "applied");
    assert_eq!(fs::read_to_string(root.join("brightness")).unwrap(), "77");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn off_solid_and_zero_brightness_are_idle_between_commands() {
    let (mut engine, device, _, clock) = setup();
    engine.tick();
    assert_eq!(engine.next_frame_wait(), None);
    for effect in ["solid", "cycle"] {
        engine
            .apply(LedConfig {
                brightness: 0,
                ..config(effect)
            })
            .unwrap();
        clock.advance(100);
        engine.tick();
        assert_eq!(engine.next_frame_wait(), None);
    }
    let count = device.frames.lock().unwrap().len();
    clock.advance(100000);
    engine.tick();
    assert_eq!(device.frames.lock().unwrap().len(), count);
    engine.apply(config("cycle")).unwrap();
    engine.tick();
    assert_eq!(engine.next_frame_wait(), Some(FRAME_INTERVAL));
}
#[test]
fn corrupt_saved_settings_remain_reported_until_a_valid_apply() {
    struct CorruptStorage;
    impl Storage for CorruptStorage {
        fn load(&mut self) -> Result<Option<LedConfig>> {
            Err("Corrupt saved lighting".into())
        }
        fn save(&mut self, _: &LedConfig) -> Result<SaveOutcome> {
            Ok(SaveOutcome::Durable)
        }
    }
    let device = TestDevice::default();
    let mut engine = Engine::new(
        "test-rings".into(),
        Box::new(device.clone()),
        Box::new(CorruptStorage),
        Box::new(TestClock::default()),
    );
    engine.tick();
    assert!(device.frames.lock().unwrap().is_empty());
    engine.suspend().unwrap();
    engine.resume();
    engine.tick();
    assert_eq!(engine.snapshot.state.status, "failed");
    assert_eq!(engine.snapshot.state.last_error, "Corrupt saved lighting");
    engine.apply(config("solid")).unwrap();
    engine.tick();
    assert_eq!(engine.snapshot.state.status, "applied");
}
#[tokio::test]
async fn game_led_output_cannot_change_managed_saved_lighting() {
    use crate::drivers::dualsense::hid_report::SetStatePackedOutputData;
    use crate::input::{output_event::OutputEvent, source::SourceOutputDevice};
    let (engine, device, storage, _) = setup();
    let handle = LedHandle::default();
    handle.start(engine).unwrap();
    handle.apply(config("solid")).await.unwrap();
    let mut source = crate::input::source::led::multicolor::LedMultiColor::Managed(handle.clone());
    for red in 0..100 {
        source
            .write_event(OutputEvent::DualSense(SetStatePackedOutputData {
                allow_led_color: true,
                led_red: red,
                led_green: 0,
                led_blue: 0,
                ..Default::default()
            }))
            .unwrap();
    }
    assert_eq!(handle.snapshot().state.config, config("solid"));
    assert_eq!(storage.writes.load(Ordering::SeqCst), 1);
    source.stop().unwrap();
    handle.wait_stopped().await.unwrap();
    assert!(device
        .frames
        .lock()
        .unwrap()
        .iter()
        .all(|frame| frame.0.color == vec![255; 3]));
}

#[tokio::test]
async fn suspend_is_prioritized_even_when_configuration_queue_is_full() {
    struct BlockingDevice {
        entered: mpsc::SyncSender<()>,
        release: mpsc::Receiver<()>,
        frames: Arc<AtomicUsize>,
    }
    impl Transport for BlockingDevice {
        fn effects(&self) -> Vec<String> {
            vec!["off".into(), "solid".into()]
        }
        fn write(&mut self, _config: &LedConfig, _color: [u8; 3], _transition: bool) -> Result<()> {
            if self.frames.fetch_add(1, Ordering::SeqCst) == 0 {
                self.entered.send(()).unwrap();
                self.release.recv_timeout(Duration::from_secs(3)).unwrap();
            }
            Ok(())
        }
    }
    let (entered_tx, entered_rx) = mpsc::sync_channel(1);
    let (release_tx, release_rx) = mpsc::sync_channel(1);
    let frames = Arc::new(AtomicUsize::new(0));
    let engine = Engine::new(
        "test-rings".into(),
        Box::new(BlockingDevice {
            entered: entered_tx,
            release: release_rx,
            frames: frames.clone(),
        }),
        Box::new(TestStorage::default()),
        Box::new(SystemClock(Instant::now())),
    );
    let handle = LedHandle::default();
    handle.start(engine).unwrap();
    tokio::task::spawn_blocking(move || entered_rx.recv_timeout(Duration::from_secs(2)).unwrap())
        .await
        .unwrap();
    let mut replies = vec![];
    for _ in 0..COMMAND_CAPACITY {
        let (tx, rx) = oneshot::channel();
        handle.send(Command::Apply(config("solid"), tx)).unwrap();
        replies.push(rx);
    }
    let (tx, _rx) = oneshot::channel();
    assert!(handle.send(Command::Apply(config("solid"), tx)).is_err());
    let mut suspend = Box::pin(handle.set_suspended(true));
    assert!(futures::poll!(suspend.as_mut()).is_pending());
    release_tx.send(()).unwrap();
    suspend.await.unwrap();
    for reply in replies {
        reply.await.unwrap().unwrap();
    }
    assert_eq!(
        frames.load(Ordering::SeqCst),
        2,
        "Only the initial frame and priority Off are written"
    );
    assert_eq!(handle.snapshot().state.status, "unavailable");
    handle.set_suspended(false).await.unwrap();
    let mut states = handle.subscribe();
    tokio::time::timeout(
        Duration::from_secs(1),
        states.wait_for(|s| s.state.status == "applied"),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(frames.load(Ordering::SeqCst), 3);
    handle.stop();
    handle.wait_stopped().await.unwrap();
}

#[tokio::test]
async fn worker_started_during_sleep_publishes_off_without_a_queued_command() {
    let (mut engine, device, _, _) = setup();
    engine.paused = true;
    let handle = LedHandle::default();
    handle.start(engine).unwrap();
    let mut states = handle.subscribe();
    tokio::time::timeout(
        Duration::from_secs(1),
        states.wait_for(|s| s.state.status == "unavailable"),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(device.frames.lock().unwrap().len(), 1);
    assert_eq!(device.frames.lock().unwrap()[0].0.effect, "off");
    handle.stop();
    handle.wait_stopped().await.unwrap();
}

#[test]
fn directory_sync_failure_reflects_committed_file_and_stops_effects_until_retry() {
    struct FailingDirectorySync {
        storage: FileStorage,
        fail: Arc<AtomicBool>,
    }
    impl Storage for FailingDirectorySync {
        fn load(&mut self) -> Result<Option<LedConfig>> {
            self.storage.load()
        }
        fn save(&mut self, config: &LedConfig) -> Result<SaveOutcome> {
            let fail = self.fail.load(Ordering::SeqCst);
            self.storage.save_with_directory_sync(config, |directory| {
                if fail {
                    Err(std::io::Error::other("injected directory fsync failure"))
                } else {
                    directory.sync_all()
                }
            })
        }
    }
    let root = temp_dir("directory-sync");
    let fail = Arc::new(AtomicBool::new(true));
    let device = TestDevice::default();
    let mut engine = Engine::new(
        "test-rings".into(),
        Box::new(device.clone()),
        Box::new(FailingDirectorySync {
            storage: FileStorage::new(&root, "test-rings").unwrap(),
            fail: fail.clone(),
        }),
        Box::new(TestClock::default()),
    );
    let error = engine.apply(config("cycle")).unwrap_err();
    assert!(error.contains("durability is uncertain"));
    assert_eq!(
        FileStorage::new(&root, "test-rings")
            .unwrap()
            .load()
            .unwrap(),
        Some(config("cycle"))
    );
    assert_eq!(engine.snapshot.state.config, config("cycle"));
    assert_eq!(engine.snapshot.state.revision, 1);
    assert_eq!(engine.snapshot.state.status, "failed");
    engine.tick();
    engine.resume();
    engine.tick();
    assert!(device.frames.lock().unwrap().is_empty());
    fail.store(false, Ordering::SeqCst);
    assert_eq!(engine.apply(config("solid")).unwrap(), 2);
    engine.tick();
    assert_eq!(engine.snapshot.state.status, "applied");
    assert_eq!(device.frames.lock().unwrap().len(), 1);
    fs::remove_dir_all(root).unwrap();
}

#[derive(Clone)]
struct NativeTestDevice(TestDevice);
impl Transport for NativeTestDevice {
    fn effects(&self) -> Vec<String> {
        self.0.effects()
    }
    fn native_cycle(&self) -> bool {
        true
    }
    fn write(&mut self, config: &LedConfig, color: [u8; 3], transition: bool) -> Result<()> {
        self.0.write(config, color, transition)
    }
}

#[test]
fn native_cycle_sleeps_without_frames_and_resumes_saved_configuration() {
    let device = TestDevice::default();
    let storage = TestStorage::default();
    let clock = TestClock::default();
    let mut engine = Engine::new(
        "native-test".into(),
        Box::new(NativeTestDevice(device.clone())),
        Box::new(storage.clone()),
        Box::new(clock.clone()),
    );
    assert_eq!(
        (
            engine.snapshot.capabilities.cycle_min_ms,
            engine.snapshot.capabilities.cycle_max_ms
        ),
        (0, 0)
    );
    let setting = LedConfig {
        color: vec![19, 61, 127],
        brightness: 60,
        cycle_period_ms: 17000,
        ..config("cycle")
    };
    engine.apply(setting.clone()).unwrap();
    engine.tick();
    assert_eq!(engine.next_frame_wait(), None);
    for _ in 0..1200 {
        clock.advance(100);
        engine.tick();
    }
    assert_eq!(device.frames.lock().unwrap().len(), 1);
    assert_eq!(device.frames.lock().unwrap()[0].1, [19, 61, 127]);
    engine.suspend().unwrap();
    engine.resume();
    engine.tick();
    assert_eq!(engine.snapshot.state.config, setting);
    assert_eq!(*storage.config.lock().unwrap(), Some(setting));
    let frames = device.frames.lock().unwrap();
    assert_eq!(frames.len(), 3);
    assert_eq!(frames[1].0.effect, "off");
    assert_eq!(frames[2].0.effect, "cycle");
    assert_eq!(engine.next_frame_wait(), None);
}

fn native_sysfs_fixture(label: &str) -> PathBuf {
    let path = temp_dir(label);
    for (name, value) in [
        ("max_brightness", "255"),
        ("multi_index", "red green blue"),
        ("trigger", "[none] pattern"),
        ("brightness", "0"),
        ("multi_intensity", "17 34 51"),
        ("hw_pattern", ""),
        ("effect_index", "none rainbow"),
        ("effect", "none"),
    ] {
        fs::write(path.join(name), value).unwrap();
    }
    path
}

#[test]
fn native_sysfs_transitions_clear_the_effect_and_preserve_remembered_colour() {
    for from in ["off", "solid", "breathing", "cycle"] {
        for to in ["off", "solid", "breathing", "cycle"] {
            let root = native_sysfs_fixture("native-transition");
            let mut transport = SysfsTransport::open(root.clone()).unwrap();
            transport.write(&config(from), [17, 34, 51], true).unwrap();
            let setting = LedConfig {
                brightness: 60,
                ..config(to)
            };
            transport.write(&setting, [17, 34, 51], true).unwrap();
            let read = |name| fs::read_to_string(root.join(name)).unwrap();
            assert_eq!(
                read("effect"),
                if to == "cycle" { "rainbow" } else { "none" }
            );
            assert_eq!(read("multi_intensity"), "17 34 51");
            if to == "off" {
                assert_eq!(read("brightness"), "0");
            } else if to == "breathing" {
                assert_eq!(read("trigger"), "pattern");
                assert_eq!(read("hw_pattern"), "0 1000 153 1000");
            } else {
                assert_eq!(read("brightness"), "153");
            }
            let zero = LedConfig {
                brightness: 0,
                ..config("cycle")
            };
            transport.write(&zero, [255; 3], true).unwrap();
            assert_eq!(read("brightness"), "0");
            assert_eq!(read("effect"), "none");
            fs::remove_dir_all(root).unwrap();
        }
    }
}

#[test]
fn hardware_only_profile_never_falls_back_when_native_effect_disappears() {
    let root = native_sysfs_fixture("native-disappeared");
    let clock = TestClock::default();
    let storage = TestStorage::default();
    let mut engine = Engine::new(
        "native-required".into(),
        Box::new(RecoveringSysfsTransport {
            root: root.clone(),
            device: None,
            hardware_cycle_only: true,
        }),
        Box::new(storage.clone()),
        Box::new(clock.clone()),
    );
    let setting = config("cycle");
    engine.apply(setting.clone()).unwrap();
    engine.tick();
    assert_eq!(engine.snapshot.state.status, "applied");
    fs::remove_file(root.join("effect")).unwrap();
    // No attribute polling or colour writes are needed to keep firmware running.
    clock.advance(120_000);
    engine.tick();
    assert_eq!(engine.snapshot.state.status, "applied");
    engine.resume();
    assert_eq!(engine.snapshot.state.status, "failed");
    assert!(!engine
        .snapshot
        .capabilities
        .effects
        .contains(&"cycle".into()));
    assert!(engine.apply(setting.clone()).is_err());
    assert_eq!(*storage.config.lock().unwrap(), Some(setting));
    assert_eq!(
        fs::read_to_string(root.join("multi_intensity")).unwrap(),
        "17 34 51"
    );
    assert!(!root.join("effect").exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn native_capability_requires_clear_and_select_and_legacy_policy_is_explicit() {
    let root = native_sysfs_fixture("native-detection");
    for modes in ["rainbow", "none", "static rainbow", "none rainbows", ""] {
        fs::write(root.join("effect_index"), modes).unwrap();
        let required = RecoveringSysfsTransport {
            root: root.clone(),
            device: None,
            hardware_cycle_only: true,
        };
        assert!(!required.native_cycle());
        assert!(!required.effects().contains(&"cycle".into()));
        let software = RecoveringSysfsTransport {
            root: root.clone(),
            device: None,
            hardware_cycle_only: false,
        };
        assert!(software.effects().contains(&"cycle".into()));
    }
    fs::write(root.join("effect_index"), "none rainbow").unwrap();
    let mut transport = RecoveringSysfsTransport {
        root: root.clone(),
        device: None,
        hardware_cycle_only: true,
    };
    assert!(transport.native_cycle());
    assert!(transport.effects().contains(&"cycle".into()));
    fs::remove_file(root.join("effect")).unwrap();
    assert!(transport.write(&config("cycle"), [255; 3], true).is_err());
    assert_eq!(fs::read_to_string(root.join("brightness")).unwrap(), "0");
    fs::remove_dir_all(root).unwrap();
}

struct DropGatedTransport {
    device: TestDevice,
    entered: Option<oneshot::Sender<()>>,
    release: mpsc::Receiver<()>,
    released: Arc<AtomicBool>,
}
impl Transport for DropGatedTransport {
    fn effects(&self) -> Vec<String> {
        self.device.effects()
    }
    fn write(&mut self, config: &LedConfig, color: [u8; 3], transition: bool) -> Result<()> {
        self.device.write(config, color, transition)
    }
}
impl Drop for DropGatedTransport {
    fn drop(&mut self) {
        if let Some(sender) = self.entered.take() {
            let _ = sender.send(());
        }
        // Dropping the test's sender also releases the worker after an assertion failure.
        let _ = self.release.recv();
        self.released.store(true, Ordering::SeqCst);
    }
}
fn drop_gated_engine() -> (
    Engine,
    oneshot::Receiver<()>,
    mpsc::Sender<()>,
    Arc<AtomicBool>,
) {
    let (entered, wait) = oneshot::channel();
    let (release, receiver) = mpsc::channel();
    let released = Arc::new(AtomicBool::new(false));
    let engine = Engine::new(
        "retiring-rings".into(),
        Box::new(DropGatedTransport {
            device: TestDevice::default(),
            entered: Some(entered),
            release: receiver,
            released: released.clone(),
        }),
        Box::new(TestStorage::default()),
        Box::new(TestClock::default()),
    );
    (engine, wait, release, released)
}

#[tokio::test]
async fn worker_completion_waits_for_transport_cleanup() {
    let handle = LedHandle::default();
    let (engine, entered, release, released) = drop_gated_engine();
    handle.start(engine).unwrap();
    handle.stop();
    tokio::time::timeout(Duration::from_secs(3), entered)
        .await
        .expect("worker did not reach transport cleanup")
        .unwrap();
    assert!(
        tokio::time::timeout(Duration::from_millis(50), handle.wait_stopped())
            .await
            .is_err(),
        "completion must wait for transport resource release"
    );
    release.send(()).unwrap();
    handle.wait_stopped().await.unwrap();
    assert!(released.load(Ordering::SeqCst));
}

#[tokio::test]
async fn registry_removal_retires_the_old_handle_before_allowing_reconnect() {
    let registry = LedRegistry::default();
    let old = registry.get("leds://retiring-rings");
    let (engine, entered, release, released) = drop_gated_engine();
    old.start(engine).unwrap();
    let removing_registry = registry.clone();
    let removal =
        tokio::spawn(async move { removing_registry.remove("leds://retiring-rings").await });
    tokio::time::timeout(Duration::from_secs(3), entered)
        .await
        .expect("removal did not reach transport cleanup")
        .unwrap();
    assert!(!removal.is_finished());
    let during_removal = registry.get("leds://retiring-rings");
    assert!(during_removal.start(setup().0).is_err());
    release.send(()).unwrap();
    removal.await.unwrap().unwrap();
    assert!(released.load(Ordering::SeqCst));
    assert!(
        old.start(setup().0).is_err(),
        "a retired handle cannot become an owner again"
    );
    let replacement = registry.get("leds://retiring-rings");
    replacement.start(setup().0).unwrap();
    old.stop();
    replacement.apply(config("solid")).await.unwrap();
    registry.remove("leds://retiring-rings").await.unwrap();
    replacement.wait_stopped().await.unwrap();
}

#[tokio::test]
async fn timed_out_registry_removal_keeps_the_retiring_owner_blocked() {
    let registry = LedRegistry::default();
    let old = registry.get("leds://blocked-rings");
    let (engine, entered, release, _) = drop_gated_engine();
    old.start(engine).unwrap();
    let removing_registry = registry.clone();
    let removal =
        tokio::spawn(async move { removing_registry.remove("leds://blocked-rings").await });
    entered.await.unwrap();
    let error = removal.await.unwrap().unwrap_err();
    assert!(error.contains("did not stop"));
    assert_eq!(old.snapshot().state.status, "failed");
    assert_eq!(old.snapshot().state.last_error, error);
    assert!(registry
        .get("leds://blocked-rings")
        .start(setup().0)
        .is_err());
    release.send(()).unwrap();
    old.wait_stopped().await.unwrap();
    registry.remove("leds://blocked-rings").await.unwrap();
    let replacement = registry.get("leds://blocked-rings");
    replacement.start(setup().0).unwrap();
    registry.remove("leds://blocked-rings").await.unwrap();
}
