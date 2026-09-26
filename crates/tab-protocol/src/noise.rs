//! Handshake dan sesi terenkripsi.
//!
//! Dua pola dipakai: `NK` untuk perangkat baru (host terautentikasi oleh static key dari
//! discovery, PIN yang menutup celah discovery palsu) dan `NKpsk2` untuk resume (token
//! diikat ke handshake, tidak pernah dikirim sebagai payload).

use crate::message::Message;
use crate::{MAX_PLAINTEXT, NOISE_PAIR, NOISE_RESUME, TOKEN_LEN};
use blake2::{Blake2s256, Digest};
use snow::{Builder, HandshakeState, TransportState};
use strum::VariantNames;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum NoiseError {
    #[error("noise: {0}")]
    Noise(#[from] snow::Error),
    #[error("token harus {TOKEN_LEN} byte, diterima {0}")]
    TokenLength(usize),
    #[error("handshake belum selesai")]
    HandshakeIncomplete,
    #[error("payload {size} byte melebihi batas {limit} byte")]
    TooLarge { size: usize, limit: usize },
    #[error("payload rusak: {0}")]
    Malformed(String),
}

/// Kunci statis jangka panjang host. Disimpan di keyring OS, bukan di file konfigurasi.
#[derive(Clone)]
pub struct StaticKeypair {
    pub private: [u8; 32],
    pub public: [u8; 32],
}

impl std::fmt::Debug for StaticKeypair {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Kunci privat tidak pernah ikut ke log.
        f.debug_struct("StaticKeypair")
            .field("public", &hex(&self.public))
            .finish_non_exhaustive()
    }
}

pub fn generate_static() -> Result<StaticKeypair, NoiseError> {
    let kp = Builder::new(NOISE_PAIR.parse()?).generate_keypair()?;
    let mut private = [0u8; 32];
    let mut public = [0u8; 32];
    private.copy_from_slice(&kp.private);
    public.copy_from_slice(&kp.public);
    Ok(StaticKeypair { private, public })
}

/// Fingerprint yang disiarkan lewat discovery dan disematkan client saat pairing.
pub fn fingerprint(public: &[u8; 32]) -> [u8; 32] {
    let mut out = [0u8; 32];
    out.copy_from_slice(&Blake2s256::digest(public));
    out
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn check_token(token: &[u8]) -> Result<(), NoiseError> {
    if token.len() != TOKEN_LEN {
        return Err(NoiseError::TokenLength(token.len()));
    }
    Ok(())
}

/// Handshake yang sedang berjalan.
pub struct Handshake {
    hs: HandshakeState,
    buf: Vec<u8>,
}

impl Handshake {
    pub fn client_pairing(host_public: &[u8; 32]) -> Result<Self, NoiseError> {
        let hs = Builder::new(NOISE_PAIR.parse()?)
            .remote_public_key(host_public)
            .build_initiator()?;
        Ok(Self::wrap(hs))
    }

    pub fn host_pairing(local_private: &[u8; 32]) -> Result<Self, NoiseError> {
        let hs = Builder::new(NOISE_PAIR.parse()?)
            .local_private_key(local_private)
            .build_responder()?;
        Ok(Self::wrap(hs))
    }

    pub fn client_resume(host_public: &[u8; 32], token: &[u8]) -> Result<Self, NoiseError> {
        check_token(token)?;
        let hs = Builder::new(NOISE_RESUME.parse()?)
            .remote_public_key(host_public)
            .psk(2, token)
            .build_initiator()?;
        Ok(Self::wrap(hs))
    }

    pub fn host_resume(local_private: &[u8; 32], token: &[u8]) -> Result<Self, NoiseError> {
        check_token(token)?;
        let hs = Builder::new(NOISE_RESUME.parse()?)
            .local_private_key(local_private)
            .psk(2, token)
            .build_responder()?;
        Ok(Self::wrap(hs))
    }

    fn wrap(hs: HandshakeState) -> Self {
        Self {
            hs,
            buf: vec![0u8; crate::MAX_FRAME],
        }
    }

    /// Hasilkan pesan handshake berikutnya untuk dikirim sebagai satu frame.
    pub fn next_message(&mut self) -> Result<Vec<u8>, NoiseError> {
        let n = self.hs.write_message(&[], &mut self.buf)?;
        Ok(self.buf[..n].to_vec())
    }

    /// Terima satu pesan handshake dari pihak lain.
    pub fn read_message(&mut self, frame: &[u8]) -> Result<(), NoiseError> {
        let mut payload = vec![0u8; crate::MAX_FRAME];
        self.hs.read_message(frame, &mut payload)?;
        Ok(())
    }

    pub fn is_finished(&self) -> bool {
        self.hs.is_handshake_finished()
    }

    pub fn into_session(self) -> Result<Session, NoiseError> {
        if !self.hs.is_handshake_finished() {
            return Err(NoiseError::HandshakeIncomplete);
        }
        Ok(Session {
            ts: self.hs.into_transport_mode()?,
            buf: self.buf,
        })
    }
}

/// Hasil dekripsi satu frame.
#[derive(Debug, Clone, PartialEq)]
pub enum Decoded {
    Known(Message),
    /// Tipe pesan dari versi yang lebih baru. **Diabaikan**, bukan error — inilah yang
    /// membuat host dan client versi berbeda tetap bisa berjalan bersama.
    Unknown(String),
}

#[derive(serde::Deserialize)]
struct TagProbe {
    t: String,
}

pub struct Session {
    ts: TransportState,
    buf: Vec<u8>,
}

impl Session {
    pub fn encrypt(&mut self, msg: &Message) -> Result<Vec<u8>, NoiseError> {
        let mut plain = Vec::with_capacity(256);
        ciborium::into_writer(msg, &mut plain).map_err(|e| NoiseError::Malformed(e.to_string()))?;
        if plain.len() > MAX_PLAINTEXT {
            return Err(NoiseError::TooLarge {
                size: plain.len(),
                limit: MAX_PLAINTEXT,
            });
        }
        let n = self.ts.write_message(&plain, &mut self.buf)?;
        Ok(self.buf[..n].to_vec())
    }

    /// Enkripsi payload CBOR yang sudah jadi. Dipakai untuk memancarkan pesan dari versi
    /// protokol lain saat menguji forward-compat, dan untuk payload yang sudah diserialisasi.
    pub fn encrypt_raw(&mut self, plain: &[u8]) -> Result<Vec<u8>, NoiseError> {
        if plain.len() > MAX_PLAINTEXT {
            return Err(NoiseError::TooLarge {
                size: plain.len(),
                limit: MAX_PLAINTEXT,
            });
        }
        let n = self.ts.write_message(plain, &mut self.buf)?;
        Ok(self.buf[..n].to_vec())
    }

    pub fn decrypt(&mut self, frame: &[u8]) -> Result<Decoded, NoiseError> {
        let mut plain = vec![0u8; crate::MAX_FRAME];
        let n = self.ts.read_message(frame, &mut plain)?;
        let plain = &plain[..n];
        match ciborium::from_reader::<Message, _>(plain) {
            Ok(msg) => Ok(Decoded::Known(msg)),
            Err(err) => match ciborium::from_reader::<TagProbe, _>(plain) {
                // Tag dikenal tetapi isinya tidak cocok: itu benar-benar rusak.
                Ok(probe) if Message::VARIANTS.contains(&probe.t.as_str()) => {
                    Err(NoiseError::Malformed(err.to_string()))
                }
                Ok(probe) => Ok(Decoded::Unknown(probe.t)),
                Err(_) => Err(NoiseError::Malformed(err.to_string())),
            },
        }
    }
}
