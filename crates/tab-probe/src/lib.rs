//! Client Tab berbasis library. Dipakai oleh CLI `probe` dan oleh test integrasi `tab-host`,
//! sehingga jalur pairing/resume yang diuji sama persis dengan yang dijalankan alat ukur.

pub mod discover;
pub mod stats;
pub mod store;

use rand::{rngs::OsRng, RngCore};
use std::net::SocketAddr;
use std::time::Duration;
use tab_protocol::frame::{read_frame, FrameWriter};
use tab_protocol::message::*;
use tab_protocol::noise::{fingerprint, Decoded, Handshake, NoiseError, Session};
use tab_protocol::preamble::{self, Intent};
use tab_protocol::{Id16, PROTOCOL_VERSION};
use thiserror::Error;
use tokio::net::tcp::OwnedWriteHalf;
use tokio::net::TcpStream;
use tokio::sync::mpsc;

#[derive(Debug, Error)]
pub enum ClientError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("noise: {0}")]
    Noise(#[from] NoiseError),
    #[error("koneksi ditutup oleh host")]
    Closed,
    #[error("timeout")]
    Timeout,
    #[error("frame: {0}")]
    Frame(#[from] tab_protocol::frame::FrameError),
    #[error("kunci publik host tidak cocok dengan fingerprint yang disiarkan")]
    FingerprintMismatch,
    #[error("respons tak terduga: {0}")]
    Unexpected(String),
}

/// Bukti identitas untuk koneksi ini.
#[derive(Clone)]
pub enum Credential {
    /// Perangkat baru: `Noise_NK`, lalu PIN.
    Pair,
    /// Perangkat yang sudah dipasangkan: `Noise_NKpsk2` dengan token.
    Resume([u8; 32]),
}

#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub device: Id16,
    pub name: String,
    pub plat: String,
}

impl DeviceInfo {
    pub fn probe(device: Id16) -> Self {
        Self {
            device,
            name: "tab-probe".into(),
            plat: "probe".into(),
        }
    }
}

#[derive(Debug)]
pub enum Outcome {
    Welcome(Welcome),
    PairRequired { ttl_ms: u32 },
    Refused(ProtoError),
}

#[derive(Debug)]
pub enum PinOutcome {
    Paired { token: [u8; 32], welcome: Welcome },
    Invalid { attempts_left: u8 },
    Refused(ProtoError),
}

pub struct Client {
    noise: Session,
    wr: OwnedWriteHalf,
    fw: FrameWriter,
    frames: mpsc::Receiver<Vec<u8>>,
    pub info: DeviceInfo,
}

/// Verifikasi `BLAKE2s(pk) == fp`. Wajib dipanggil sebelum kunci dari discovery dipakai.
pub fn check_fingerprint(pk: &[u8; 32], fp: &[u8]) -> Result<(), ClientError> {
    if fingerprint(pk).as_slice() == fp {
        Ok(())
    } else {
        Err(ClientError::FingerprintMismatch)
    }
}

impl Client {
    /// Buka koneksi + handshake Noise. `host_pub` adalah kunci yang sudah dipin.
    pub async fn connect(
        addr: SocketAddr,
        host_pub: &[u8; 32],
        info: DeviceInfo,
        cred: Credential,
    ) -> Result<Self, ClientError> {
        let mut stream = tokio::time::timeout(Duration::from_secs(5), TcpStream::connect(addr))
            .await
            .map_err(|_| ClientError::Timeout)??;
        stream.set_nodelay(true)?;
        let mut fw = FrameWriter::new();

        let (intent, mut hs) = match &cred {
            Credential::Pair => (Intent::Pair, Handshake::client_pairing(host_pub)?),
            Credential::Resume(t) => (Intent::Resume, Handshake::client_resume(host_pub, t)?),
        };
        fw.write(&mut stream, &preamble::encode(intent, info.device))
            .await?;
        fw.write(&mut stream, &hs.next_message()?).await?;

        let mut buf = Vec::new();
        tokio::time::timeout(Duration::from_secs(5), read_frame(&mut stream, &mut buf))
            .await
            .map_err(|_| ClientError::Timeout)?
            .map_err(|_| ClientError::Closed)?;
        hs.read_message(&buf)?;
        let noise = hs.into_session()?;

        let (mut rd, wr) = stream.into_split();
        let (tx, frames) = mpsc::channel(256);
        tokio::spawn(async move {
            let mut buf = Vec::new();
            while read_frame(&mut rd, &mut buf).await.is_ok() {
                if tx.send(buf.clone()).await.is_err() {
                    break;
                }
            }
        });

        Ok(Self {
            noise,
            wr,
            fw,
            frames,
            info,
        })
    }

