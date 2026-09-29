//! Penyimpanan kunci statis host dan token perangkat.
//!
//! Versi produksi memakai keyring OS (Keychain / Windows Credential Manager) sehingga token
//! dan kunci privat tidak pernah berada di file biasa. `MemoryTokenStore` untuk test.

use tab_protocol::noise::{self, StaticKeypair};
use tab_protocol::Id16;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("penyimpanan: {0}")]
    Backend(String),
    #[error("data tersimpan rusak: {0}")]
    Corrupt(String),
}

#[derive(Clone, PartialEq, Eq)]
pub struct DeviceEntry {
    pub device: Id16,
    pub token: [u8; 32],
    pub name: String,
    pub platform: String,
    /// Detik sejak epoch Unix.
    pub paired_at: u64,
    pub last_seen: u64,
}

impl std::fmt::Debug for DeviceEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Token tidak pernah ikut ke log.
        f.debug_struct("DeviceEntry")
            .field("device", &self.device)
            .field("name", &self.name)
            .field("platform", &self.platform)
            .field("paired_at", &self.paired_at)
            .field("last_seen", &self.last_seen)
            .finish_non_exhaustive()
    }
}

pub trait TokenStore: Send + Sync {
    /// Kunci statis host; dibuat dan disimpan saat pertama kali diminta.
    fn keypair(&self) -> Result<StaticKeypair, StoreError>;
    fn devices(&self) -> Result<Vec<DeviceEntry>, StoreError>;
    fn upsert(&mut self, entry: &DeviceEntry) -> Result<(), StoreError>;
    fn remove(&mut self, device: Id16) -> Result<(), StoreError>;
}

pub fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Untuk test dan mode tanpa persistensi.
pub struct MemoryTokenStore {
    keys: StaticKeypair,
    devices: Vec<DeviceEntry>,
}

impl MemoryTokenStore {
    pub fn new() -> Self {
        Self {
            keys: noise::generate_static().expect("generate_static"),
            devices: Vec::new(),
        }
    }
}

impl Default for MemoryTokenStore {
    fn default() -> Self {
        Self::new()
    }
}

impl TokenStore for MemoryTokenStore {
    fn keypair(&self) -> Result<StaticKeypair, StoreError> {
        Ok(self.keys.clone())
    }
    fn devices(&self) -> Result<Vec<DeviceEntry>, StoreError> {
        Ok(self.devices.clone())
    }
    fn upsert(&mut self, entry: &DeviceEntry) -> Result<(), StoreError> {
        self.devices.retain(|d| d.device != entry.device);
        self.devices.push(entry.clone());
        Ok(())
    }
    fn remove(&mut self, device: Id16) -> Result<(), StoreError> {
        self.devices.retain(|d| d.device != device);
        Ok(())
    }
}

#[cfg(any(windows, target_os = "macos"))]
pub use keyring_store::KeyringTokenStore;

#[cfg(any(windows, target_os = "macos"))]
mod keyring_store {
    use super::*;
    use keyring::Entry;

    const SERVICE: &str = "app.tab.host";
    const KEYPAIR: &str = "static-keypair";
    const INDEX: &str = "device-index";

    /// Satu entri keyring per perangkat (Credential Manager membatasi ~2,5 KB per entri, jadi
    /// tidak mungkin menaruh semua perangkat dalam satu blob), ditambah satu entri indeks.
    pub struct KeyringTokenStore;

    impl KeyringTokenStore {
        pub fn new() -> Self {
            Self
        }
    }

    impl Default for KeyringTokenStore {
        fn default() -> Self {
            Self::new()
        }
    }

    fn entry(user: &str) -> Result<Entry, StoreError> {
        Entry::new(SERVICE, user).map_err(|e| StoreError::Backend(e.to_string()))
    }

    fn read(user: &str) -> Result<Option<Vec<u8>>, StoreError> {
        match entry(user)?.get_secret() {
            Ok(v) => Ok(Some(v)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(StoreError::Backend(e.to_string())),
        }
    }

    fn write(user: &str, value: &[u8]) -> Result<(), StoreError> {
        entry(user)?
            .set_secret(value)
            .map_err(|e| StoreError::Backend(e.to_string()))
    }

    fn delete(user: &str) -> Result<(), StoreError> {
        match entry(user)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(StoreError::Backend(e.to_string())),
        }
    }

    fn device_user(id: Id16) -> String {
        format!("device:{}", id.to_hex())
    }

