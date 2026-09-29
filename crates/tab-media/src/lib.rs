//! Sumber media (now playing, transport, artwork) dan pembaca lirik `.lrc`.
//!
//! Lirik hanya berasal dari file `.lrc` milik user di folder yang ia pilih sendiri; tidak
//! ada permintaan ke layanan pihak ketiga.

use std::path::{Path, PathBuf};
use tab_protocol::message::{LyricLine, MediaCmd, NowPlaying};
use thiserror::Error;

mod lrc;
#[cfg(windows)]
mod windows;

pub use lrc::{find_lrc, parse_lrc};

#[derive(Debug, Error)]
pub enum MediaError {
    #[error("tidak didukung: {0}")]
    Unsupported(String),
    #[error("gagal membaca sumber media: {0}")]
    Backend(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MediaCaps {
    pub now_playing: bool,
    pub seek: bool,
    pub volume: bool,
}

pub trait MediaSource: Send {
    fn capabilities(&self) -> MediaCaps;
    fn now_playing(&mut self) -> Result<Option<NowPlaying>, MediaError>;
    fn artwork(&mut self, id: &str) -> Result<Option<Vec<u8>>, MediaError>;
    fn command(&mut self, cmd: MediaCmd) -> Result<(), MediaError>;
    fn seek(&mut self, ms: u64) -> Result<(), MediaError>;
    fn volume(&mut self, v: f32) -> Result<(), MediaError>;
}

/// Sumber media untuk OS yang sedang berjalan. Bila backend tidak tersedia (macOS sampai
/// sidecar terpasang), `capabilities().now_playing` bernilai `false` dan client menyembunyikan
/// kontrol musik — bukan menampilkan panel kosong.
pub fn platform_media() -> Box<dyn MediaSource> {
    #[cfg(windows)]
    {
        match windows::WindowsMedia::new() {
            Ok(m) => return Box::new(m),
            Err(e) => tracing::warn!("media Windows tidak tersedia: {e}"),
        }
    }
    Box::new(UnavailableMedia)
}

/// Backend yang jujur mengatakan tidak ada.
#[derive(Debug, Default)]
pub struct UnavailableMedia;

impl MediaSource for UnavailableMedia {
    fn capabilities(&self) -> MediaCaps {
        MediaCaps::default()
    }
    fn now_playing(&mut self) -> Result<Option<NowPlaying>, MediaError> {
        Ok(None)
    }
    fn artwork(&mut self, _: &str) -> Result<Option<Vec<u8>>, MediaError> {
        Ok(None)
    }
    fn command(&mut self, _: MediaCmd) -> Result<(), MediaError> {
        Err(MediaError::Unsupported("media".into()))
    }
    fn seek(&mut self, _: u64) -> Result<(), MediaError> {
        Err(MediaError::Unsupported("seek".into()))
    }
    fn volume(&mut self, _: f32) -> Result<(), MediaError> {
        Err(MediaError::Unsupported("volume".into()))
    }
}

/// Lagu tetap untuk test host dan UI.
#[derive(Debug, Clone)]
pub struct FakeMedia {
    pub track: Option<NowPlaying>,
    pub artwork: Option<Vec<u8>>,
    pub commands: Vec<MediaCmd>,
}

impl Default for FakeMedia {
    fn default() -> Self {
        Self {
            track: Some(NowPlaying {
                title: "Fake Song".into(),
                artist: "Fake Artist".into(),
                album: "Fake Album".into(),
                dur: 180_000,
                pos: 12_000,
                play: true,
                art: Some("fake-art".into()),
                lyr: false,
            }),
            artwork: Some(vec![0xAB; 20_000]),
            commands: Vec::new(),
        }
    }
}

impl MediaSource for FakeMedia {
    fn capabilities(&self) -> MediaCaps {
        MediaCaps {
            now_playing: true,
            seek: true,
            volume: false,
        }
    }
    fn now_playing(&mut self) -> Result<Option<NowPlaying>, MediaError> {
        Ok(self.track.clone())
    }
    fn artwork(&mut self, id: &str) -> Result<Option<Vec<u8>>, MediaError> {
        Ok(if id == "fake-art" {
            self.artwork.clone()
        } else {
            None
        })
    }
    fn command(&mut self, cmd: MediaCmd) -> Result<(), MediaError> {
        self.commands.push(cmd);
        Ok(())
    }
    fn seek(&mut self, ms: u64) -> Result<(), MediaError> {
        if let Some(t) = self.track.as_mut() {
            t.pos = ms;
        }
        Ok(())
    }
    fn volume(&mut self, _: f32) -> Result<(), MediaError> {
        Err(MediaError::Unsupported("volume".into()))
    }
}

/// Muat lirik untuk sebuah lagu dari folder milik user. `None` bila tidak ada file yang cocok.
pub fn load_lyrics(dir: &Path, artist: &str, title: &str) -> Option<Vec<LyricLine>> {
    let path: PathBuf = find_lrc(dir, artist, title)?;
    let src = std::fs::read_to_string(path).ok()?;
    let lines = parse_lrc(&src);
    (!lines.is_empty()).then_some(lines)
}
