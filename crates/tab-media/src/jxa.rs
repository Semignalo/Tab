//! Media macOS lewat JXA (`osascript -l JavaScript`) untuk Spotify dan Music.app.
//!
//! Ini jalur fallback yang **tidak** menuntut SIP dimatikan dan tidak memakai API privat
//! MediaRemote. Bagian pemrosesan (skrip dan parser) dibuat lintas-platform supaya bisa diuji
//! di mana saja; hanya eksekusi `osascript` yang khusus macOS (lihat `macos.rs`).

// Di OS selain macOS modul ini hanya dipakai test; jangan dianggap kode mati.
#![cfg_attr(not(target_os = "macos"), allow(dead_code))]

use serde::Deserialize;
use tab_protocol::message::NowPlaying;

/// Skrip JXA: melaporkan pemutar aktif sebagai satu baris JSON. Aplikasi yang tidak berjalan
/// dilewati tanpa membukanya (`running()` dicek dulu, kalau tidak `Application(...)` akan
/// meluncurkan aplikasinya).
pub const QUERY_SCRIPT: &str = r#"
function run() {
  var best = null;
  ["Spotify", "Music"].forEach(function (name) {
    try {
      var app = Application(name);
      if (!app.running()) return;
      var st = String(app.playerState());
      if (st === "stopped") return;
      var t = app.currentTrack;
      var item = {
        app: name, state: st,
        title: String(t.name()), artist: String(t.artist()), album: String(t.album()),
        duration: Number(t.duration()), position: Number(app.playerPosition())
      };
      if (best === null || (st === "playing" && best.state !== "playing")) best = item;
    } catch (e) {}
  });
  return JSON.stringify(best);
}
"#;

#[derive(Debug, Deserialize, PartialEq)]
pub struct Reading {
    pub app: String,
    pub state: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    /// Spotify: milidetik. Music.app: detik. Lihat [`Reading::duration_ms`].
    pub duration: f64,
    /// Selalu detik.
    pub position: f64,
}

impl Reading {
    pub fn duration_ms(&self) -> u64 {
        let ms = if self.app == "Spotify" {
            self.duration
        } else {
            self.duration * 1000.0
        };
        ms.max(0.0) as u64
    }

    pub fn position_ms(&self) -> u64 {
        (self.position * 1000.0).max(0.0) as u64
    }

    pub fn is_playing(&self) -> bool {
        self.state == "playing"
    }

    pub fn to_now_playing(&self) -> NowPlaying {
        let dur = self.duration_ms();
        let pos = self.position_ms();
        NowPlaying {
            title: self.title.clone(),
            artist: self.artist.clone(),
            album: self.album.clone(),
            dur,
            pos: if dur > 0 { pos.min(dur) } else { pos },
            play: self.is_playing(),
            // Artwork lewat JXA tidak tersedia tanpa mengambil dari layanan pihak ketiga;
            // dengan jujur tidak diumumkan.
            art: None,
            lyr: false,
        }
    }
}

/// `None` untuk "tidak ada yang diputar" (`null`) dan untuk keluaran yang tidak bisa dibaca.
pub fn parse(output: &str) -> Option<Reading> {
    let trimmed = output.trim();
    if trimmed.is_empty() || trimmed == "null" {
        return None;
    }
    serde_json::from_str::<Reading>(trimmed).ok()
}

/// Skrip perintah untuk pemutar `app`. `command` salah satu kunci yang dikenal; nilai dikirim
/// sebagai angka sehingga tidak ada teks bebas yang masuk ke skrip.
pub fn command_script(app: &str, command: &Command) -> Option<String> {
    if app != "Spotify" && app != "Music" {
        return None;
    }
    let body = match command {
        Command::Play => "a.play()".to_owned(),
        Command::Pause => "a.pause()".to_owned(),
        Command::Toggle => "a.playpause()".to_owned(),
        Command::Next => "a.nextTrack()".to_owned(),
        Command::Prev => "a.previousTrack()".to_owned(),
        Command::SeekMs(ms) => format!("a.playerPosition = {}", *ms as f64 / 1000.0),
        Command::Volume(v) => format!(
            "a.soundVolume = {}",
            (v.clamp(0.0, 1.0) * 100.0).round() as u32
        ),
    };
    Some(format!("var a = Application('{app}'); {body};"))
}

#[derive(Debug, PartialEq)]
pub enum Command {
    Play,
    Pause,
    Toggle,
    Next,
    Prev,
    SeekMs(u64),
    Volume(f32),
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPOTIFY: &str = r#"{"app":"Spotify","state":"playing","title":"Contoh Lagu","artist":"Artis","album":"Album","duration":213000,"position":12.5}"#;
    const MUSIC: &str = r#"{"app":"Music","state":"paused","title":"Lagu","artist":"A","album":"B","duration":200.5,"position":30}"#;

    #[test]
    fn spotify_duration_is_milliseconds_position_is_seconds() {
        let r = parse(SPOTIFY).unwrap();
        assert_eq!(r.duration_ms(), 213_000);
        assert_eq!(r.position_ms(), 12_500);
        assert!(r.is_playing());
    }

    #[test]
    fn music_app_duration_is_seconds() {
        let r = parse(MUSIC).unwrap();
        assert_eq!(r.duration_ms(), 200_500);
        assert_eq!(r.position_ms(), 30_000);
        assert!(!r.is_playing());
    }

    #[test]
    fn now_playing_is_honest_about_artwork_and_clamps_position() {
        let np = parse(SPOTIFY).unwrap().to_now_playing();
        assert_eq!(np.title, "Contoh Lagu");
        assert_eq!(np.art, None);
        assert!(!np.lyr);
        let mut r = parse(MUSIC).unwrap();
        r.position = 999.0;
        assert_eq!(r.to_now_playing().pos, 200_500);
    }

    #[test]
    fn nothing_playing_and_garbage_are_none() {
        for s in ["null", "", "  \n", "bukan json", "{\"app\":1}"] {
            assert_eq!(parse(s), None, "{s:?}");
        }
    }

    #[test]
    fn command_scripts_only_target_known_players_and_carry_numbers_only() {
        assert_eq!(
            command_script("Spotify", &Command::Toggle).unwrap(),
            "var a = Application('Spotify'); a.playpause();"
        );
        assert_eq!(
            command_script("Music", &Command::SeekMs(90_500)).unwrap(),
            "var a = Application('Music'); a.playerPosition = 90.5;"
        );
        assert!(command_script("Music", &Command::Volume(2.0))
            .unwrap()
            .contains("soundVolume = 100"));
        assert!(command_script("Music", &Command::Volume(-1.0))
            .unwrap()
            .contains("soundVolume = 0"));
        // Nama aplikasi sembarang (mis. injeksi) ditolak.
        assert_eq!(command_script("Spotify'); evil();//", &Command::Play), None);
    }

    #[test]
    fn query_script_never_launches_apps_that_are_not_running() {
        assert!(QUERY_SCRIPT.contains("running()"));
        let run_pos = QUERY_SCRIPT.find("running()").unwrap();
        let state_pos = QUERY_SCRIPT.find("playerState").unwrap();
        assert!(
            run_pos < state_pos,
            "cek running() harus mendahului akses properti"
        );
    }
}
