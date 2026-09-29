//! Listener TCP: menerima koneksi, memasang opsi socket, dan menegakkan batas sesi.

use crate::{session, Shared};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{watch, Semaphore};

pub fn bind(addr: SocketAddr) -> std::io::Result<TcpListener> {
    let sock = socket2::Socket::new(
        socket2::Domain::for_address(addr),
        socket2::Type::STREAM,
        Some(socket2::Protocol::TCP),
    )?;
    // Di Windows SO_REUSEADDR berarti "boleh mencuri port", jadi hanya dipasang di non-Windows.
    #[cfg(not(windows))]
    sock.set_reuse_address(true)?;
    sock.set_nonblocking(true)?;
    sock.bind(&addr.into())?;
    sock.listen(64)?;
    TcpListener::from_std(sock.into())
}

/// `TCP_NODELAY` supaya satu frame = satu paket (tanpa Nagle), dan keepalive supaya koneksi
/// yang mati diam-diam (HP masuk saku, Wi-Fi hilang) akhirnya terdeteksi OS.
fn tune(stream: &TcpStream) {
    let _ = stream.set_nodelay(true);
    let sock = socket2::SockRef::from(stream);
    let ka = socket2::TcpKeepalive::new()
        .with_time(Duration::from_secs(10))
        .with_interval(Duration::from_secs(3));
    let _ = sock.set_tcp_keepalive(&ka);
}

pub(crate) async fn run(
    shared: Arc<Shared>,
    listener: TcpListener,
    slots: Arc<Semaphore>,
    mut shutdown: watch::Receiver<bool>,
) {
    loop {
        let (stream, peer) = tokio::select! {
            r = listener.accept() => match r {
                Ok(v) => v,
                Err(e) => {
                    tracing::warn!("accept gagal: {e}");
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    continue;
                }
            },
            _ = shutdown.changed() => return,
        };

        // Slot habis: tutup langsung. Sebelum handshake belum ada kanal terenkripsi untuk
        // menjelaskan alasannya, dan client akan mencoba lagi dengan backoff.
        let Ok(permit) = slots.clone().try_acquire_owned() else {
            tracing::info!("koneksi dari {peer} ditolak: sesi penuh");
            drop(stream);
            continue;
        };

        tune(&stream);
        let shared = Arc::clone(&shared);
        let shutdown = shutdown.clone();
        tokio::spawn(async move {
            session::serve(shared, stream, shutdown).await;
            drop(permit);
        });
    }
}
