//! Responder discovery UDP.
//!
//! Datagram tidak terenkripsi, jadi balasan hanya memuat hal yang aman diketahui siapa pun di
//! LAN. Rate limit per alamat sumber mencegah host dipakai sebagai penguat lalu lintas.

use crate::Shared;
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tab_protocol::discovery::{self, Datagram, DiscoverResponse};
use tab_protocol::{MAX_DATAGRAM, PROTOCOL_MAX, PROTOCOL_MIN};
use tokio::net::UdpSocket;
use tokio::sync::watch;

pub const RATE_LIMIT: Duration = Duration::from_millis(200);
const MAX_TRACKED: usize = 1024;

/// Bind UDP dengan `SO_REUSEADDR` supaya host bisa restart cepat dan beberapa proses uji
/// bisa berbagi port di mesin yang sama.
pub fn bind(addr: SocketAddr) -> std::io::Result<UdpSocket> {
    let sock = socket2::Socket::new(
        socket2::Domain::for_address(addr),
        socket2::Type::DGRAM,
        Some(socket2::Protocol::UDP),
    )?;
    sock.set_reuse_address(true)?;
    sock.set_nonblocking(true)?;
    sock.bind(&addr.into())?;
    UdpSocket::from_std(sock.into())
}

/// Pembatas laju per alamat. Dipisah dari socket agar bisa dites tanpa jaringan.
#[derive(Default)]
pub struct RateLimiter {
    last: HashMap<IpAddr, Instant>,
}

impl RateLimiter {
    pub fn allow(&mut self, ip: IpAddr, now: Instant) -> bool {
        if self.last.len() >= MAX_TRACKED {
            self.last
                .retain(|_, t| now.saturating_duration_since(*t) < RATE_LIMIT);
        }
        match self.last.get(&ip) {
            Some(t) if now.saturating_duration_since(*t) < RATE_LIMIT => false,
            _ => {
                self.last.insert(ip, now);
                true
            }
        }
    }
}

pub(crate) async fn run(shared: Arc<Shared>, sock: UdpSocket, mut shutdown: watch::Receiver<bool>) {
    let mut limiter = RateLimiter::default();
    let mut buf = vec![0u8; MAX_DATAGRAM + 1];
    loop {
        let (n, from) = tokio::select! {
            r = sock.recv_from(&mut buf) => match r {
                Ok(v) => v,
                // Di Windows, ICMP "port unreachable" dari balasan sebelumnya muncul sebagai
                // error di recv; itu bukan alasan menghentikan responder.
                Err(_) => continue,
            },
            _ = shutdown.changed() => return,
        };

        // Datagram asing dan tidak sah dibuang tanpa balasan.
        let Ok(Datagram::Request(req)) = discovery::decode(&buf[..n]) else {
            continue;
        };
        if !discovery::should_answer(&req) || !limiter.allow(from.ip(), Instant::now()) {
            continue;
        }

        let res = DiscoverResponse {
            hid: shared.host_id,
            name: shared.cfg.name.chars().take(64).collect(),
            os: shared.cfg.os.clone(),
            osv: shared.cfg.os_version.clone(),
            app: shared.cfg.app_version.clone(),
            pmin: PROTOCOL_MIN,
            pmax: PROTOCOL_MAX,
            port: shared.session_port,
            fp: serde_bytes::ByteBuf::from(shared.fingerprint.to_vec()),
            pk: serde_bytes::ByteBuf::from(shared.keys.public.to_vec()),
            known: shared.registry.is_known(req.dev),
        };
        if let Ok(wire) = discovery::encode_response(&res) {
            let _ = sock.send_to(&wire, from).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_limiter_allows_one_per_window_per_address() {
        let mut rl = RateLimiter::default();
        let a: IpAddr = "192.168.1.10".parse().unwrap();
        let b: IpAddr = "192.168.1.11".parse().unwrap();
        let t0 = Instant::now();
        assert!(rl.allow(a, t0));
        assert!(!rl.allow(a, t0 + Duration::from_millis(100)));
        assert!(rl.allow(b, t0 + Duration::from_millis(100)), "alamat lain bebas");
        assert!(rl.allow(a, t0 + Duration::from_millis(250)));
    }

    #[test]
    fn rate_limiter_memory_is_bounded() {
        let mut rl = RateLimiter::default();
        let t0 = Instant::now();
        for i in 0..(MAX_TRACKED as u32 * 3) {
            let ip = IpAddr::from(std::net::Ipv4Addr::from(i));
            rl.allow(ip, t0 + Duration::from_secs(i as u64));
        }
        assert!(rl.last.len() <= MAX_TRACKED + 1);
    }
}
