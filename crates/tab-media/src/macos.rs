//! Media macOS: JXA lewat `osascript` untuk Spotify dan Music.app.
//!
//! Pertama kali dipakai macOS menampilkan dialog izin "Automation" (Tab ingin mengontrol
//! Spotify/Music); tanpa izin `osascript` gagal dan sumber ini melaporkan "tidak ada yang
//! diputar" alih-alih error yang mengganggu.

use crate::jxa::{self, Command};
use crate::{MediaCaps, MediaError, MediaSource};
use std::process::Command as Proc;
use tab_protocol::message::{MediaCmd, NowPlaying};

pub struct MacMedia {
    /// Pemutar terakhir yang terlihat aktif; sasaran perintah transport.
    last_app: Option<String>,
}

impl MacMedia {
    pub fn new() -> Self {
        Self { last_app: None }
    }

    fn osascript(script: &str) -> Result<String, MediaError> {
        let out = Proc::new("osascript")
            .args(["-l", "JavaScript", "-e", script])
            .output()
            .map_err(|e| MediaError::Backend(e.to_string()))?;
        if !out.status.success() {
            return Err(MediaError::Backend(
                String::from_utf8_lossy(&out.stderr).trim().to_owned(),
            ));
        }
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    }

    fn send(&self, command: Command) -> Result<(), MediaError> {
        let app = self
            .last_app
            .as_deref()
            .ok_or_else(|| MediaError::Backend("tidak ada pemutar aktif".into()))?;
        let script = jxa::command_script(app, &command)
            .ok_or_else(|| MediaError::Unsupported(format!("pemutar {app}")))?;
        Self::osascript(&script).map(|_| ())
    }
}

impl Default for MacMedia {
    fn default() -> Self {
        Self::new()
    }
}

impl MediaSource for MacMedia {
    fn capabilities(&self) -> MediaCaps {
        MediaCaps {
            now_playing: true,
            seek: true,
            // Volume aplikasi pemutar (bukan volume sistem).
            volume: true,
        }
    }

    fn now_playing(&mut self) -> Result<Option<NowPlaying>, MediaError> {
        let out = match Self::osascript(jxa::QUERY_SCRIPT) {
            Ok(o) => o,
            // Tanpa izin Automation atau osascript bermasalah: perlakukan sebagai kosong.
            Err(_) => return Ok(None),
        };
        Ok(jxa::parse(&out).map(|r| {
            self.last_app = Some(r.app.clone());
            r.to_now_playing()
        }))
    }

    fn artwork(&mut self, _id: &str) -> Result<Option<Vec<u8>>, MediaError> {
        Ok(None)
    }

    fn command(&mut self, cmd: MediaCmd) -> Result<(), MediaError> {
        self.send(match cmd {
            MediaCmd::Play => Command::Play,
            MediaCmd::Pause => Command::Pause,
            MediaCmd::Toggle => Command::Toggle,
            MediaCmd::Next => Command::Next,
            MediaCmd::Prev => Command::Prev,
        })
    }

    fn seek(&mut self, ms: u64) -> Result<(), MediaError> {
        self.send(Command::SeekMs(ms))
    }

    fn volume(&mut self, v: f32) -> Result<(), MediaError> {
        self.send(Command::Volume(v))
    }
}
