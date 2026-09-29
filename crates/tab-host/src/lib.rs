//! Host Tab: discovery UDP, listener TCP, sesi terenkripsi, pairing PIN, dan registry perangkat.
//!
//! Crate ini library murni tanpa UI, sehingga seluruh alurnya (pairing, reconnect, token
//! dicabut, sesi penuh) bisa dites di dalam proses tanpa Tauri maupun perangkat Android.

mod discovery;
mod listener;
pub mod pairing;
mod registry;
mod session;
pub mod store;

pub use registry::Registry;
pub use store::{secret_get, secret_set, DeviceEntry, MemoryTokenStore, StoreError, TokenStore};

#[cfg(any(windows, target_os = "macos"))]
pub use store::KeyringTokenStore;

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tab_deck::{ActionRunner, ObsHandle, ProfileStore};
use tab_input::{Clipboard, PlatformInput};
use tab_media::MediaSource;
use tab_metrics::MetricsSource;
use tab_protocol::message::{Capabilities, InputSettings, Mode};
use tab_protocol::noise::{fingerprint, StaticKeypair};
use tab_protocol::{Id16, MAX_FRAME, MAX_SESSIONS};
use thiserror::Error;
use tokio::sync::{broadcast, watch, Semaphore};
use tokio::task::JoinHandle;

