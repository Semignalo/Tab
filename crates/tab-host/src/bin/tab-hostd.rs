//! `tab-hostd` — host Tab tanpa UI. Berguna untuk pengembangan, server tanpa layar, dan
//! sebagai jalur cadangan bila shell Tauri belum terpasang.
//!
//! Perintah interaktif (ketik lalu Enter): `p` PIN pairing baru, `l` daftar perangkat,
//! `r <awalan id>` cabut perangkat, `c` batalkan pairing, `q` keluar.

use std::path::PathBuf;
use tab_deck::{system_runner, JsonProfileStore};
use tab_host::{HostConfig, HostDeps, HostEvent, HostHandle};
use tab_protocol::Id16;
use tokio::io::{AsyncBufReadExt, BufReader};

fn config_dir() -> PathBuf {
    let base = std::env::var_os("APPDATA")
        .or_else(|| std::env::var_os("XDG_CONFIG_HOME"))
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("Tab")
}

fn arg_value(flag: &str) -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1).cloned())
}

#[cfg(any(windows, target_os = "macos"))]
fn token_store() -> Box<dyn tab_host::TokenStore> {
    Box::new(tab_host::KeyringTokenStore::new())
}

#[cfg(not(any(windows, target_os = "macos")))]
fn token_store() -> Box<dyn tab_host::TokenStore> {
    eprintln!("peringatan: platform ini belum punya keyring; pairing hilang saat restart");
    Box::new(tab_host::MemoryTokenStore::new())
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "tab_host=info".into()),
        )
        .init();

    let mut cfg = HostConfig::detect();
    if let Some(name) = arg_value("--name") {
        cfg.name = name;
    }
    cfg.os_version = sysinfo::System::os_version().unwrap_or_else(|| "unknown".into());
    cfg.lyrics_dir = arg_value("--lyrics").map(PathBuf::from);

    let profiles = JsonProfileStore::open(config_dir().join("profiles"))
        .expect("folder profil deck tidak bisa dibuka");
    let deps = HostDeps {
        input: tab_input::platform_input(),
        clipboard: Box::new(tab_input::SystemClipboard::new()),
        metrics: Box::new(tab_metrics::SysMetrics::new()),
        media: tab_media::platform_media(),
        deck: Box::new(profiles),
        runner: Box::new(system_runner(tab_input::platform_input())),
        store: token_store(),
    };

    let host = match HostHandle::start(cfg.clone(), deps).await {
        Ok(h) => h,
        Err(e) => {
            eprintln!("host gagal start: {e}");
            eprintln!("(port {} / {} sudah dipakai? satu host per mesin)", cfg.discovery_port, cfg.session_port);
            std::process::exit(1);
        }
    };

    println!("Tab host '{}' aktif", cfg.name);
    println!("  discovery UDP {}, sesi TCP {}", host.discovery_addr().port(), host.session_addr().port());
    println!("  host id     {}", host.host_id());
    println!("  fingerprint {}", tab_protocol::noise::hex(&host.fingerprint())[..16].to_owned());
    println!("  mode: {:?}", host.capabilities().modes);
    println!("Windows: izinkan 'Private network' saat dialog Firewall muncul, atau HP tidak akan menemukan host.");
    println!("Ketik `p` + Enter untuk membuat PIN pairing.\n");

    if std::env::args().any(|a| a == "--pair") {
        print_pin(&host).await;
    }

    let mut events = host.events();
    tokio::spawn(async move {
        while let Ok(ev) = events.recv().await {
            match ev {
                HostEvent::PairingSucceeded { name, .. } => println!("✔ perangkat dipasangkan: {name}"),
                HostEvent::PairingFailed { reason, attempts_left } => {
                    println!("✘ pairing gagal: {reason} (sisa {attempts_left})")
                }
                HostEvent::PairingEnded => println!("(jendela pairing ditutup)"),
                HostEvent::DeviceConnected { name, .. } => println!("→ tersambung: {name}"),
                HostEvent::DeviceDisconnected { device } => println!("← terputus: {}", &device.to_hex()[..8]),
                HostEvent::DeviceRevoked { device } => println!("⊘ dicabut: {}", &device.to_hex()[..8]),
                HostEvent::ModeChanged { device, mode } => {
                    println!("  {} → mode {:?}", &device.to_hex()[..8], mode)
                }
                HostEvent::PairingStarted { .. } => {}
            }
        }
    });

    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    let ctrl_c = tokio::signal::ctrl_c();
    tokio::pin!(ctrl_c);
    loop {
        tokio::select! {
            _ = &mut ctrl_c => break,
            line = lines.next_line() => {
                let Ok(Some(line)) = line else {
                    // stdin tertutup (dijalankan sebagai layanan): tetap hidup sampai Ctrl+C.
                    let _ = (&mut ctrl_c).await;
                    break;
                };
                let mut parts = line.split_whitespace();
                match parts.next() {
                    Some("p") => print_pin(&host).await,
                    Some("c") => { let _ = host.cancel_pairing().await; }
                    Some("l") => {
                        for d in host.devices().await.unwrap_or_default() {
                            println!("  {}  {:<20} {}", d.device, d.name, d.platform);
                        }
                    }
                    Some("r") => {
                        let prefix = parts.next().unwrap_or("");
                        let devices = host.devices().await.unwrap_or_default();
                        let hit: Vec<Id16> = devices.iter().filter(|d| !prefix.is_empty() && d.device.to_hex().starts_with(prefix)).map(|d| d.device).collect();
                        match hit.as_slice() {
                            [one] => { let _ = host.revoke(*one).await; }
                            _ => println!("awalan tidak tepat satu perangkat"),
                        }
                    }
                    Some("q") => break,
                    _ => {}
                }
            }
        }
    }
    let _ = host.shutdown().await;
}

async fn print_pin(host: &HostHandle) {
    match host.begin_pairing().await {
        Ok(t) => println!("\n  PIN PAIRING: {}   (berlaku {} detik)\n", t.pin, t.ttl.as_secs()),
        Err(e) => println!("gagal: {e}"),
    }
}
