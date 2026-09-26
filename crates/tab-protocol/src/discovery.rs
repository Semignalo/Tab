//! Discovery UDP 4179.
//!
//! Datagram tidak terenkripsi, jadi isinya sengaja dibatasi pada hal yang aman diketahui
//! siapa pun di jaringan lokal: nama mesin, versi, port, dan fingerprint kunci publik host.

use crate::ids::Id16;
use crate::{MAX_DATAGRAM, PROTOCOL_MAX, PROTOCOL_MIN};
use serde::{Deserialize, Serialize};
use serde_bytes::ByteBuf;
use thiserror::Error;

pub const MAGIC: [u8; 4] = *b"TABD";
pub const WIRE_VERSION: u8 = 1;
pub const KIND_REQUEST: u8 = 0x01;
pub const KIND_RESPONSE: u8 = 0x02;

const HEADER_LEN: usize = 6;

#[derive(Debug, Error)]
pub enum DiscoveryError {
    /// Datagram bukan milik protokol ini. Pemanggil **membuangnya tanpa balasan**.
    #[error("datagram asing, diabaikan")]
    Foreign,
    #[error("datagram melebihi {MAX_DATAGRAM} byte")]
    TooLarge,
    #[error("payload rusak: {0}")]
    Malformed(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiscoverRequest {
    pub dev: Id16,
    pub pv: u16,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiscoverResponse {
    pub hid: Id16,
    pub name: String,
    pub os: String,
    pub osv: String,
    pub app: String,
    pub pmin: u16,
    pub pmax: u16,
    pub port: u16,
    /// BLAKE2s dari static public key X25519 host.
    pub fp: ByteBuf,
    /// `true` bila device_id pada permintaan sudah punya token di host ini.
    pub known: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Datagram {
    Request(DiscoverRequest),
    Response(DiscoverResponse),
}

fn encode(kind: u8, payload: &impl Serialize) -> Result<Vec<u8>, DiscoveryError> {
    let mut out = Vec::with_capacity(128);
    out.extend_from_slice(&MAGIC);
    out.push(WIRE_VERSION);
    out.push(kind);
    ciborium::into_writer(payload, &mut out)
        .map_err(|e| DiscoveryError::Malformed(e.to_string()))?;
    if out.len() > MAX_DATAGRAM {
        return Err(DiscoveryError::TooLarge);
    }
    Ok(out)
}

pub fn encode_request(req: &DiscoverRequest) -> Result<Vec<u8>, DiscoveryError> {
    encode(KIND_REQUEST, req)
}

pub fn encode_response(res: &DiscoverResponse) -> Result<Vec<u8>, DiscoveryError> {
    encode(KIND_RESPONSE, res)
}

pub fn decode(datagram: &[u8]) -> Result<Datagram, DiscoveryError> {
    if datagram.len() > MAX_DATAGRAM {
        return Err(DiscoveryError::TooLarge);
    }
    if datagram.len() <= HEADER_LEN || datagram[..4] != MAGIC || datagram[4] != WIRE_VERSION {
        return Err(DiscoveryError::Foreign);
    }
    let body = &datagram[HEADER_LEN..];
    match datagram[5] {
        KIND_REQUEST => ciborium::from_reader(body)
            .map(Datagram::Request)
            .map_err(|e| DiscoveryError::Malformed(e.to_string())),
        KIND_RESPONSE => ciborium::from_reader(body)
            .map(Datagram::Response)
            .map_err(|e| DiscoveryError::Malformed(e.to_string())),
        _ => Err(DiscoveryError::Foreign),
    }
}

/// Host hanya menjawab bila versi client berada dalam rentang yang dilayani.
pub fn should_answer(req: &DiscoverRequest) -> bool {
    (PROTOCOL_MIN..=PROTOCOL_MAX).contains(&req.pv)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_response() -> DiscoverResponse {
        DiscoverResponse {
            hid: Id16::random(),
            name: "MacBook Pro Stefanus".into(),
            os: "macos".into(),
            osv: "15.6".into(),
            app: "0.1.0".into(),
            pmin: PROTOCOL_MIN,
            pmax: PROTOCOL_MAX,
            port: crate::SESSION_PORT,
            fp: ByteBuf::from(vec![7u8; 32]),
            known: true,
        }
    }

    #[test]
    fn request_roundtrip() {
        let req = DiscoverRequest {
            dev: Id16::random(),
            pv: 1,
        };
        let wire = encode_request(&req).unwrap();
        assert_eq!(&wire[..4], &MAGIC);
        assert_eq!(wire[5], KIND_REQUEST);
        assert_eq!(decode(&wire).unwrap(), Datagram::Request(req));
    }

    #[test]
    fn response_roundtrip_stays_within_datagram_budget() {
        let res = sample_response();
        let wire = encode_response(&res).unwrap();
        assert!(wire.len() <= MAX_DATAGRAM, "datagram {} byte", wire.len());
        assert_eq!(decode(&wire).unwrap(), Datagram::Response(res));
    }

    #[test]
    fn foreign_traffic_is_ignored_not_an_error_to_answer() {
        // Trafik lain di port yang sama, magic salah, versi wire tak dikenal, dan kind asing
        // semuanya harus menghasilkan Foreign supaya host membuangnya tanpa membalas.
        for bad in [
            b"hello world here".to_vec(),
            {
                let mut v = b"XXXX".to_vec();
                v.push(WIRE_VERSION);
                v.push(KIND_REQUEST);
                v.extend_from_slice(&[0xa0]);
                v
            },
            {
                let mut v = MAGIC.to_vec();
                v.push(99);
                v.push(KIND_REQUEST);
                v.extend_from_slice(&[0xa0]);
                v
            },
            {
                let mut v = MAGIC.to_vec();
                v.push(WIRE_VERSION);
                v.push(0x7f);
                v.extend_from_slice(&[0xa0]);
                v
            },
            MAGIC.to_vec(),
        ] {
            assert!(
                matches!(decode(&bad), Err(DiscoveryError::Foreign)),
                "seharusnya diabaikan: {bad:?}"
            );
        }
    }

    #[test]
    fn version_outside_range_is_not_answered() {
        assert!(should_answer(&DiscoverRequest {
            dev: Id16::default(),
            pv: 1
        }));
        assert!(!should_answer(&DiscoverRequest {
            dev: Id16::default(),
            pv: 0
        }));
        assert!(!should_answer(&DiscoverRequest {
            dev: Id16::default(),
            pv: 99
        }));
    }
}
