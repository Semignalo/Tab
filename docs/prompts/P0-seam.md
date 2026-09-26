# Track P0 — Gerbang Seam (jalankan sendirian, sebelum track lain)

Branch: `track/p0-seam`

Kamu mengerjakan **satu-satunya** track yang harus selesai sebelum 14 track lain bisa jalan.
Tugasmu bukan mengimplementasikan fitur, tapi **membekukan bentuk seam** supaya 14 sesi lain
tidak saling menebak. Isi setiap fungsi tetap stub.

Baca `docs/prompts/_COMMON.md` lebih dulu.

## Yang harus mendarat

### P0.1 — Lima crate baru (kosong, terdaftar di workspace)

`crates/tab-host`, `crates/tab-input`, `crates/tab-metrics`, `crates/tab-media`,
`crates/tab-deck`. Tambahkan ke `members` di `Cargo.toml` root, urut alfabetis, dan daftarkan
juga di `[workspace.dependencies]` dengan `path`.

### P0.2 — Definisi trait

Tulis persis seperti di bawah. Tipe wire diambil langsung dari `tab_protocol::message` supaya
tidak ada lapisan pemetaan yang harus dijaga sinkron.

`crates/tab-input/src/lib.rs`:

```rust
pub trait PlatformInput: Send {
    fn permission(&self) -> PermissionState;
    fn open_permission_settings(&self) -> Result<(), InputError>;
    fn move_pointer(&mut self, dx: f32, dy: f32) -> Result<(), InputError>;
    fn move_pointer_abs(&mut self, x: f32, y: f32) -> Result<(), InputError>;
    fn button(&mut self, b: Button, down: bool) -> Result<(), InputError>;
    fn scroll(&mut self, dx: f32, dy: f32, phase: ScrollPhase, momentum: bool) -> Result<(), InputError>;
    fn key(&mut self, key: &str, down: bool) -> Result<(), InputError>;
    fn text(&mut self, s: &str) -> Result<(), InputError>;
    fn gesture(&mut self, g: &str, fingers: u8) -> Result<(), InputError>;
    fn supported_gestures(&self) -> &'static [&'static str];
    /// Dipanggil saat sesi berakhir. Modifier yang "nyangkut" setelah koneksi putus adalah
    /// bug yang paling terasa, jadi ini wajib melepas semua tombol & tombol mouse.
    fn release_all(&mut self) -> Result<(), InputError>;
}

pub trait Clipboard: Send {
    fn get(&self) -> Result<String, InputError>;
    fn set(&mut self, s: &str) -> Result<(), InputError>;
}

pub enum PermissionState { Granted, Denied, NotRequired }
```

`crates/tab-metrics/src/lib.rs`:

```rust
pub trait MetricsSource: Send {
    fn capabilities(&self) -> MetricsCaps;
    fn sample(&mut self) -> Result<tab_protocol::message::Metrics, MetricsError>;
}

pub struct MetricsCaps { pub temps: bool, pub gpu: bool }
```

`crates/tab-media/src/lib.rs`:

```rust
pub trait MediaSource: Send {
    fn capabilities(&self) -> MediaCaps;
    fn now_playing(&mut self) -> Result<Option<tab_protocol::message::NowPlaying>, MediaError>;
    fn artwork(&mut self, id: &str) -> Result<Option<Vec<u8>>, MediaError>;
    fn command(&mut self, cmd: tab_protocol::message::MediaCmd) -> Result<(), MediaError>;
    fn seek(&mut self, ms: u64) -> Result<(), MediaError>;
    fn volume(&mut self, v: f32) -> Result<(), MediaError>;
}

pub struct MediaCaps { pub now_playing: bool, pub seek: bool, pub volume: bool }

/// Fungsi murni, diuji tanpa I/O apa pun.
pub fn parse_lrc(src: &str) -> Vec<tab_protocol::message::LyricLine>;
```

`crates/tab-deck/src/lib.rs`:

```rust
pub enum Action {
    Hotkey { keys: Vec<String> },
    LaunchApp { path: String },
    OpenPath { path: String },
    OpenUrl { url: String },
    MediaKey { key: String },
    ObsScene { scene: String },
    Multi { steps: Vec<Action> },
}

pub struct ActionDef { pub id: String, pub action: Action }

pub struct Profile {
    pub id: String,
    pub name: String,
    pub cols: u8,
    pub rows: u8,
    pub buttons: Vec<ButtonDef>,
    pub actions: Vec<ActionDef>,
}

pub struct ButtonDef {
    pub index: u16,
    pub label: String,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub action_id: String,
}

pub trait ActionRunner: Send {
    fn run(&mut self, action: &Action) -> Result<(), ActionError>;
}

pub trait ProfileStore: Send {
    fn list(&self) -> Result<Vec<Profile>, ActionError>;
    fn load(&self, id: &str) -> Result<Option<Profile>, ActionError>;
    fn save(&mut self, profile: &Profile) -> Result<(), ActionError>;
    fn delete(&mut self, id: &str) -> Result<(), ActionError>;
}

/// Membuang definisi aksi sebelum profil dikirim ke perangkat: client hanya menerima
/// `action_id`. Perangkat yang dipasangkan boleh memicu aksi milik user, tapi tidak boleh
/// bisa menyusun perintah sendiri.
pub fn to_wire(profile: &Profile) -> tab_protocol::message::DeckProfile;
```

