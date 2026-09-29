//! Host uji untuk uji lintas-bahasa (Kotlin ↔ Rust).
//!
//! Berjalan di 127.0.0.1 dengan port acak dan dependensi palsu, lalu melaporkan semua yang
//! terjadi lewat stdout dalam format satu-baris agar mudah dibaca test:
//!
//! ```text
//! READY session=<port> discovery=<port> pk=<hex> hid=<hex>
//! PIN <pin>            (setelah perintah `pair`)
//! EVENT <deskripsi>
//! CALL <panggilan input>
//! RAN <aksi deck>
//! ```
//!
//! Perintah stdin: `pair`, `cancel`, `revoke <hex id>`, `settings <sens> <natural>`, `quit`.

use std::io::Write;
use std::net::{IpAddr, Ipv4Addr};
use std::sync::Arc;
use tab_deck::RecordingRunner;
use tab_host::{HostConfig, HostDeps, HostEvent, HostHandle};
use tab_input::RecordingInput;
use tab_protocol::message::InputSettings;
use tab_protocol::noise::hex;
use tab_protocol::Id16;
use tokio::io::{AsyncBufReadExt, BufReader};

#[tokio::main]
async fn main() {
    let input = RecordingInput::new();
    let input_log = input.log();
    let runner = RecordingRunner::default();
    let ran = Arc::clone(&runner.ran);

    let mut deps = HostDeps::fakes();
    deps.input = Box::new(input);
    deps.runner = Box::new(runner);

    let mut cfg = HostConfig::detect();
    cfg.name = "TestHost".into();
    cfg.bind = IpAddr::V4(Ipv4Addr::LOCALHOST);
    cfg.discovery_port = 0;
    cfg.session_port = 0;
    let host = HostHandle::start(cfg, deps).await.expect("host start");

    println!(
        "READY session={} discovery={} pk={} hid={}",
        host.session_addr().port(),
        host.discovery_addr().port(),
        hex(&host.public_key()),
        host.host_id()
    );

    let mut events = host.events();
    tokio::spawn(async move {
        while let Ok(ev) = events.recv().await {
            match ev {
                HostEvent::PairingStarted { .. } => {}
                other => println!("EVENT {other:?}"),
            }
        }
    });

    // Sampaikan panggilan input dan aksi deck yang baru muncul.
    tokio::spawn(async move {
        let (mut seen_in, mut seen_run) = (0usize, 0usize);
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            {
                let log = input_log.lock().unwrap();
                for c in &log[seen_in..] {
                    println!("CALL {c:?}");
                }
                seen_in = log.len();
            }
            {
                let log = ran.lock().unwrap();
                for a in &log[seen_run..] {
                    println!("RAN {a:?}");
                }
                seen_run = log.len();
            }
            let _ = std::io::stdout().flush();
        }
    });

    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        let mut p = line.split_whitespace();
        match p.next() {
            Some("pair") => {
                let t = host.begin_pairing().await.expect("begin_pairing");
                println!("PIN {}", t.pin);
            }
            Some("cancel") => {
                let _ = host.cancel_pairing().await;
            }
            Some("revoke") => {
                if let Some(id) = p.next().and_then(Id16::from_hex) {
                    let _ = host.revoke(id).await;
                }
            }
            Some("settings") => {
                let sens = p.next().and_then(|s| s.parse().ok()).unwrap_or(1.0);
                let natural = p.next().map(|s| s == "true").unwrap_or(true);
                host.set_input_settings(InputSettings { sens, natural });
            }
            Some("quit") => break,
            _ => {}
        }
        let _ = std::io::stdout().flush();
    }
    let _ = host.shutdown().await;
}
