//! Registry perangkat: pembungkus `TokenStore` yang juga memberi tahu sesi hidup saat sebuah
//! perangkat dicabut, supaya koneksi aktifnya langsung ditutup.

use crate::store::{now_secs, DeviceEntry, StoreError, TokenStore};
use std::sync::Mutex;
use tab_protocol::Id16;
use tokio::sync::broadcast;

pub struct Registry {
    store: Mutex<Box<dyn TokenStore>>,
    revoked: broadcast::Sender<Id16>,
}

impl Registry {
    pub fn new(store: Box<dyn TokenStore>) -> Self {
        let (revoked, _) = broadcast::channel(16);
        Self {
            store: Mutex::new(store),
            revoked,
        }
    }

    pub fn keypair(&self) -> Result<tab_protocol::noise::StaticKeypair, StoreError> {
        self.store.lock().unwrap().keypair()
    }

    pub fn devices(&self) -> Result<Vec<DeviceEntry>, StoreError> {
        self.store.lock().unwrap().devices()
    }

    pub fn find(&self, device: Id16) -> Option<DeviceEntry> {
        self.devices()
            .ok()?
            .into_iter()
            .find(|d| d.device == device)
    }

    pub fn is_known(&self, device: Id16) -> bool {
        self.find(device).is_some()
    }

    pub fn upsert(&self, entry: &DeviceEntry) -> Result<(), StoreError> {
        self.store.lock().unwrap().upsert(entry)
    }

    pub fn touch(&self, device: Id16) {
        if let Some(mut e) = self.find(device) {
            e.last_seen = now_secs();
            let _ = self.upsert(&e);
        }
    }

    /// Hapus token lalu umumkan ke semua sesi; sesi milik perangkat itu menutup dirinya.
    pub fn revoke(&self, device: Id16) -> Result<(), StoreError> {
        self.store.lock().unwrap().remove(device)?;
        let _ = self.revoked.send(device);
        Ok(())
    }

    pub fn subscribe_revoked(&self) -> broadcast::Receiver<Id16> {
        self.revoked.subscribe()
    }
}
