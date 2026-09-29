//! Penyimpanan lokal probe: device_id, token, dan **kunci publik host yang dipin**.
//!
//! Bila kunci host berubah, probe menolak sambungan dan menjelaskan kenapa — ia tidak pernah
//! diam-diam pairing ulang, karena perubahan kunci bisa berarti host palsu.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;
use tab_protocol::noise::hex;
use tab_protocol::Id16;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HostRecord {
    pub token: String,
    pub public_key: String,
    pub addr: String,
    pub name: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ProbeStore {
    pub device_id: Option<String>,
    pub hosts: BTreeMap<String, HostRecord>,
}

pub fn default_path() -> PathBuf {
    let base = std::env::var_os("APPDATA")
        .or_else(|| std::env::var_os("XDG_CONFIG_HOME"))
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("tab-probe").join("probe.json")
}

pub fn from_hex<const N: usize>(s: &str) -> Option<[u8; N]> {
    if s.len() != N * 2 {
        return None;
    }
    let mut out = [0u8; N];
    for (i, b) in out.iter_mut().enumerate() {
        *b = u8::from_str_radix(s.get(i * 2..i * 2 + 2)?, 16).ok()?;
    }
    Some(out)
}

impl ProbeStore {
    pub fn load(path: &PathBuf) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &PathBuf) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(path, serde_json::to_string_pretty(self)?)
    }

    /// device_id dibuat sekali lalu dipakai terus.
    pub fn device_id(&mut self) -> Id16 {
        if let Some(id) = self.device_id.as_deref().and_then(Id16::from_hex) {
            return id;
        }
        let id = crate::random_device_id();
        self.device_id = Some(id.to_hex());
        id
    }

    pub fn remember(&mut self, hid: Id16, rec: HostRecord) {
        self.hosts.insert(hid.to_hex(), rec);
    }

    pub fn host(&self, hid: Id16) -> Option<&HostRecord> {
        self.hosts.get(&hid.to_hex())
    }
}

pub fn record(token: &[u8; 32], pk: &[u8; 32], addr: std::net::SocketAddr, name: &str) -> HostRecord {
    HostRecord {
        token: hex(token),
        public_key: hex(pk),
        addr: addr.to_string(),
        name: name.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_id_is_stable_across_reload() {
        let dir = std::env::temp_dir().join(format!("tab-probe-{}", std::process::id()));
        let path = dir.join("p.json");
        let mut s = ProbeStore::default();
        let id = s.device_id();
        s.save(&path).unwrap();
        let mut again = ProbeStore::load(&path);
        assert_eq!(again.device_id(), id);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn hex_roundtrip_and_rejects_bad_length() {
        let v = [7u8; 32];
        assert_eq!(from_hex::<32>(&hex(&v)), Some(v));
        assert_eq!(from_hex::<32>("abcd"), None);
        assert_eq!(from_hex::<4>("zzzzzzzz"), None);
    }
}
