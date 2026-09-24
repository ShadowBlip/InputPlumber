//! Persistent multicolour lighting, independent of frontend and input polling lifetimes.
//! Only explicitly opted-in profiles use this worker. No game output enters it.
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::{
    collections::HashMap,
    fs::{self, File, OpenOptions},
    io::Write,
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
    sync::{mpsc, Arc, Mutex},
    thread,
    time::{Duration, Instant},
};
use tokio::sync::{oneshot, watch};
use zbus::zvariant::{OwnedValue, Type, Value};

pub const FRAME_INTERVAL: Duration = Duration::from_millis(100);
const COMMAND_CAPACITY: usize = 8;
const CALL_TIMEOUT: Duration = Duration::from_secs(3);
pub type Result<T> = std::result::Result<T, String>;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type, Value, OwnedValue)]
#[serde(deny_unknown_fields)]
pub struct LedConfig {
    pub effect: String,
    pub color: Vec<u8>,
    pub brightness: u32,
    pub cycle_period_ms: u32,
}
impl Default for LedConfig {
    fn default() -> Self {
        Self {
            effect: "off".into(),
            color: vec![255; 3],
            brightness: 30,
            cycle_period_ms: 8000,
        }
    }
}
impl LedConfig {
    pub fn validate(&self, effects: &[String]) -> Result<()> {
        if !effects.contains(&self.effect) {
            return Err("Unsupported lighting effect".into());
        }
        if self.color.len() != 3 {
            return Err("Color must contain exactly three RGB bytes".into());
        }
        if self.brightness > 100 {
            return Err("Brightness must be between 0 and 100 percent".into());
        }
        if !(2000..=30000).contains(&self.cycle_period_ms) {
            return Err("Cycle period must be between 2000 and 30000 milliseconds".into());
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type, Value, OwnedValue)]
pub struct Capabilities {
    pub api_version: u32,
    pub persistent_id: String,
    pub effects: Vec<String>,
    pub cycle_min_ms: u32,
    pub cycle_max_ms: u32,
}
impl Default for Capabilities {
    fn default() -> Self {
        Self {
            api_version: 1,
            persistent_id: String::new(),
            effects: vec![],
            cycle_min_ms: 2000,
            cycle_max_ms: 30000,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type, Value, OwnedValue)]
pub struct LedState {
    pub revision: u64,
    pub config: LedConfig,
    pub status: String,
    pub last_error: String,
}
impl Default for LedState {
    fn default() -> Self {
        Self {
            revision: 0,
            config: LedConfig::default(),
            status: "unavailable".into(),
            last_error: String::new(),
        }
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Snapshot {
    pub capabilities: Capabilities,
    pub state: LedState,
}

/// Hardware boundary. All methods execute on one dedicated thread per LED.
pub trait Transport: Send {
    fn effects(&self) -> Vec<String>;
    /// True when cycle timing is owned by the device firmware.
    fn native_cycle(&self) -> bool {
        false
    }
    fn write(&mut self, config: &LedConfig, color: [u8; 3], transition: bool) -> Result<()>;
}
#[derive(Debug, PartialEq, Eq)]
pub enum SaveOutcome {
    Durable,
    /// Atomic replacement completed, but crash durability could not be confirmed.
    CommittedUncertain(String),
}
pub trait Storage: Send {
    fn load(&mut self) -> Result<Option<LedConfig>>;
    fn save(&mut self, config: &LedConfig) -> Result<SaveOutcome>;
}
/// Monotonic clock boundary; skipped deadlines are never replayed.
pub trait Clock: Send {
    fn now(&self) -> Duration;
}
struct SystemClock(Instant);
impl Clock for SystemClock {
    fn now(&self) -> Duration {
        self.0.elapsed()
    }
}

pub struct FileStorage {
    path: PathBuf,
}
impl FileStorage {
    pub fn new(directory: &Path, identity: &str) -> Result<Self> {
        if identity.is_empty()
            || identity.len() > 128
            || !identity
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            return Err("Invalid persistent lighting identity".into());
        }
        Ok(Self {
            path: directory.join(format!("{identity}.json")),
        })
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SavedConfig {
    version: u32,
    config: LedConfig,
}
impl Storage for FileStorage {
    fn load(&mut self) -> Result<Option<LedConfig>> {
        let data = match fs::read(&self.path) {
            Ok(data) => data,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(format!("Cannot read saved lighting: {e}")),
        };
        let saved: SavedConfig =
            serde_json::from_slice(&data).map_err(|e| format!("Invalid saved lighting: {e}"))?;
        if saved.version != 1 {
            return Err("Unsupported saved lighting version".into());
        }
        Ok(Some(saved.config))
    }
    fn save(&mut self, config: &LedConfig) -> Result<SaveOutcome> {
        self.save_with_directory_sync(config, |directory| directory.sync_all())
    }
}
impl FileStorage {
    fn save_with_directory_sync(
        &mut self,
        config: &LedConfig,
        sync_directory: impl FnOnce(&File) -> std::io::Result<()>,
    ) -> Result<SaveOutcome> {
        let parent = self.path.parent().ok_or("Missing state directory")?;
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        // Open before replacement, so inability to open the directory is a
        // pre-commit failure and leaves the old file/configuration unchanged.
        let directory =
            File::open(parent).map_err(|e| format!("Cannot open lighting state directory: {e}"))?;
        // Each device identity has exactly one owner. create_new also detects stale
        // temporary files instead of following a link or truncating arbitrary data.
        static NEXT_TEMP: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let temp = self.path.with_extension(format!(
            "{}-{}.tmp",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        let data = serde_json::to_vec(&SavedConfig {
            version: 1,
            config: config.clone(),
        })
        .map_err(|e| e.to_string())?;
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temp)
            .map_err(|e| format!("Cannot create lighting state: {e}"))?;
        let result = (|| -> std::io::Result<()> {
            file.write_all(&data)?;
            file.sync_all()?;
            fs::rename(&temp, &self.path)
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temp);
        }
        result.map_err(|e| format!("Cannot persist lighting: {e}"))?;
        match sync_directory(&directory) {
            Ok(()) => Ok(SaveOutcome::Durable),
            Err(error) => Ok(SaveOutcome::CommittedUncertain(format!(
                "Lighting state was replaced, but directory synchronization failed ({error}); crash durability is uncertain. Apply again to retry."
            ))),
        }
    }
}

/// Deterministic effect/state machine, driven by a worker or fake clock in tests.
pub struct Engine {
    pub snapshot: Snapshot,
    transport: Box<dyn Transport>,
    storage: Box<dyn Storage>,
    clock: Box<dyn Clock>,
    last_frame: Option<Duration>,
    epoch: Duration,
    paused: bool,
    dirty: bool,
    restore_error: Option<String>,
}
impl Engine {
    pub fn new(
        identity: String,
        transport: Box<dyn Transport>,
        mut storage: Box<dyn Storage>,
        clock: Box<dyn Clock>,
    ) -> Self {
        let effects = transport.effects();
        let cycle_range = if transport.native_cycle() {
            (0, 0)
        } else {
            (2000, 30000)
        };
        let mut state = LedState {
            status: "pending".into(),
            ..LedState::default()
        };
        let mut restore_error = None;
        match storage.load() {
            Ok(Some(config)) => {
                let all_effects = ["off", "solid", "breathing", "cycle"].map(str::to_owned);
                match config.validate(&all_effects) {
                    Ok(()) => {
                        state.config = config;
                        if let Err(e) = state.config.validate(&effects) {
                            state.status = "failed".into();
                            state.last_error = e;
                        }
                    }
                    Err(e) => {
                        restore_error = Some(e.clone());
                        state.status = "failed".into();
                        state.last_error = e;
                    }
                }
            }
            Ok(None) => (),
            Err(e) => {
                restore_error = Some(e.clone());
                state.status = "failed".into();
                state.last_error = e;
            }
        }
        let epoch = clock.now();
        Self {
            snapshot: Snapshot {
                capabilities: Capabilities {
                    persistent_id: identity,
                    effects,
                    cycle_min_ms: cycle_range.0,
                    cycle_max_ms: cycle_range.1,
                    ..Capabilities::default()
                },
                state,
            },
            transport,
            storage,
            clock,
            epoch,
            last_frame: None,
            paused: false,
            dirty: true,
            restore_error,
        }
    }
    pub fn apply(&mut self, config: LedConfig) -> Result<u64> {
        config.validate(&self.snapshot.capabilities.effects)?;
        let revision = self
            .snapshot
            .state
            .revision
            .checked_add(1)
            .ok_or("Revision exhausted")?;
        let saved = self.storage.save(&config)?; // Commit before scheduling any hardware write.
        self.snapshot.state.revision = revision;
        self.restore_error = None;
        self.snapshot.state.config = config;
        if let SaveOutcome::CommittedUncertain(error) = saved {
            // The visible file changed. Reflect that exact configuration rather
            // than falsely claiming the previous settings survived this error.
            self.snapshot.state.status = "failed".into();
            self.snapshot.state.last_error = error.clone();
            self.restore_error = Some(error.clone());
            self.dirty = false;
            return Err(error);
        }
        self.snapshot.state.status = if self.paused {
            "unavailable"
        } else {
            "pending"
        }
        .into();
        self.snapshot.state.last_error.clear();
        self.epoch = self.clock.now();
        self.dirty = true;
        Ok(self.snapshot.state.revision)
    }
    fn software_cycle(&self) -> bool {
        self.snapshot.state.config.effect == "cycle"
            && self.snapshot.state.config.brightness != 0
            && self.snapshot.capabilities.cycle_min_ms != 0
    }
    fn next_frame_wait(&self) -> Option<Duration> {
        if self.paused
            || self.snapshot.state.status == "failed"
            || (!self.dirty && !self.software_cycle())
        {
            return None;
        }
        Some(
            self.last_frame
                .map(|last| FRAME_INTERVAL.saturating_sub(self.clock.now().saturating_sub(last)))
                .unwrap_or_default(),
        )
    }
    pub fn tick(&mut self) {
        if self.paused || self.snapshot.state.status == "failed" {
            return;
        }
        let now = self.clock.now();
        if self
            .last_frame
            .is_some_and(|last| now.saturating_sub(last) < FRAME_INTERVAL)
        {
            return;
        }
        let config = &self.snapshot.state.config;
        if !self.dirty && !self.software_cycle() {
            return;
        }
        let color = if self.software_cycle() {
            cycle_color(now.saturating_sub(self.epoch), config.cycle_period_ms)
        } else {
            [config.color[0], config.color[1], config.color[2]]
        };
        self.last_frame = Some(now);
        match self.transport.write(config, color, self.dirty) {
            Ok(()) => {
                self.snapshot.state.status = "applied".into();
                self.snapshot.state.last_error.clear();
                self.dirty = false;
            }
            Err(e) => {
                self.snapshot.state.status = "failed".into();
                self.snapshot.state.last_error = e;
            }
        }
    }
    pub fn suspend(&mut self) -> Result<()> {
        self.paused = true;
        let result = self.transport.write(&LedConfig::default(), [0; 3], true);
        self.snapshot.state.status = if result.is_ok() {
            "unavailable"
        } else {
            "failed"
        }
        .into();
        self.snapshot.state.last_error = result.as_ref().err().cloned().unwrap_or_default();
        result
    }
    pub fn resume(&mut self) {
        self.paused = false;
        if let Some(error) = self.restore_error.as_ref() {
            self.snapshot.state.status = "failed".into();
            self.snapshot.state.last_error = error.clone();
            return;
        }
        self.snapshot.capabilities.effects = self.transport.effects();
        let (minimum, maximum) = if self.transport.native_cycle() {
            (0, 0)
        } else {
            (2000, 30000)
        };
        self.snapshot.capabilities.cycle_min_ms = minimum;
        self.snapshot.capabilities.cycle_max_ms = maximum;
        if let Err(error) = self
            .snapshot
            .state
            .config
            .validate(&self.snapshot.capabilities.effects)
        {
            self.snapshot.state.status = "failed".into();
            self.snapshot.state.last_error = error;
            return;
        }
        self.dirty = true;
        self.last_frame = None;
        self.epoch = self.clock.now();
        self.snapshot.state.status = "pending".into();
        self.snapshot.state.last_error.clear();
    }
}

/// Integer RGB wheel; brightness remains a separate hardware scalar.
pub fn cycle_color(elapsed: Duration, period_ms: u32) -> [u8; 3] {
    let phase =
        ((elapsed.as_millis() % u128::from(period_ms)) * 1536 / u128::from(period_ms)) as u32;
    let step = (phase % 256) as u8;
    match phase / 256 {
        0 => [255, step, 0],
        1 => [255 - step, 255, 0],
        2 => [0, 255, step],
        3 => [0, 255 - step, 255],
        4 => [step, 0, 255],
        _ => [255, 0, 255 - step],
    }
}

#[derive(Debug)]
enum Command {
    Apply(LedConfig, oneshot::Sender<Result<u64>>),
    Wake,
    Stop,
}
#[derive(Clone, Copy, Debug, Default)]
struct Lifecycle {
    revision: u64,
    suspended: bool,
}
#[derive(Clone, Debug, Default)]
struct LifecycleAck {
    revision: u64,
    suspended: bool,
    error: Option<String>,
}

fn apply_lifecycle(
    engine: &mut Engine,
    desired: &Mutex<Lifecycle>,
    applied: &mut u64,
    ack: &watch::Sender<LifecycleAck>,
    snapshot: &watch::Sender<Snapshot>,
) {
    let desired = *desired.lock().unwrap();
    if desired.revision == *applied {
        return;
    }
    let error = if desired.suspended {
        engine.suspend().err()
    } else {
        engine.resume();
        (engine.snapshot.state.status == "failed").then(|| engine.snapshot.state.last_error.clone())
    };
    *applied = desired.revision;
    snapshot.send_replace(engine.snapshot.clone());
    ack.send_replace(LifecycleAck {
        revision: desired.revision,
        suspended: desired.suspended,
        error,
    });
}

#[derive(Clone, Debug)]
pub struct LedHandle {
    snapshot: watch::Sender<Snapshot>,
    commands: Arc<Mutex<Option<mpsc::SyncSender<Command>>>>,
    finished: watch::Sender<bool>,
    stopping: Arc<AtomicBool>,
    lifecycle: Arc<Mutex<Lifecycle>>,
    lifecycle_ack: watch::Sender<LifecycleAck>,
}
impl Default for LedHandle {
    fn default() -> Self {
        let (snapshot, _) = watch::channel(Snapshot::default());
        Self {
            snapshot,
            commands: Arc::new(Mutex::new(None)),
            finished: watch::channel(true).0,
            stopping: Arc::new(AtomicBool::new(false)),
            lifecycle: Arc::new(Mutex::new(Lifecycle::default())),
            lifecycle_ack: watch::channel(LifecycleAck::default()).0,
        }
    }
}
impl LedHandle {
    pub fn fail(&self, error: String) {
        self.snapshot.send_modify(|snapshot| {
            snapshot.state.status = "failed".into();
            snapshot.state.last_error = error;
        });
    }
    pub fn snapshot(&self) -> Snapshot {
        self.snapshot.borrow().clone()
    }
    pub fn subscribe(&self) -> watch::Receiver<Snapshot> {
        self.snapshot.subscribe()
    }
    pub fn start(&self, engine: Engine) -> Result<()> {
        let mut commands = self.commands.lock().map_err(|_| "Lighting lock poisoned")?;
        if commands.is_some() || !*self.finished.borrow() {
            return Err("Lighting source already has an owner".into());
        }
        let (tx, rx) = mpsc::sync_channel(COMMAND_CAPACITY);
        let snapshot = self.snapshot.clone();
        let initial = engine.snapshot.clone();
        self.snapshot.send_replace(initial);
        self.finished.send_replace(false);
        self.stopping.store(false, Ordering::SeqCst);
        let finished = self.finished.clone();
        let stopping = self.stopping.clone();
        let lifecycle = self.lifecycle.clone();
        *lifecycle.lock().unwrap() = Lifecycle {
            revision: 0,
            suspended: engine.paused,
        };
        let lifecycle_ack = self.lifecycle_ack.clone();
        lifecycle_ack.send_replace(LifecycleAck {
            revision: 0,
            suspended: engine.paused,
            error: None,
        });
        thread::Builder::new()
            .name("inputplumber-led".into())
            .spawn(move || {
                let mut engine = engine;
                let mut lifecycle_revision = 0;
                if engine.paused {
                    let error = engine.suspend().err();
                    snapshot.send_replace(engine.snapshot.clone());
                    lifecycle_ack.send_replace(LifecycleAck {
                        revision: 0,
                        suspended: true,
                        error,
                    });
                }
                loop {
                    let command = match engine.next_frame_wait() {
                        Some(wait) => rx.recv_timeout(wait),
                        None => rx.recv().map_err(|_| mpsc::RecvTimeoutError::Disconnected),
                    };
                    // Stop takes priority even when the configuration queue is full.
                    if stopping.load(Ordering::SeqCst) {
                        let _ = engine.suspend();
                        break;
                    }
                    apply_lifecycle(
                        &mut engine,
                        &lifecycle,
                        &mut lifecycle_revision,
                        &lifecycle_ack,
                        &snapshot,
                    );
                    match command {
                        Ok(Command::Apply(config, reply)) => {
                            if !reply.is_closed() {
                                let result = engine.apply(config);
                                snapshot.send_replace(engine.snapshot.clone());
                                let _ = reply.send(result);
                            }
                        }
                        Ok(Command::Wake) => (),
                        Ok(Command::Stop) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                            let _ = engine.suspend();
                            break;
                        }
                        Err(mpsc::RecvTimeoutError::Timeout) => (),
                    }
                    apply_lifecycle(
                        &mut engine,
                        &lifecycle,
                        &mut lifecycle_revision,
                        &lifecycle_ack,
                        &snapshot,
                    );
                    // No queued frames: compute only the frame needed now.
                    engine.tick();
                    snapshot.send_if_modified(|old| {
                        if *old == engine.snapshot {
                            false
                        } else {
                            *old = engine.snapshot.clone();
                            true
                        }
                    });
                }
                snapshot.send_replace(engine.snapshot);
                finished.send_replace(true);
            })
            .map_err(|e| {
                self.finished.send_replace(true);
                e.to_string()
            })?;
        *commands = Some(tx);
        Ok(())
    }
    fn send(&self, command: Command) -> Result<()> {
        let commands = self.commands.lock().map_err(|_| "Lighting lock poisoned")?;
        commands
            .as_ref()
            .ok_or("Lighting device unavailable")?
            .try_send(command)
            .map_err(|e| format!("Lighting worker unavailable or busy: {e}"))
    }
    pub async fn apply(&self, config: LedConfig) -> Result<u64> {
        config.validate(&self.snapshot().capabilities.effects)?;
        let (tx, rx) = oneshot::channel();
        self.send(Command::Apply(config, tx))?;
        tokio::time::timeout(CALL_TIMEOUT, rx)
            .await
            .map_err(|_| "Lighting request timed out; read state before retrying")?
            .map_err(|_| "Lighting worker stopped")?
    }
    pub async fn set_suspended(&self, suspended: bool) -> Result<()> {
        if self.snapshot().capabilities.persistent_id.is_empty() {
            return Ok(());
        }
        if self.stopping.load(Ordering::SeqCst) {
            return self.wait_stopped().await;
        }
        let revision = {
            let commands = self.commands.lock().map_err(|_| "Lighting lock poisoned")?;
            let sender = commands.as_ref().ok_or("Lighting device unavailable")?;
            let mut desired = self
                .lifecycle
                .lock()
                .map_err(|_| "Lighting lifecycle lock poisoned")?;
            desired.revision = desired
                .revision
                .checked_add(1)
                .ok_or("Lighting lifecycle revision exhausted")?;
            desired.suspended = suspended;
            // A full queue already guarantees a wake-up. Lifecycle state is
            // separate and applied before queued configurations and frames.
            if matches!(
                sender.try_send(Command::Wake),
                Err(mpsc::TrySendError::Disconnected(_))
            ) {
                return Err("Lighting worker stopped".into());
            }
            desired.revision
        };
        let mut ack = self.lifecycle_ack.subscribe();
        tokio::time::timeout(CALL_TIMEOUT, async {
            loop {
                let value = ack.borrow_and_update().clone();
                if value.revision >= revision {
                    if value.suspended != suspended {
                        return Err("Lighting lifecycle request superseded".into());
                    }
                    return value.error.map_or(Ok(()), Err);
                }
                ack.changed().await.map_err(|_| "Lighting worker stopped")?;
            }
        })
        .await
        .map_err(|_| "Lighting lifecycle request timed out")?
    }

    pub async fn wait_stopped(&self) -> Result<()> {
        let mut done = self.finished.subscribe();
        tokio::time::timeout(CALL_TIMEOUT, async {
            while !*done.borrow_and_update() {
                done.changed()
                    .await
                    .map_err(|_| "Lighting worker completion channel closed")?;
            }
            Ok(())
        })
        .await
        .map_err(|_| "Lighting worker did not stop before timeout")?
    }
    pub fn stop(&self) {
        self.stopping.store(true, Ordering::SeqCst);
        if let Ok(mut commands) = self.commands.lock() {
            // Dropping the last sender also makes the worker turn off if its queue is full.
            if let Some(tx) = commands.take() {
                let _ = tx.try_send(Command::Stop);
            }
        }
    }
}
#[derive(Clone, Debug, Default)]
pub struct LedRegistry(Arc<Mutex<HashMap<String, LedHandle>>>, Arc<AtomicBool>);
impl LedRegistry {
    pub fn is_suspended(&self) -> bool {
        self.1.load(Ordering::SeqCst)
    }
    pub fn get(&self, id: &str) -> LedHandle {
        self.0.lock().unwrap().entry(id.into()).or_default().clone()
    }
    pub fn remove(&self, id: &str) {
        if let Some(handle) = self.0.lock().unwrap().remove(id) {
            handle.stop();
        }
    }
    pub async fn set_suspended(&self, suspended: bool) {
        self.1.store(suspended, Ordering::SeqCst);
        let handles: Vec<_> = self.0.lock().unwrap().values().cloned().collect();
        for handle in handles {
            if let Err(e) = handle.set_suspended(suspended).await {
                log::warn!("LED lifecycle: {e}");
            }
        }
    }
}

/// sysfs implementation of the standard Linux multicolour and pattern-trigger ABI.
pub struct SysfsTransport {
    root: PathBuf,
    maximum: u32,
    channels: Vec<String>,
    breathing: bool,
    native_cycle: bool,
}
/// Only opt into the effect ABI when both selection and explicit clearing exist.
fn native_cycle_supported(root: &Path) -> bool {
    root.join("effect").is_file()
        && fs::read_to_string(root.join("effect_index")).is_ok_and(|modes| {
            let modes: Vec<_> = modes.split_whitespace().collect();
            modes.contains(&"none") && modes.contains(&"rainbow")
        })
}
impl SysfsTransport {
    pub fn open(root: PathBuf) -> Result<Self> {
        let maximum: u32 = fs::read_to_string(root.join("max_brightness"))
            .map_err(|e| e.to_string())?
            .trim()
            .parse()
            .map_err(|_| "Invalid max_brightness")?;
        if maximum == 0 {
            return Err("Zero max_brightness".into());
        }
        let channels: Vec<_> = fs::read_to_string(root.join("multi_index"))
            .map_err(|e| e.to_string())?
            .split_whitespace()
            .map(str::to_string)
            .collect();
        if channels.len() != 3
            || ["red", "green", "blue"]
                .iter()
                .any(|c| channels.iter().filter(|s| s.as_str() == *c).count() != 1)
        {
            return Err("Managed lighting requires exactly red, green, and blue channels".into());
        }
        let breathing = fs::read_to_string(root.join("trigger"))
            .unwrap_or_default()
            .split_whitespace()
            .any(|s| s.trim_matches(['[', ']']) == "pattern");
        let native_cycle = native_cycle_supported(&root);
        Ok(Self {
            root,
            maximum,
            channels,
            breathing,
            native_cycle,
        })
    }
    fn write_attribute(&self, name: &str, value: &str) -> Result<()> {
        // write() never creates an attribute; missing hardware must fail visibly.
        let mut file = OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(self.root.join(name))
            .map_err(|e| format!("Cannot open LED {name}: {e}"))?;
        file.write_all(value.as_bytes())
            .map_err(|e| format!("Cannot write LED {name}: {e}"))
    }
}
impl Transport for SysfsTransport {
    fn native_cycle(&self) -> bool {
        self.native_cycle
    }
    fn effects(&self) -> Vec<String> {
        let mut effects = vec!["off".into(), "solid".into(), "cycle".into()];
        if self.breathing {
            effects.push("breathing".into());
        }
        effects
    }
    fn write(&mut self, config: &LedConfig, color: [u8; 3], transition: bool) -> Result<()> {
        // Clear old triggers before any effect transition, including Off.
        if transition {
            self.write_attribute("trigger", "none")?;
        }
        if config.effect == "off" || config.brightness == 0 {
            // Blank before clearing a native effect, so Off cannot flash the
            // remembered static colour.
            self.write_attribute("brightness", "0")?;
            if self.native_cycle {
                self.write_attribute("effect", "none")?;
            }
            return Ok(());
        }
        let brightness =
            ((u64::from(config.brightness) * u64::from(self.maximum) + 50) / 100).to_string();
        if self.native_cycle && config.effect == "cycle" {
            if transition {
                self.write_attribute("brightness", &brightness)?;
                self.write_attribute("effect", "rainbow")?;
            }
            return Ok(());
        }
        if self.native_cycle && transition {
            // Clear the hardware mode before selecting the pattern trigger.
            self.write_attribute("effect", "none")?;
        }
        let values = self
            .channels
            .iter()
            .map(|channel| {
                let index = match channel.as_str() {
                    "red" => 0,
                    "green" => 1,
                    _ => 2,
                };
                ((u64::from(color[index]) * u64::from(self.maximum) + 127) / 255).to_string()
            })
            .collect::<Vec<_>>()
            .join(" ");
        self.write_attribute("multi_intensity", &values)?;
        if config.effect == "breathing" {
            // hw_pattern appears only after selecting the pattern trigger. Firmware
            // fixes its tempo; do not expose these placeholder durations as speed.
            self.write_attribute("trigger", "pattern")?;
            self.write_attribute("hw_pattern", &format!("0 1000 {brightness} 1000"))
        } else if transition {
            self.write_attribute("brightness", &brightness)
        } else {
            // Cycle frames change hue only; brightness was set at the transition.
            Ok(())
        }
    }
}

/// Reopen failed sysfs resources only when explicitly applying/restoring state.
struct RecoveringSysfsTransport {
    root: PathBuf,
    device: Option<SysfsTransport>,
    hardware_cycle_only: bool,
}
impl Transport for RecoveringSysfsTransport {
    fn native_cycle(&self) -> bool {
        native_cycle_supported(&self.root)
    }
    fn effects(&self) -> Vec<String> {
        let mut effects = vec!["off".into(), "solid".into()];
        if !self.hardware_cycle_only || self.native_cycle() {
            effects.push("cycle".into());
        }
        if fs::read_to_string(self.root.join("trigger"))
            .unwrap_or_default()
            .split_whitespace()
            .any(|s| s.trim_matches(['[', ']']) == "pattern")
        {
            effects.push("breathing".into());
        }
        effects
    }
    fn write(&mut self, config: &LedConfig, color: [u8; 3], transition: bool) -> Result<()> {
        if transition || self.device.is_none() {
            self.device = Some(SysfsTransport::open(self.root.clone())?);
        }
        let device = self
            .device
            .as_mut()
            .ok_or("Lighting transport unavailable")?;
        if self.hardware_cycle_only && config.effect == "cycle" && !device.native_cycle {
            return Err("Colour cycle requires the controller's native effect support".into());
        }
        let result = device.write(config, color, transition);
        if result.is_err() {
            self.device = None;
        }
        result
    }
}

pub fn start_sysfs(
    handle: &LedHandle,
    identity: String,
    root: PathBuf,
    suspended: bool,
    hardware_cycle_only: bool,
) -> Result<()> {
    let directory = std::env::var_os("STATE_DIRECTORY")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/var/lib/inputplumber"));
    let storage = FileStorage::new(&directory.join("leds"), &identity)?;
    let transport = RecoveringSysfsTransport {
        root,
        device: None,
        hardware_cycle_only,
    };
    let mut engine = Engine::new(
        identity,
        Box::new(transport),
        Box::new(storage),
        Box::new(SystemClock(Instant::now())),
    );
    if suspended {
        engine.paused = true;
        engine.snapshot.state.status = "unavailable".into();
    }
    handle.start(engine)
}

#[cfg(test)]
#[path = "managed_test.rs"]
mod tests;
