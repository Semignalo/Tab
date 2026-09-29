//! Aksi deck dan profil persisten.
//!
//! Definisi aksi hidup **hanya di host**. Ke perangkat hanya dikirim `action_id` (lihat
//! [`to_wire`]), sehingga perangkat yang dipasangkan bisa memicu aksi milik user tetapi tidak
//! bisa menyusun perintah sendiri.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tab_input::PlatformInput;
use tab_protocol::message::{DeckButton, DeckProfile};
use thiserror::Error;

pub mod obs;
mod runner;
pub use obs::{ObsConfig, ObsError, ObsHandle};
pub use runner::SystemRunner;

#[derive(Debug, Error)]
pub enum ActionError {
    #[error("aksi gagal: {0}")]
    Failed(String),
    #[error("tidak didukung: {0}")]
    Unsupported(String),
    #[error("profil tidak valid: {0}")]
    Invalid(String),
    #[error("penyimpanan profil: {0}")]
    Store(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    Hotkey { keys: Vec<String> },
    LaunchApp { path: String },
    OpenPath { path: String },
    OpenUrl { url: String },
    MediaKey { key: String },
    ObsScene { scene: String },
    Multi { steps: Vec<Action> },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActionDef {
    pub id: String,
    pub action: Action,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ButtonDef {
    pub index: u16,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    pub action_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub cols: u8,
    pub rows: u8,
    pub buttons: Vec<ButtonDef>,
    pub actions: Vec<ActionDef>,
}

impl Profile {
    /// Pemeriksaan yang harus lolos sebelum profil disimpan atau dipakai.
    pub fn validate(&self) -> Result<(), ActionError> {
        check_id(&self.id)?;
        if self.cols == 0 || self.rows == 0 || self.cols > 8 || self.rows > 8 {
            return Err(ActionError::Invalid("grid harus 1..8 kolom/baris".into()));
        }
        let cells = self.cols as u16 * self.rows as u16;
        for b in &self.buttons {
            if b.index >= cells {
                return Err(ActionError::Invalid(format!(
                    "tombol {} di luar grid",
                    b.index
                )));
            }
            if !self.actions.iter().any(|a| a.id == b.action_id) {
                return Err(ActionError::Invalid(format!(
                    "tombol {} merujuk aksi yang tidak ada: {}",
                    b.index, b.action_id
                )));
            }
        }
        Ok(())
    }

    pub fn action(&self, id: &str) -> Option<&Action> {
        self.actions.iter().find(|a| a.id == id).map(|a| &a.action)
    }
}

/// Id dipakai sebagai nama file, jadi hanya karakter aman yang diizinkan.
fn check_id(id: &str) -> Result<(), ActionError> {
    let ok = !id.is_empty()
        && id.len() <= 64
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if ok {
        Ok(())
    } else {
        Err(ActionError::Invalid(format!("id profil tidak sah: {id:?}")))
    }
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
/// `action_id`.
pub fn to_wire(profile: &Profile) -> DeckProfile {
    DeckProfile {
        id: profile.id.clone(),
        name: profile.name.clone(),
        cols: profile.cols,
        rows: profile.rows,
        btns: profile
            .buttons
            .iter()
            .map(|b| DeckButton {
                i: b.index,
                lbl: b.label.clone(),
                ic: b.icon.clone(),
                col: b.color.clone(),
                aid: b.action_id.clone(),
            })
            .collect(),
    }
}

/// Profil awal supaya deck langsung berguna setelah pairing pertama.
pub fn starter_profile() -> Profile {
    let media = |id: &str, key: &str| ActionDef {
        id: id.into(),
        action: Action::MediaKey { key: key.into() },
    };
    let button = |index, label: &str, icon: &str, action_id: &str| ButtonDef {
        index,
        label: label.into(),
        icon: Some(icon.into()),
        color: None,
        action_id: action_id.into(),
    };
    Profile {
        id: "default".into(),
        name: "Media".into(),
        cols: 3,
        rows: 2,
        buttons: vec![
            button(0, "Sebelumnya", "skip_previous", "prev"),
            button(1, "Putar/Jeda", "play_pause", "play"),
            button(2, "Berikutnya", "skip_next", "next"),
            button(3, "Vol -", "volume_down", "vol_down"),
            button(4, "Mute", "volume_off", "mute"),
            button(5, "Vol +", "volume_up", "vol_up"),
        ],
        actions: vec![
            media("prev", "media_prev"),
            media("play", "media_play"),
            media("next", "media_next"),
            media("vol_down", "volume_down"),
            media("mute", "mute"),
            media("vol_up", "volume_up"),
        ],
    }
}

/// Pintasan umum ala Stream Deck. Modifier utama mengikuti OS: Ctrl di Windows, Cmd di macOS.
pub fn shortcuts_profile() -> Profile {
    let mac = cfg!(target_os = "macos");
    let m = if mac { "meta" } else { "ctrl" };
    let keys = |v: &[&str]| Action::Hotkey {
        keys: v.iter().map(|s| (*s).to_owned()).collect(),
    };
    let items: Vec<(&str, &str, Action)> = vec![
        ("Copy", "copy", keys(&[m, "c"])),
        ("Paste", "paste", keys(&[m, "v"])),
        ("Cut", "cut", keys(&[m, "x"])),
        ("Undo", "undo", keys(&[m, "z"])),
        (
            "Redo",
            "redo",
            if mac {
                keys(&["meta", "shift", "z"])
            } else {
                keys(&["ctrl", "y"])
            },
        ),
        ("Select all", "select_all", keys(&[m, "a"])),
        (
            "Screenshot",
            "screenshot",
            if mac {
                keys(&["meta", "shift", "4"])
            } else {
                keys(&["meta", "shift", "s"])
            },
        ),
        (
            "Kunci layar",
            "lock",
            if mac {
                keys(&["ctrl", "meta", "q"])
            } else {
                keys(&["meta", "l"])
            },
        ),
        (
            "Task Manager",
            "app",
            if mac {
                keys(&["meta", "alt", "esc"])
            } else {
                keys(&["ctrl", "shift", "esc"])
            },
        ),
        (
            "Desktop",
            "folder",
            if mac {
                keys(&["f11"])
            } else {
                keys(&["meta", "d"])
            },
        ),
        ("Tab baru", "browser", keys(&[m, "t"])),
        ("Tutup tab", "stop", keys(&[m, "w"])),
    ];
    let mut buttons = Vec::new();
    let mut actions = Vec::new();
    for (i, (label, icon, action)) in items.into_iter().enumerate() {
        let id = format!("b{i}");
        buttons.push(ButtonDef {
            index: i as u16,
            label: label.into(),
            icon: Some(icon.into()),
            color: None,
            action_id: id.clone(),
        });
        actions.push(ActionDef { id, action });
    }
    Profile {
        id: "pintasan".into(),
        name: "Pintasan".into(),
        cols: 4,
        rows: 3,
        buttons,
        actions,
    }
}

// ------------------------------------------------------------- penyimpanan

/// Satu file JSON per profil di sebuah folder.
pub struct JsonProfileStore {
    dir: PathBuf,
}

impl JsonProfileStore {
    /// Membuat folder bila belum ada dan menanam [`starter_profile`] bila kosong.
    pub fn open(dir: impl Into<PathBuf>) -> Result<Self, ActionError> {
        let dir = dir.into();
        std::fs::create_dir_all(&dir).map_err(|e| ActionError::Store(e.to_string()))?;
        let mut store = Self { dir };
        if store.list()?.is_empty() {
            store.save(&starter_profile())?;
        }
        // Profil pintasan ditanam sekali (penanda berkas), supaya yang sudah dihapus atau
        // diubah user tidak muncul lagi.
        let marker = store.dir.join(".seeded-shortcuts-v1");
        if !marker.exists() {
            if store.load("pintasan")?.is_none() {
                store.save(&shortcuts_profile())?;
            }
            let _ = std::fs::write(marker, b"");
        }
        Ok(store)
    }

    fn path(&self, id: &str) -> Result<PathBuf, ActionError> {
        check_id(id)?;
        Ok(self.dir.join(format!("{id}.json")))
    }
}

impl ProfileStore for JsonProfileStore {
    fn list(&self) -> Result<Vec<Profile>, ActionError> {
        let mut out = Vec::new();
        let rd = std::fs::read_dir(&self.dir).map_err(|e| ActionError::Store(e.to_string()))?;
        for entry in rd.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "json") {
                let text = std::fs::read_to_string(&path)
                    .map_err(|e| ActionError::Store(e.to_string()))?;
                // Satu file rusak tidak boleh membuat seluruh daftar profil hilang.
                match serde_json::from_str::<Profile>(&text) {
                    Ok(p) if p.validate().is_ok() => out.push(p),
                    _ => tracing::warn!("profil dilewati karena rusak: {}", path.display()),
                }
            }
        }
        out.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(out)
    }

    fn load(&self, id: &str) -> Result<Option<Profile>, ActionError> {
        let path = self.path(id)?;
        match std::fs::read_to_string(path) {
            Ok(text) => {
                let p: Profile =
                    serde_json::from_str(&text).map_err(|e| ActionError::Invalid(e.to_string()))?;
                p.validate()?;
                Ok(Some(p))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(ActionError::Store(e.to_string())),
        }
    }

    fn save(&mut self, profile: &Profile) -> Result<(), ActionError> {
        profile.validate()?;
        let path = self.path(&profile.id)?;
        let json =
            serde_json::to_string_pretty(profile).map_err(|e| ActionError::Store(e.to_string()))?;
        // Tulis ke file sementara lalu rename: crash di tengah tulis tidak merusak profil lama.
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, json).map_err(|e| ActionError::Store(e.to_string()))?;
        std::fs::rename(&tmp, &path).map_err(|e| ActionError::Store(e.to_string()))
    }

    fn delete(&mut self, id: &str) -> Result<(), ActionError> {
        match std::fs::remove_file(self.path(id)?) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(ActionError::Store(e.to_string())),
        }
    }
}

/// Untuk test dan mode tanpa disk.
#[derive(Default)]
pub struct MemoryProfileStore(Vec<Profile>);

impl MemoryProfileStore {
    pub fn with_starter() -> Self {
        Self(vec![starter_profile()])
    }
}

impl ProfileStore for MemoryProfileStore {
    fn list(&self) -> Result<Vec<Profile>, ActionError> {
        Ok(self.0.clone())
    }
    fn load(&self, id: &str) -> Result<Option<Profile>, ActionError> {
        Ok(self.0.iter().find(|p| p.id == id).cloned())
    }
    fn save(&mut self, profile: &Profile) -> Result<(), ActionError> {
        profile.validate()?;
        self.0.retain(|p| p.id != profile.id);
        self.0.push(profile.clone());
        Ok(())
    }
    fn delete(&mut self, id: &str) -> Result<(), ActionError> {
        self.0.retain(|p| p.id != id);
        Ok(())
    }
}

/// Runner yang hanya mencatat; dipakai test host.
#[derive(Default)]
pub struct RecordingRunner {
    pub ran: std::sync::Arc<std::sync::Mutex<Vec<Action>>>,
}

impl ActionRunner for RecordingRunner {
    fn run(&mut self, action: &Action) -> Result<(), ActionError> {
        self.ran.lock().unwrap().push(action.clone());
        Ok(())
    }
}

/// Runner sungguhan: hotkey lewat `PlatformInput`, sisanya lewat proses OS.
pub fn system_runner(input: Box<dyn PlatformInput>) -> SystemRunner {
    SystemRunner::new(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starter_profiles_are_valid() {
        starter_profile().validate().unwrap();
        shortcuts_profile().validate().unwrap();
    }

    #[test]
    fn wire_profile_never_carries_action_definitions() {
        let mut p = starter_profile();
        p.actions.push(ActionDef {
            id: "rahasia".into(),
            action: Action::LaunchApp {
                path: "C:/rahasia/app.exe".into(),
            },
        });
        p.buttons.push(ButtonDef {
            index: 5,
            label: "x".into(),
            icon: None,
            color: None,
            action_id: "rahasia".into(),
        });
        let wire = to_wire(&p);
        let mut bytes = Vec::new();
        ciborium_free_check(&wire, &mut bytes);
        let dump = String::from_utf8_lossy(&bytes);
        assert!(!dump.contains("app.exe") && !dump.contains("LaunchApp"));
        assert!(wire.btns.iter().any(|b| b.aid == "rahasia"));
    }

    /// Serialisasi JSON cukup untuk memastikan tidak ada field aksi yang ikut.
    fn ciborium_free_check(wire: &DeckProfile, out: &mut Vec<u8>) {
        *out = format!("{wire:?}").into_bytes();
    }

    #[test]
    fn rejects_bad_ids_and_dangling_actions() {
        let mut p = starter_profile();
        p.id = "../../etc/passwd".into();
        assert!(p.validate().is_err());

        let mut p = starter_profile();
        p.buttons[0].action_id = "tidak-ada".into();
        assert!(p.validate().is_err());

        let mut p = starter_profile();
        p.buttons[0].index = 99;
        assert!(p.validate().is_err());
    }

    #[test]
    fn json_store_roundtrip_seeds_and_survives_corrupt_file() {
        let dir = std::env::temp_dir().join(format!("tab-deck-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut store = JsonProfileStore::open(&dir).unwrap();
        assert_eq!(
            store.list().unwrap().len(),
            2,
            "profil awal + pintasan ditanam"
        );

        let mut p = starter_profile();
        p.id = "kerja".into();
        p.name = "Kerja".into();
        store.save(&p).unwrap();
        assert_eq!(store.load("kerja").unwrap().unwrap(), p);

        std::fs::write(dir.join("rusak.json"), "{bukan json").unwrap();
        assert_eq!(store.list().unwrap().len(), 3, "file rusak dilewati");

        store.delete("kerja").unwrap();
        assert!(store.load("kerja").unwrap().is_none());
        assert!(store.load("../x").is_err());
        std::fs::remove_dir_all(&dir).ok();
    }
}
