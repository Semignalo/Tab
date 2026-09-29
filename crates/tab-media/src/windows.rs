//! Media Windows lewat `GlobalSystemMediaTransportControlsSessionManager` (SMTC).
//!
//! Ini API yang sama dipakai overlay volume Windows, sehingga bekerja untuk Spotify, browser,
//! dan pemutar apa pun yang mendaftar ke SMTC — tanpa hak khusus.

use crate::{MediaCaps, MediaError, MediaSource};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::time::{SystemTime, UNIX_EPOCH};
use tab_protocol::message::{MediaCmd, NowPlaying};
use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSession as Session,
    GlobalSystemMediaTransportControlsSessionManager as Manager,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus as Status,
};
use windows::Storage::Streams::DataReader;

/// Selisih antara epoch Windows (1601) dan epoch Unix, dalam satuan 100 ns.
const WIN_TO_UNIX_TICKS: i64 = 116_444_736_000_000_000;

pub struct WindowsMedia {
    manager: Manager,
    art_id: Option<String>,
    art_bytes: Option<Vec<u8>>,
}

fn be<E: std::fmt::Display>(e: E) -> MediaError {
    MediaError::Backend(e.to_string())
}

impl WindowsMedia {
    pub fn new() -> Result<Self, MediaError> {
        let manager = Manager::RequestAsync().map_err(be)?.join().map_err(be)?;
        Ok(Self {
            manager,
            art_id: None,
            art_bytes: None,
        })
    }

    fn session(&self) -> Option<Session> {
        self.manager.GetCurrentSession().ok()
    }
}

fn art_id(title: &str, artist: &str, album: &str) -> String {
    let mut h = DefaultHasher::new();
    (title, artist, album).hash(&mut h);
    format!("{:016x}", h.finish())
}

fn read_thumbnail(session: &Session) -> Option<Vec<u8>> {
    let props = session.TryGetMediaPropertiesAsync().ok()?.join().ok()?;
    let thumb = props.Thumbnail().ok()?;
    let stream = thumb.OpenReadAsync().ok()?.join().ok()?;
    let size = stream.Size().ok()? as u32;
    if size == 0 || size > 4 * 1024 * 1024 {
        return None;
    }
    let reader = DataReader::CreateDataReader(&stream).ok()?;
    reader.LoadAsync(size).ok()?.join().ok()?;
    let mut buf = vec![0u8; size as usize];
    reader.ReadBytes(&mut buf).ok()?;
    Some(buf)
}

impl MediaSource for WindowsMedia {
    fn capabilities(&self) -> MediaCaps {
        MediaCaps {
            now_playing: true,
            seek: true,
            // SMTC tidak mengekspos volume master; jangan berpura-pura.
            volume: false,
        }
    }

    fn now_playing(&mut self) -> Result<Option<NowPlaying>, MediaError> {
        let Some(session) = self.session() else {
            return Ok(None);
        };
        let props = session
            .TryGetMediaPropertiesAsync()
            .map_err(be)?
            .join()
            .map_err(be)?;
        let title = props.Title().map(|s| s.to_string()).unwrap_or_default();
        let artist = props.Artist().map(|s| s.to_string()).unwrap_or_default();
        let album = props.AlbumTitle().map(|s| s.to_string()).unwrap_or_default();
        if title.is_empty() && artist.is_empty() {
            return Ok(None);
        }

        let playing = session
            .GetPlaybackInfo()
            .and_then(|i| i.PlaybackStatus())
            .map(|s| s == Status::Playing)
            .unwrap_or(false);

        let (dur, pos) = match session.GetTimelineProperties() {
            Ok(t) => {
                let ticks = |v: Result<windows::Foundation::TimeSpan, _>| {
                    v.map(|d| d.Duration.max(0)).unwrap_or(0)
                };
                let end = ticks(t.EndTime());
                let start = ticks(t.StartTime());
                let mut pos = ticks(t.Position());
                // Posisi dari SMTC adalah snapshot saat `LastUpdatedTime`; majukan sejauh
                // waktu yang lewat bila sedang diputar.
                if playing {
                    if let Ok(updated) = t.LastUpdatedTime() {
                        let now_win = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .map(|d| d.as_nanos() as i64 / 100 + WIN_TO_UNIX_TICKS)
                            .unwrap_or(0);
                        let since = (now_win - updated.UniversalTime).clamp(0, 6 * 3_600 * 10_000_000);
                        pos += since;
                    }
                }
                let dur = (end - start).max(0);
                let pos = if dur > 0 { pos.min(dur) } else { pos };
                ((dur / 10_000) as u64, (pos / 10_000) as u64)
            }
            Err(_) => (0, 0),
        };

        let id = art_id(&title, &artist, &album);
        if self.art_id.as_deref() != Some(id.as_str()) {
            self.art_bytes = read_thumbnail(&session);
            self.art_id = Some(id.clone());
        }
        let art = self.art_bytes.as_ref().map(|_| id);

        Ok(Some(NowPlaying {
            title,
            artist,
            album,
            dur,
            pos,
            play: playing,
            art,
            // Diisi host: hanya host yang tahu apakah ada file .lrc yang cocok.
            lyr: false,
        }))
    }

    fn artwork(&mut self, id: &str) -> Result<Option<Vec<u8>>, MediaError> {
        Ok(if self.art_id.as_deref() == Some(id) {
            self.art_bytes.clone()
        } else {
            None
        })
    }

    fn command(&mut self, cmd: MediaCmd) -> Result<(), MediaError> {
        let session = self
            .session()
            .ok_or_else(|| MediaError::Backend("tidak ada sesi media aktif".into()))?;
        let ok = match cmd {
            MediaCmd::Play => session.TryPlayAsync(),
            MediaCmd::Pause => session.TryPauseAsync(),
            MediaCmd::Toggle => session.TryTogglePlayPauseAsync(),
            MediaCmd::Next => session.TrySkipNextAsync(),
            MediaCmd::Prev => session.TrySkipPreviousAsync(),
        }
        .map_err(be)?
        .join()
        .map_err(be)?;
        if ok {
            Ok(())
        } else {
            Err(MediaError::Backend("pemutar menolak perintah".into()))
        }
    }

    fn seek(&mut self, ms: u64) -> Result<(), MediaError> {
        let session = self
            .session()
            .ok_or_else(|| MediaError::Backend("tidak ada sesi media aktif".into()))?;
        let ok = session
            .TryChangePlaybackPositionAsync(ms as i64 * 10_000)
            .map_err(be)?
            .join()
            .map_err(be)?;
        if ok {
            Ok(())
        } else {
            Err(MediaError::Backend("pemutar menolak seek".into()))
        }
    }

    fn volume(&mut self, _: f32) -> Result<(), MediaError> {
        Err(MediaError::Unsupported("volume".into()))
    }
}
