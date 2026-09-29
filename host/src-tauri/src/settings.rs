//! Setelan host yang disimpan di folder konfigurasi aplikasi (bukan rahasia; token dan kunci
//! ada di keyring OS).

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Settings {
    /// Pengali sensitivitas pointer (dipakai semua perangkat).
    pub sens: f32,
    /// Scroll natural: konten mengikuti jari.
    pub natural: bool,
    /// Folder berisi file `.lrc` milik user; `None` = lirik mati.
    pub lyrics_dir: Option<String>,
    /// Integrasi OBS (obs-websocket v5). Password disimpan di keyring, bukan di berkas ini.
    #[serde(default)]
    pub obs_enabled: bool,
    #[serde(default = "default_obs_host")]
    pub obs_host: String,
    #[serde(default = "default_obs_port")]
    pub obs_port: u16,
}

fn default_obs_host() -> String {
    "127.0.0.1".into()
}

fn default_obs_port() -> u16 {
    4455
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            sens: 1.0,
            natural: true,
            lyrics_dir: None,
            obs_enabled: false,
            obs_host: default_obs_host(),
            obs_port: default_obs_port(),
        }
    }
}

impl Settings {
    /// File rusak atau tidak ada → nilai bawaan; aplikasi tetap bisa dibuka.
    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|t| serde_json::from_str::<Settings>(&t).ok())
            .map(Self::sanitized)
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_string_pretty(self)?)?;
        std::fs::rename(tmp, path)
    }

    pub fn sanitized(mut self) -> Self {
        self.sens = if self.sens.is_finite() {
            self.sens.clamp(0.1, 5.0)
        } else {
            1.0
        };
        self.lyrics_dir = self.lyrics_dir.filter(|d| !d.trim().is_empty());
        self.obs_host = self.obs_host.trim().to_owned();
        if self.obs_host.is_empty() {
            self.obs_host = default_obs_host();
        }
        if self.obs_port == 0 {
            self.obs_port = default_obs_port();
        }
        self
    }

    pub fn lyrics_path(&self) -> Option<PathBuf> {
        self.lyrics_dir.as_ref().map(PathBuf::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_sanitize() {
        let dir = std::env::temp_dir().join(format!("tab-settings-{}", std::process::id()));
        let path = dir.join("settings.json");
        let s = Settings {
            sens: 99.0,
            natural: false,
            lyrics_dir: Some("  ".into()),
            ..Settings::default()
        }
        .sanitized();
        assert_eq!(s.sens, 5.0);
        assert!(s.lyrics_dir.is_none());
        s.save(&path).unwrap();
        assert_eq!(Settings::load(&path), s);
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn corrupt_file_falls_back_to_defaults() {
        let dir = std::env::temp_dir().join(format!("tab-settings-bad-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        std::fs::write(&path, "{bukan json").unwrap();
        assert_eq!(Settings::load(&path), Settings::default());
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn non_finite_sensitivity_is_repaired() {
        let s = Settings {
            sens: f32::NAN,
            ..Settings::default()
        }
        .sanitized();
        assert_eq!(s.sens, 1.0);
    }
}