    pub async fn send(&mut self, msg: &Message) -> Result<(), ClientError> {
        let ct = self.noise.encrypt(msg)?;
        self.fw.write(&mut self.wr, &ct).await?;
        Ok(())
    }

    /// Pesan berikutnya (pesan tak dikenal dilewati). Aman dibatalkan di dalam `select!`.
    pub async fn recv(&mut self) -> Result<Message, ClientError> {
        loop {
            let frame = self.frames.recv().await.ok_or(ClientError::Closed)?;
            match self.noise.decrypt(&frame)? {
                Decoded::Known(m) => return Ok(m),
                Decoded::Unknown(_) => continue,
            }
        }
    }

    pub async fn recv_timeout(&mut self, d: Duration) -> Result<Message, ClientError> {
        tokio::time::timeout(d, self.recv())
            .await
            .map_err(|_| ClientError::Timeout)?
    }

    /// Kirim Hello dan baca jawaban pertama.
    pub async fn hello(&mut self) -> Result<Outcome, ClientError> {
        let hello = Message::Hello(Hello {
            pv: PROTOCOL_VERSION,
            dev: self.info.device,
            name: self.info.name.clone(),
            plat: self.info.plat.clone(),
            platv: std::env::consts::OS.into(),
            app: env!("CARGO_PKG_VERSION").into(),
            scr: Screen {
                w: 1080,
                h: 2400,
                dpi: 2.6,
            },
        });
        self.send(&hello).await?;
        match self.recv_timeout(Duration::from_secs(5)).await? {
            Message::Welcome(w) => Ok(Outcome::Welcome(w)),
            Message::PairRequired(p) => Ok(Outcome::PairRequired { ttl_ms: p.ttl_ms }),
            Message::Error(e) => Ok(Outcome::Refused(e)),
            other => Err(ClientError::Unexpected(format!("{other:?}"))),
        }
    }

    pub async fn submit_pin(&mut self, pin: &str) -> Result<PinOutcome, ClientError> {
        self.send(&Message::PairRequest(PairRequest { pin: pin.into() }))
            .await?;
        match self.recv_timeout(Duration::from_secs(5)).await? {
            Message::PairOk(ok) => {
                let token: [u8; 32] = ok
                    .tok
                    .as_ref()
                    .try_into()
                    .map_err(|_| ClientError::Unexpected("token bukan 32 byte".into()))?;
                match self.recv_timeout(Duration::from_secs(5)).await? {
                    Message::Welcome(welcome) => Ok(PinOutcome::Paired { token, welcome }),
                    other => Err(ClientError::Unexpected(format!("{other:?}"))),
                }
            }
            Message::Error(e) if e.c == ErrorCode::PinInvalid => Ok(PinOutcome::Invalid {
                attempts_left: e.attempts_left.unwrap_or(0),
            }),
            Message::Error(e) => Ok(PinOutcome::Refused(e)),
            other => Err(ClientError::Unexpected(format!("{other:?}"))),
        }
    }
}

pub fn random_device_id() -> Id16 {
    let mut b = [0u8; 16];
    OsRng.fill_bytes(&mut b);
    Id16(b)
}

pub fn micros_since(start: std::time::Instant) -> u64 {
    start.elapsed().as_micros() as u64
}

impl Client {
    /// Kirim payload CBOR mentah (untuk menguji pesan dari versi protokol lain).
    pub async fn send_raw(&mut self, plain: &[u8]) -> Result<(), ClientError> {
        let ct = self.noise.encrypt_raw(plain)?;
        self.fw.write(&mut self.wr, &ct).await?;
        Ok(())
    }
}
