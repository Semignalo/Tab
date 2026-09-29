//! Discovery sisi client: broadcast ke **setiap** antarmuka aktif (bukan hanya
//! 255.255.255.255, yang sering diblokir), 3 kali dengan jeda 300 ms, lalu kumpulkan balasan.

use crate::{check_fingerprint, ClientError};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;
use tab_protocol::discovery::{self, Datagram, DiscoverRequest, DiscoverResponse};
use tab_protocol::{Id16, DISCOVERY_PORT, PROTOCOL_VERSION};
use tokio::net::UdpSocket;

#[derive(Debug, Clone)]
pub struct FoundHost {
    pub response: DiscoverResponse,
    pub addr: SocketAddr,
    pub public_key: [u8; 32],
}

/// Alamat tujuan broadcast: tiap subnet IPv4 aktif, ditambah broadcast global sebagai cadangan.
pub fn broadcast_targets(port: u16) -> Vec<SocketAddr> {
    let mut out: Vec<SocketAddr> = Vec::new();
    if let Ok(ifaces) = if_addrs::get_if_addrs() {
        for iface in ifaces {
            if iface.is_loopback() {
                continue;
            }
            if let if_addrs::IfAddr::V4(v4) = iface.addr {
                if let Some(b) = v4.broadcast {
                    out.push(SocketAddr::new(IpAddr::V4(b), port));
                }
            }
        }
    }
    out.push(SocketAddr::new(IpAddr::V4(Ipv4Addr::BROADCAST), port));
    out.sort();
    out.dedup();
    out
}

/// Cari host. `targets` bisa diisi alamat unicast (mis. 127.0.0.1:port) untuk test.
pub async fn discover_at(
    device: Id16,
    targets: &[SocketAddr],
    wait: Duration,
) -> Result<Vec<FoundHost>, ClientError> {
    let sock = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).await?;
    sock.set_broadcast(true)?;
    let req = discovery::encode_request(&DiscoverRequest {
        dev: device,
        pv: PROTOCOL_VERSION,
    })
    .map_err(|e| ClientError::Unexpected(e.to_string()))?;

    let mut found: Vec<FoundHost> = Vec::new();
    let mut buf = vec![0u8; 2048];
    let deadline = tokio::time::Instant::now() + wait;
    let mut sends = 0;
    let mut next_send = tokio::time::Instant::now();

    loop {
        if sends < 3 && tokio::time::Instant::now() >= next_send {
            for t in targets {
                let _ = sock.send_to(&req, t).await;
            }
            sends += 1;
            next_send += Duration::from_millis(300);
        }
        let wake = if sends < 3 {
            next_send.min(deadline)
        } else {
            deadline
        };
        match tokio::time::timeout_at(wake, sock.recv_from(&mut buf)).await {
            Ok(Ok((n, from))) => {
                if let Ok(Datagram::Response(res)) = discovery::decode(&buf[..n]) {
                    let Ok(pk): Result<[u8; 32], _> = res.pk.as_ref().try_into() else {
                        continue;
                    };
                    // Kunci tanpa fingerprint yang cocok dibuang: host palsu/rusak.
                    if check_fingerprint(&pk, &res.fp).is_err() {
                        continue;
                    }
                    if found.iter().any(|f| f.response.hid == res.hid) {
                        continue;
                    }
                    found.push(FoundHost {
                        addr: SocketAddr::new(from.ip(), res.port),
                        public_key: pk,
                        response: res,
                    });
                }
            }
            Ok(Err(_)) => continue,
            Err(_) => {
                if tokio::time::Instant::now() >= deadline {
                    break;
                }
            }
        }
    }
    Ok(found)
}

pub async fn discover(device: Id16, wait: Duration) -> Result<Vec<FoundHost>, ClientError> {
    discover_at(device, &broadcast_targets(DISCOVERY_PORT), wait).await
}
