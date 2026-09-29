//! Preamble koneksi: frame pertama dari client, sebelum handshake Noise.
//!
//! Host perlu tahu *perangkat mana* yang datang sebelum bisa memilih pola dan token PSK
//! untuk `NKpsk2`, sementara pesan handshake pertama tidak membawa identitas apa pun.
//! `device_id` sudah disiarkan telanjang lewat discovery, jadi mengirimnya tanpa enkripsi
//! di sini tidak membuka informasi baru; token tetap hanya bekerja lewat handshake.

use crate::ids::Id16;

pub const PREAMBLE_LEN: usize = 17;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Intent {
    /// `Noise_NK` — perangkat belum punya token, PIN yang membuktikan kepemilikan.
    Pair = 0x01,
    /// `Noise_NKpsk2` — perangkat membawa token.
    Resume = 0x02,
}

pub fn encode(intent: Intent, device: Id16) -> [u8; PREAMBLE_LEN] {
    let mut out = [0u8; PREAMBLE_LEN];
    out[0] = intent as u8;
    out[1..].copy_from_slice(device.as_bytes());
    out
}

/// `None` bila panjang atau nilai `intent` tidak dikenal; pemanggil menutup koneksi.
pub fn decode(frame: &[u8]) -> Option<(Intent, Id16)> {
    if frame.len() != PREAMBLE_LEN {
        return None;
    }
    let intent = match frame[0] {
        0x01 => Intent::Pair,
        0x02 => Intent::Resume,
        _ => return None,
    };
    let mut id = [0u8; 16];
    id.copy_from_slice(&frame[1..]);
    Some((intent, Id16(id)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_rejects_garbage() {
        let dev = Id16::random();
        for intent in [Intent::Pair, Intent::Resume] {
            assert_eq!(decode(&encode(intent, dev)), Some((intent, dev)));
        }
        assert_eq!(decode(&[0x09; PREAMBLE_LEN]), None);
        assert_eq!(decode(&[0x01; 5]), None);
    }
}
