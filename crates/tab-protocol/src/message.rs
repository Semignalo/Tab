//! Katalog pesan sesi.
//!
//! Representasi wire adalah map CBOR dengan diskriminator internal `"t"`, yang sepadan
//! dengan `classDiscriminator = "t"` di kotlinx-serialization. Setiap nama field dituliskan
//! eksplisit lewat `rename`, sehingga mengganti nama field Rust tidak pernah memecah
//! kompatibilitas wire.

use crate::ids::Id16;
use serde::{Deserialize, Serialize};
use serde_bytes::ByteBuf;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, strum::VariantNames)]
#[serde(tag = "t")]
pub enum Message {
    // ---- control ----
    Hello(Hello),
    Welcome(Welcome),
    PairRequired(PairRequired),
    PairRequest(PairRequest),
    PairOk(PairOk),
    Ping(Ping),
    Pong(Pong),
    SetMode(SetMode),
    ModeState(ModeState),
    Bye(Bye),
    Error(ProtoError),

    // ---- trackpad & input ----
    InputBatch(InputBatch),
    InputSettings(InputSettings),
    SetInputSettings(SetInputSettings),
    ClipboardPush(ClipboardPush),
    ClipboardUpdate(ClipboardUpdate),

    // ---- deck ----
    DeckProfiles(DeckProfiles),
    DeckProfile(DeckProfile),
    SelectProfile(SelectProfile),
    DeckPress(DeckPress),
    DeckRelease(DeckRelease),
    DeckFeedback(DeckFeedback),

    // ---- telemetry ----
    SubscribeTelemetry(SubscribeTelemetry),
    Metrics(Metrics),

    // ---- music ----
    NowPlaying(NowPlaying),
    MediaCommand(MediaCommand),
    MediaSeek(MediaSeek),
    MediaVolume(MediaVolume),
    GetArtwork(GetArtwork),
    ArtworkChunk(ArtworkChunk),
    GetLyrics(GetLyrics),
    LyricsDoc(LyricsDoc),
}