`crates/tab-host/src/lib.rs`:

```rust
pub struct HostConfig {
    pub name: String,
    pub discovery_port: u16,
    pub session_port: u16,
    pub max_sessions: usize,
}

pub struct HostDeps {
    pub input: Box<dyn PlatformInput>,
    pub clipboard: Box<dyn Clipboard>,
    pub metrics: Box<dyn MetricsSource>,
    pub media: Box<dyn MediaSource>,
    pub deck: Box<dyn ProfileStore>,
    pub runner: Box<dyn ActionRunner>,
    pub store: Box<dyn TokenStore>,
}

pub trait TokenStore: Send + Sync {
    fn keypair(&self) -> Result<tab_protocol::noise::StaticKeypair, StoreError>;
    fn devices(&self) -> Result<Vec<DeviceEntry>, StoreError>;
    fn upsert(&mut self, entry: &DeviceEntry) -> Result<(), StoreError>;
    fn remove(&mut self, device: tab_protocol::Id16) -> Result<(), StoreError>;
}

pub struct DeviceEntry {
    pub device: tab_protocol::Id16,
    pub token: [u8; 32],
    pub name: String,
    pub platform: String,
    pub paired_at: u64,
    pub last_seen: u64,
}

pub enum HostEvent {
    PairingStarted { pin: String, ttl_ms: u32 },
    PairingSucceeded { device: tab_protocol::Id16, name: String },
    PairingFailed { reason: String, attempts_left: u8 },
    DeviceConnected { device: tab_protocol::Id16, name: String },
    DeviceDisconnected { device: tab_protocol::Id16 },
    ModeChanged { device: tab_protocol::Id16, mode: tab_protocol::message::Mode },
    Rtt { device: tab_protocol::Id16, micros: u64 },
}

impl HostHandle {
    pub async fn start(cfg: HostConfig, deps: HostDeps) -> Result<Self, HostError>;
    pub fn events(&self) -> tokio::sync::broadcast::Receiver<HostEvent>;
    pub async fn begin_pairing(&self) -> Result<(), HostError>;
    pub async fn cancel_pairing(&self) -> Result<(), HostError>;
    pub async fn devices(&self) -> Result<Vec<DeviceEntry>, HostError>;
    pub async fn revoke(&self, device: tab_protocol::Id16) -> Result<(), HostError>;
    pub async fn shutdown(self) -> Result<(), HostError>;
}
```

Semua isi fungsi: `unimplemented!("track X")` yang menyebut track pemiliknya. Semua tipe error
memakai `thiserror`.

Sertakan juga implementasi **stub yang bisa dipakai track lain untuk test**: `NoopInput`,
`NoopClipboard`, `FakeMetrics`, `FakeMedia`, `MemoryProfileStore`, `MemoryTokenStore`. Ini
yang membuat track A bisa dites tanpa track C/D/E/F, dan track G tanpa siapa pun.

### P0.3 — Modul Gradle Android

`client-android/` dengan Gradle multi-module (ikuti pola `/Users/stefanuslo/Projects/HUD`:
`settings.gradle.kts` + `gradle/libs.versions.toml`), minSdk 26, Kotlin + Compose:
`app`, `core:model`, `core:net`, `feature:trackpad`, `feature:deck`, `feature:monitor`,
`feature:music`, `feature:clock`. Modul selain `core:model` boleh kosong.

`core:model` berisi tipe pesan CBOR dengan `Cbor { classDiscriminator = "t" }`, sealed
interface `Message`, dan **uji kontrak** yang membaca `protocol/fixtures/*.cbor` lalu
memastikan setiap fixture terbaca menjadi tipe yang benar **dan** terserialisasi ulang menjadi
byte yang identik. Fixture itu dihasilkan sisi Rust, jadi inilah yang mencegah dua sisi
protokol mengambang.

### P0.4 — CI

`.github/workflows/ci.yml`: matrix `macos-latest` + `windows-latest` untuk
`cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`,
`cargo fmt --check`; plus job Ubuntu untuk `./gradlew test`.

## Selesai berarti

```bash
cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --check
cd client-android && ./gradlew test
```

hijau, dengan seluruh implementasi masih stub. Lalu centang P0.1–P0.5 di `docs/STEPS.md`.

## Jangan

Jangan mengimplementasikan satu pun fitur. Kalau kamu tergoda mengisi `sample()` atau
`move_pointer()`, itu pekerjaan track D dan C — mengisinya di sini justru menimbulkan konflik
merge yang mahal.