    fn read_index() -> Result<Vec<Id16>, StoreError> {
        let Some(raw) = read(INDEX)? else {
            return Ok(Vec::new());
        };
        let text = String::from_utf8(raw).map_err(|e| StoreError::Corrupt(e.to_string()))?;
        Ok(text.split(',').filter_map(Id16::from_hex).collect())
    }

    fn write_index(ids: &[Id16]) -> Result<(), StoreError> {
        let text = ids
            .iter()
            .map(|i| i.to_hex())
            .collect::<Vec<_>>()
            .join(",");
        write(INDEX, text.as_bytes())
    }

    #[derive(serde::Serialize, serde::Deserialize)]
    struct Stored {
        token: String,
        name: String,
        platform: String,
        paired_at: u64,
        last_seen: u64,
    }

    impl TokenStore for KeyringTokenStore {
        fn keypair(&self) -> Result<StaticKeypair, StoreError> {
            if let Some(raw) = read(KEYPAIR)? {
                if raw.len() == 64 {
                    let mut private = [0u8; 32];
                    let mut public = [0u8; 32];
                    private.copy_from_slice(&raw[..32]);
                    public.copy_from_slice(&raw[32..]);
                    return Ok(StaticKeypair { private, public });
                }
                return Err(StoreError::Corrupt("panjang kunci tidak sah".into()));
            }
            let kp = noise::generate_static().map_err(|e| StoreError::Backend(e.to_string()))?;
            let mut raw = Vec::with_capacity(64);
            raw.extend_from_slice(&kp.private);
            raw.extend_from_slice(&kp.public);
            write(KEYPAIR, &raw)?;
            Ok(kp)
        }

        fn devices(&self) -> Result<Vec<DeviceEntry>, StoreError> {
            let mut out = Vec::new();
            for id in read_index()? {
                let Some(raw) = read(&device_user(id))? else {
                    continue;
                };
                let s: Stored = serde_json::from_slice(&raw)
                    .map_err(|e| StoreError::Corrupt(e.to_string()))?;
                let token: [u8; 32] = decode_hex32(&s.token)
                    .ok_or_else(|| StoreError::Corrupt("token tidak sah".into()))?;
                out.push(DeviceEntry {
                    device: id,
                    token,
                    name: s.name,
                    platform: s.platform,
                    paired_at: s.paired_at,
                    last_seen: s.last_seen,
                });
            }
            Ok(out)
        }

        fn upsert(&mut self, e: &DeviceEntry) -> Result<(), StoreError> {
            let stored = Stored {
                token: noise::hex(&e.token),
                name: e.name.clone(),
                platform: e.platform.clone(),
                paired_at: e.paired_at,
                last_seen: e.last_seen,
            };
            let raw =
                serde_json::to_vec(&stored).map_err(|e| StoreError::Corrupt(e.to_string()))?;
            write(&device_user(e.device), &raw)?;
            let mut ids = read_index()?;
            if !ids.contains(&e.device) {
                ids.push(e.device);
                write_index(&ids)?;
            }
            Ok(())
        }

        fn remove(&mut self, device: Id16) -> Result<(), StoreError> {
            delete(&device_user(device))?;
            let mut ids = read_index()?;
            ids.retain(|i| *i != device);
            write_index(&ids)
        }
    }

    fn decode_hex32(s: &str) -> Option<[u8; 32]> {
        if s.len() != 64 {
            return None;
        }
        let mut out = [0u8; 32];
        for (i, b) in out.iter_mut().enumerate() {
            *b = u8::from_str_radix(s.get(i * 2..i * 2 + 2)?, 16).ok()?;
        }
        Some(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_store_upsert_replaces_and_remove_forgets() {
        let mut s = MemoryTokenStore::new();
        let dev = Id16::random();
        let mut e = DeviceEntry {
            device: dev,
            token: [1; 32],
            name: "HP".into(),
            platform: "android".into(),
            paired_at: 1,
            last_seen: 1,
        };
        s.upsert(&e).unwrap();
        e.token = [2; 32];
        s.upsert(&e).unwrap();
        let all = s.devices().unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].token, [2; 32]);
        s.remove(dev).unwrap();
        assert!(s.devices().unwrap().is_empty());
    }

    #[test]
    fn debug_never_prints_token() {
        let e = DeviceEntry {
            device: Id16::random(),
            token: [0xAB; 32],
            name: "x".into(),
            platform: "android".into(),
            paired_at: 0,
            last_seen: 0,
        };
        let dump = format!("{e:?}");
        assert!(!dump.contains("171") && !dump.to_lowercase().contains("abab"));
    }
}