// =============================== control ===============================

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Hello {
    pub pv: u16,
    pub dev: Id16,
    pub name: String,
    pub plat: String,
    pub platv: String,
    pub app: String,
    pub scr: Screen,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Screen {
    pub w: u32,
    pub h: u32,
    pub dpi: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Welcome {
    pub pv: u16,
    pub hid: Id16,
    pub name: String,
    pub os: String,
    pub osv: String,
    pub app: String,
    pub sid: Id16,
    pub caps: Capabilities,
}

/// Apa yang benar-benar bisa dilakukan host ini. Client wajib menyembunyikan kontrol untuk
/// kapabilitas yang `false` — lebih baik tidak tampil daripada tampil lalu gagal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Capabilities {
    pub modes: Vec<Mode>,
    pub clipboard: bool,
    pub gestures: Vec<String>,
    pub now_playing: bool,
    pub media_seek: bool,
    pub media_volume: bool,
    pub obs: bool,
    pub temps: bool,
    pub gpu: bool,
    pub second_screen: bool,
    pub gamepad: bool,
    pub max_frame: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Idle,
    Trackpad,
    Deck,
    Monitor,
    Music,
    Clock,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PairRequired {
    pub ttl_ms: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PairRequest {
    pub pin: String,
}

/// Dikirim tepat sebelum `Welcome` saat pairing berhasil.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PairOk {
    pub tok: ByteBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Ping {
    pub n: u64,
    /// Jam monotonik client dalam mikrodetik.
    pub tc: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Pong {
    pub n: u64,
    /// Dikembalikan apa adanya agar client bisa menghitung RTT tanpa menyimpan state.
    pub tc: u64,
    /// Jam monotonik host dalam mikrodetik.
    pub th: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SetMode {
    pub m: Mode,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModeState {
    pub m: Mode,
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub msg: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Bye {
    pub r: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProtoError {
    pub c: ErrorCode,
    pub msg: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attempts_left: Option<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorCode {
    ProtocolUnsupported,
    PairRequired,
    PinInvalid,
    PinExpired,
    PairingBusy,
    TokenRevoked,
    RateLimited,
    Unsupported,
    Internal,
}

// ============================ trackpad & input ============================

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InputBatch {
    /// Naik monoton per sesi, sehingga host dapat mendeteksi batch yang hilang.
    pub s: u64,
    pub ev: Vec<InputEvent>,
}

/// Event sudah terklasifikasi di client (tap vs scroll dua jari vs drag), karena di sanalah
/// data sentuhan mentah berada. Host menerima niat, bukan koordinat mentah.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "t")]
pub enum InputEvent {
    PointerMove {
        dx: f32,
        dy: f32,
    },
    PointerAbs {
        x: f32,
        y: f32,
    },
    PointerButton {
        b: Button,
        d: bool,
    },
    Scroll {
        dx: f32,
        dy: f32,
        ph: ScrollPhase,
        mom: bool,
    },
    Gesture {
        g: String,
        f: u8,
    },
    Key {
        k: String,
        d: bool,
    },
    Text {
        s: String,
    },
    Modifiers {
        m: u8,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Button {
    L,
    R,
    M,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScrollPhase {
    B,
    U,
    E,
}

pub mod modifiers {
    pub const SHIFT: u8 = 1;
    pub const CTRL: u8 = 2;
    pub const ALT: u8 = 4;
    pub const META: u8 = 8;
}

/// Sensitivitas dan arah scroll adalah setelan host, agar konsisten di semua perangkat.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct InputSettings {
    pub sens: f32,
    pub natural: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SetInputSettings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sens: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub natural: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClipboardPush {
    pub s: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClipboardUpdate {
    pub s: String,
}

// =============================== deck ===============================

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeckProfiles {
    pub list: Vec<ProfileRef>,
    pub active: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProfileRef {
    pub id: String,
    pub name: String,
}

/// Definisi aksi tidak pernah ikut dikirim: client hanya menerima `aid`. Perangkat yang
/// dipasangkan hanya bisa memicu aksi yang sudah dibuat user di host, bukan menyusun
/// perintah sendiri.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeckProfile {
    pub id: String,
    pub name: String,
    pub cols: u8,
    pub rows: u8,
    pub btns: Vec<DeckButton>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeckButton {
    pub i: u16,
    pub lbl: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ic: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub col: Option<String>,
    pub aid: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SelectProfile {
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeckPress {
    pub aid: String,
    pub i: u16,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeckRelease {
    pub aid: String,
    pub i: u16,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeckFeedback {
    pub aid: String,
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub msg: Option<String>,
}

// ============================= telemetry =============================

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SubscribeTelemetry {
    pub kinds: Vec<TelemetryKind>,
    /// Milidetik; host memaksa minimum `MIN_TELEMETRY_INTERVAL_MS`.
    pub iv: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TelemetryKind {
    Metrics,
    NowPlaying,
}

/// Field suhu dan GPU dihilangkan sepenuhnya bila host tidak bisa membacanya. Tidak ada
/// nilai placeholder — gauge yang tidak tersedia disembunyikan client, bukan diisi nol.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Metrics {
    pub cpu: Vec<f32>,
    pub cpua: f32,
    pub mu: u64,
    pub mt: u64,
    pub su: u64,
    pub st: u64,
    pub disks: Vec<Disk>,
    pub rx: u64,
    pub tx: u64,
    pub pwr: Power,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tcpu: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gpu: Option<Gpu>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Disk {
    pub n: String,
    pub u: u64,
    pub t: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Power {
    pub ac: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pct: Option<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Gpu {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub u: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub t: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mu: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mt: Option<u64>,
}

// =============================== music ===============================

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NowPlaying {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub dur: u64,
    /// Dikirim paling sering 1 Hz; client menginterpolasi sendiri agar timeline halus.
    pub pos: u64,
    pub play: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub art: Option<String>,
    pub lyr: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MediaCommand {
    pub c: MediaCmd,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MediaCmd {
    Play,
    Pause,
    Toggle,
    Next,
    Prev,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MediaSeek {
    pub ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MediaVolume {
    pub v: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GetArtwork {
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArtworkChunk {
    pub id: String,
    pub i: u16,
    pub n: u16,
    pub b: ByteBuf,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GetLyrics {
    pub id: String,
}

/// Lirik berasal dari file `.lrc` milik user sendiri di folder yang ia tentukan di host.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LyricsDoc {
    pub id: String,
    pub lines: Vec<LyricLine>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LyricLine {
    pub t: u64,
    pub s: String,
}
