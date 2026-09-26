//! Tab wire protocol.
//!
//! Satu implementasi dipakai bersama oleh host Tauri dan `tab-probe`, sehingga tidak ada
//! kemungkinan kedua sisi Rust menafsirkan wire format secara berbeda. Sisi Kotlin
//! mengikuti dokumen `protocol/PROTOCOL.md` dan diuji terhadap fixture dari crate ini.

pub mod discovery;
pub mod frame;
pub mod ids;
pub mod message;
pub mod noise;

pub use ids::Id16;
pub use message::Message;

use std::time::Duration;

/// Versi yang dipakai implementasi ini saat memulai koneksi.
pub const PROTOCOL_VERSION: u16 = 1;
/// Rentang versi yang diterima host.
pub const PROTOCOL_MIN: u16 = 1;
pub const PROTOCOL_MAX: u16 = 1;

pub const DISCOVERY_PORT: u16 = 4179;
pub const SESSION_PORT: u16 = 4180;

/// Batas satu pesan Noise, sekaligus batas satu frame.
pub const MAX_FRAME: usize = 65_535;
/// Ruang untuk tag AEAD 16 byte di dalam `MAX_FRAME`.
pub const MAX_PLAINTEXT: usize = MAX_FRAME - 16;
/// Frame telemetri/artwork dijaga kecil agar tidak menahan event input.
pub const TELEMETRY_FRAME_LIMIT: usize = 8 * 1024;
pub const MAX_INPUT_EVENTS: usize = 64;
pub const MAX_CLIPBOARD: usize = 64 * 1024;
pub const MAX_DATAGRAM: usize = 1200;
pub const MAX_SESSIONS: usize = 8;

pub const PIN_DIGITS: usize = 6;
pub const PIN_TTL: Duration = Duration::from_secs(120);
pub const PIN_MAX_ATTEMPTS: u8 = 5;

pub const PING_INTERVAL: Duration = Duration::from_secs(2);
pub const SESSION_TIMEOUT: Duration = Duration::from_secs(6);
pub const MIN_TELEMETRY_INTERVAL_MS: u32 = 250;

/// Pola Noise untuk perangkat yang belum dipasangkan: host terautentikasi oleh static key
/// dari discovery, dan PIN-lah yang membuktikan bahwa host itu benar-benar milik user.
pub const NOISE_PAIR: &str = "Noise_NK_25519_ChaChaPoly_BLAKE2s";
/// Pola Noise untuk resume: token diikat ke handshake sebagai PSK, jadi token tidak pernah
/// dikirim sebagai payload dan tidak bisa diputar ulang ke host lain.
pub const NOISE_RESUME: &str = "Noise_NKpsk2_25519_ChaChaPoly_BLAKE2s";

pub const TOKEN_LEN: usize = 32;

/// Apakah versi protokol client dapat dilayani host ini.
pub fn version_supported(client: u16) -> bool {
    (PROTOCOL_MIN..=PROTOCOL_MAX).contains(&client)
}