#[derive(Debug, Error)]
pub enum HostError {
    #[error("tidak bisa membuka port: {0}")]
    Bind(#[from] std::io::Error),
    #[error("penyimpanan: {0}")]
    Store(#[from] StoreError),
    #[error("host sudah berhenti")]
    Stopped,
}

#[derive(Debug, Clone)]
pub struct HostConfig {
    pub name: String,
    /// `"macos"` | `"windows"`.
    pub os: String,
    pub os_version: String,
    pub app_version: String,
    pub bind: IpAddr,
    pub discovery_port: u16,
    pub session_port: u16,
    pub max_sessions: usize,
    /// Folder `.lrc` milik user; `None` berarti fitur lirik mati.
    pub lyrics_dir: Option<PathBuf>,
}

impl HostConfig {
    /// Nilai bawaan yang diambil dari mesin ini.
    pub fn detect() -> Self {
        Self {
            name: std::env::var("COMPUTERNAME")
                .or_else(|_| std::env::var("HOSTNAME"))
                .unwrap_or_else(|_| "Tab Host".into()),
            os: std::env::consts::OS.to_owned(),
            os_version: std::env::var("TAB_OS_VERSION").unwrap_or_else(|_| "unknown".into()),
            app_version: env!("CARGO_PKG_VERSION").to_owned(),
            bind: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            discovery_port: tab_protocol::DISCOVERY_PORT,
            session_port: tab_protocol::SESSION_PORT,
            max_sessions: MAX_SESSIONS,
            lyrics_dir: None,
        }
    }
}

pub struct HostDeps {
    pub input: Box<dyn PlatformInput>,
    pub clipboard: Box<dyn Clipboard>,
    pub metrics: Box<dyn MetricsSource>,
    pub media: Box<dyn MediaSource>,
    pub deck: Box<dyn ProfileStore>,
    pub runner: Box<dyn ActionRunner>,
    pub store: Box<dyn TokenStore>,
    /// Pegangan OBS bersama runner; `caps.obs` hanya true bila OBS menjawab saat `Welcome`.
    pub obs: Arc<ObsHandle>,
}

impl HostDeps {
    /// Semua stub: tidak menyentuh OS sama sekali. Untuk test.
    pub fn fakes() -> Self {
        Self {
            input: Box::new(tab_input::NoopInput),
            clipboard: Box::new(tab_input::NoopClipboard::default()),
            metrics: Box::new(tab_metrics::FakeMetrics::default()),
            media: Box::new(tab_media::FakeMedia::default()),
            deck: Box::new(tab_deck::MemoryProfileStore::with_starter()),
            runner: Box::new(tab_deck::RecordingRunner::default()),
            store: Box::new(MemoryTokenStore::new()),
            obs: Arc::new(ObsHandle::new()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostEvent {
    PairingStarted {
        pin: String,
        ttl_ms: u32,
    },
    PairingSucceeded {
        device: Id16,
        name: String,
    },
    PairingFailed {
        reason: String,
        attempts_left: u8,
    },
    /// PIN kedaluwarsa, habis percobaan, atau dibatalkan — UI perlu menutup dialognya.
    PairingEnded,
    DeviceConnected {
        device: Id16,
        name: String,
    },
    DeviceDisconnected {
        device: Id16,
    },
    DeviceRevoked {
        device: Id16,
    },
    ModeChanged {
        device: Id16,
        mode: Mode,
    },
}

/// Hasil `begin_pairing`: PIN untuk ditampilkan ke user.
#[derive(Clone)]
pub struct PairingTicket {
    pub pin: String,
    pub ttl: Duration,
}

impl std::fmt::Debug for PairingTicket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // PIN tidak pernah dicatat ke log.
        f.debug_struct("PairingTicket")
            .field("ttl", &self.ttl)
            .finish_non_exhaustive()
    }
}

/// State yang dipakai bersama oleh discovery, listener, dan semua sesi.
pub(crate) struct Shared {
    pub cfg: HostConfig,
    pub keys: StaticKeypair,
    pub host_id: Id16,
    pub fingerprint: [u8; 32],
    pub session_port: u16,
    pub registry: Registry,
    pub pairing: Mutex<pairing::Pairing>,
    pub input: Mutex<Box<dyn PlatformInput>>,
    pub clipboard: Mutex<Box<dyn Clipboard>>,
    pub metrics: Mutex<Box<dyn MetricsSource>>,
    pub media: Mutex<Box<dyn MediaSource>>,
    pub deck: Mutex<Box<dyn ProfileStore>>,
    pub runner: Mutex<Box<dyn ActionRunner>>,
    pub events: broadcast::Sender<HostEvent>,
    pub settings: watch::Sender<InputSettings>,
    pub caps: Capabilities,
    pub obs: Arc<ObsHandle>,
    pub start: Instant,
    /// Folder .lrc milik user; dapat diganti saat host berjalan.
    pub lyrics_dir: Mutex<Option<PathBuf>>,
    /// Jumlah sesi yang sudah lolos Welcome; dipakai untuk memutuskan `release_all`.
    pub live_sessions: Mutex<usize>,
}

impl Shared {
    pub fn emit(&self, ev: HostEvent) {
        let _ = self.events.send(ev);
    }
}

fn build_caps(deps: &HostDeps, deck_ok: bool) -> Capabilities {
    let m = deps.metrics.capabilities();
    let media = deps.media.capabilities();
    let mut modes = vec![Mode::Trackpad];
    if deck_ok {
        modes.push(Mode::Deck);
    }
    modes.push(Mode::Monitor);
    if media.now_playing {
        modes.push(Mode::Music);
    }
    modes.push(Mode::Clock);
    Capabilities {
        modes,
        clipboard: true,
        gestures: deps
            .input
            .supported_gestures()
            .iter()
            .map(|s| (*s).to_owned())
            .collect(),
        now_playing: media.now_playing,
        media_seek: media.seek,
        media_volume: media.volume,
        // Sampai integrasi OBS ada, kapabilitas ini jujur `false`.
        obs: false,
        temps: m.temps,
        gpu: m.gpu,
        second_screen: false,
        gamepad: false,
        max_frame: MAX_FRAME as u32,
    }
}

pub struct HostHandle {
    shared: Arc<Shared>,
    shutdown: watch::Sender<bool>,
    slots: Arc<Semaphore>,
    max_sessions: usize,
    tasks: Vec<JoinHandle<()>>,
    session_addr: SocketAddr,
    discovery_addr: SocketAddr,
}

impl HostHandle {
    pub async fn start(cfg: HostConfig, deps: HostDeps) -> Result<Self, HostError> {
        let keys = deps.store.keypair()?;
        let fp = fingerprint(&keys.public);
        let mut hid = [0u8; 16];
        hid.copy_from_slice(&fp[..16]);

        let listener = listener::bind(SocketAddr::new(cfg.bind, cfg.session_port))?;
        let session_addr = listener.local_addr()?;
        let udp = discovery::bind(SocketAddr::new(cfg.bind, cfg.discovery_port))?;
        let discovery_addr = udp.local_addr()?;

        let deck_ok = deps.deck.list().map(|l| !l.is_empty()).unwrap_or(false);
        let caps = build_caps(&deps, deck_ok);
        let max_sessions = cfg.max_sessions;
        let lyrics_dir = cfg.lyrics_dir.clone();

        let (events, _) = broadcast::channel(64);
        let (settings, _) = watch::channel(InputSettings {
            sens: 1.0,
            natural: true,
        });
        let shared = Arc::new(Shared {
            cfg,
            keys,
            host_id: Id16(hid),
            fingerprint: fp,
            session_port: session_addr.port(),
            registry: Registry::new(deps.store),
            pairing: Mutex::new(pairing::Pairing::new()),
            input: Mutex::new(deps.input),
            clipboard: Mutex::new(deps.clipboard),
            metrics: Mutex::new(deps.metrics),
            media: Mutex::new(deps.media),
            deck: Mutex::new(deps.deck),
            runner: Mutex::new(deps.runner),
            events,
            settings,
            caps,
            obs: deps.obs,
            start: Instant::now(),
            lyrics_dir: Mutex::new(lyrics_dir),
            live_sessions: Mutex::new(0),
        });

        let (shutdown, rx) = watch::channel(false);
        let slots = Arc::new(Semaphore::new(max_sessions));
        let tasks = vec![
            tokio::spawn(listener::run(
                Arc::clone(&shared),
                listener,
                Arc::clone(&slots),
                rx.clone(),
            )),
            tokio::spawn(discovery::run(Arc::clone(&shared), udp, rx)),
        ];

        Ok(Self {
            shared,
            shutdown,
            slots,
            max_sessions,
            tasks,
            session_addr,
            discovery_addr,
        })
    }

    pub fn events(&self) -> broadcast::Receiver<HostEvent> {
        self.shared.events.subscribe()
    }

    pub fn session_addr(&self) -> SocketAddr {
        self.session_addr
    }

    pub fn discovery_addr(&self) -> SocketAddr {
        self.discovery_addr
    }

    pub fn host_id(&self) -> Id16 {
        self.shared.host_id
    }

    pub fn fingerprint(&self) -> [u8; 32] {
        self.shared.fingerprint
    }

    pub fn public_key(&self) -> [u8; 32] {
        self.shared.keys.public
    }

    pub fn capabilities(&self) -> Capabilities {
        self.shared.caps.clone()
    }

    /// Buka jendela pairing. PIN dikembalikan untuk ditampilkan, dan juga dipancarkan lewat
    /// `HostEvent::PairingStarted`.
    pub async fn begin_pairing(&self) -> Result<PairingTicket, HostError> {
        let (pin, ttl) = self.shared.pairing.lock().unwrap().begin(Instant::now());
        self.shared.emit(HostEvent::PairingStarted {
            pin: pin.clone(),
            ttl_ms: ttl.as_millis() as u32,
        });
        Ok(PairingTicket { pin, ttl })
    }

    pub async fn cancel_pairing(&self) -> Result<(), HostError> {
        self.shared.pairing.lock().unwrap().cancel();
        self.shared.emit(HostEvent::PairingEnded);
        Ok(())
    }

    pub async fn devices(&self) -> Result<Vec<DeviceEntry>, HostError> {
        Ok(self.shared.registry.devices()?)
    }

    /// Cabut token; sesi hidup milik perangkat itu ditutup segera.
    pub async fn revoke(&self, device: Id16) -> Result<(), HostError> {
        self.shared.registry.revoke(device)?;
        self.shared.emit(HostEvent::DeviceRevoked { device });
        Ok(())
    }

    /// Ganti folder lirik saat host berjalan (`None` mematikan fitur lirik).
    pub fn set_lyrics_dir(&self, dir: Option<PathBuf>) {
        *self.shared.lyrics_dir.lock().unwrap() = dir;
    }

    /// Setelan input global (sensitivitas, arah scroll) — dipancarkan ke semua sesi.
    pub fn set_input_settings(&self, s: InputSettings) {
        self.shared.settings.send_replace(s);
    }

    pub async fn shutdown(self) -> Result<(), HostError> {
        let _ = self.shutdown.send(true);
        for t in self.tasks {
            let _ = t.await;
        }
        // Tunggu semua sesi melepas slot-nya (mereka menutup diri saat sinyal shutdown).
        let all = self.max_sessions as u32;
        let _ = tokio::time::timeout(Duration::from_secs(3), self.slots.acquire_many(all)).await;
        Ok(())
    }
}
