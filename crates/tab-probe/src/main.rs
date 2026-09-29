//! `probe` — client uji baris perintah. Alat ukur, bukan pemulus: angka ditampilkan apa adanya.

use clap::{Parser, Subcommand, ValueEnum};
use std::net::SocketAddr;
use std::time::{Duration, Instant};
use tab_probe::discover::{discover, FoundHost};
use tab_probe::stats::{count_gaps, summarize};
use tab_probe::store::{default_path, from_hex, record, ProbeStore};
use tab_probe::{micros_since, Client, ClientError, Credential, DeviceInfo, Outcome, PinOutcome};
use tab_protocol::message::*;
use tab_protocol::noise::hex;
use tab_protocol::Id16;

#[derive(Parser)]
#[command(name = "probe", about = "Client uji Tab")]
struct Cli {
    /// File penyimpanan token (default: %APPDATA%/tab-probe/probe.json).
    #[arg(long, global = true)]
    store: Option<std::path::PathBuf>,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Broadcast ke semua antarmuka dan daftar host yang menjawab.
    Discover {
        #[arg(long, default_value_t = 1500)]
        wait_ms: u64,
    },
    /// Pairing dengan PIN yang tampil di layar host.
    Pair {
        #[arg(long)]
        pin: String,
        /// Awalan host_id (hex); boleh dikosongkan bila hanya ada satu host.
        #[arg(long)]
        host: Option<String>,
        /// Alamat langsung, mis. 192.168.1.5:4180 (melewati discovery).
        #[arg(long)]
        addr: Option<SocketAddr>,
    },
    /// Ukur RTT (Ping) sambil mengirim InputBatch 120 Hz.
    Rtt {
        #[arg(long, default_value_t = 30)]
        duration: u64,
        #[arg(long, default_value_t = 2.0)]
        ping_hz: f64,
        #[arg(long)]
        host: Option<String>,
    },
    /// Gerakan pointer sintetis untuk menguji kehalusan. MENGGERAKKAN KURSOR host.
    Input {
        #[arg(long, value_enum, default_value_t = Pattern::Circle)]
        pattern: Pattern,
        #[arg(long, default_value_t = 10)]
        duration: u64,
        #[arg(long)]
        host: Option<String>,
    },
    /// Masuk ke sebuah mode dan cetak telemetri yang datang.
    Modes {
        #[arg(long, value_enum)]
        mode: ModeArg,
        #[arg(long, default_value_t = 10)]
        duration: u64,
        #[arg(long)]
        host: Option<String>,
    },
    /// Cabut pairing lokal (token + kunci yang dipin) untuk host tertentu.
    Forget {
        #[arg(long)]
        host: String,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum Pattern {
    Circle,
    Line,
    Scroll,
}

#[derive(Clone, Copy, ValueEnum)]
enum ModeArg {
    Trackpad,
    Deck,
    Monitor,
    Music,
    Clock,
}

impl From<ModeArg> for Mode {
    fn from(m: ModeArg) -> Mode {
        match m {
            ModeArg::Trackpad => Mode::Trackpad,
            ModeArg::Deck => Mode::Deck,
            ModeArg::Monitor => Mode::Monitor,
            ModeArg::Music => Mode::Music,
            ModeArg::Clock => Mode::Clock,
        }
    }
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let path = cli.store.unwrap_or_else(default_path);
    let mut store = ProbeStore::load(&path);
    let code = match run(cli.cmd, &mut store, &path).await {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("error: {e}");
            1
        }
    };
    let _ = store.save(&path);
    std::process::exit(code);
}

type R<T> = Result<T, Box<dyn std::error::Error>>;

async fn run(cmd: Cmd, store: &mut ProbeStore, path: &std::path::Path) -> R<()> {
    let _ = path;
    match cmd {
        Cmd::Discover { wait_ms } => {
            let dev = store.device_id();
            let found = discover(dev, Duration::from_millis(wait_ms)).await?;
            if found.is_empty() {
                println!("tidak ada host yang menjawab");
            }
            for h in &found {
                print_host(h, store);
            }
            Ok(())
        }
        Cmd::Pair { pin, host, addr } => pair(store, &pin, host, addr).await,
        Cmd::Rtt {
            duration,
            ping_hz,
            host,
        } => rtt(store, duration, ping_hz, host).await,
        Cmd::Input {
            pattern,
            duration,
            host,
        } => input(store, pattern, duration, host).await,
        Cmd::Modes {
            mode,
            duration,
            host,
        } => modes(store, mode.into(), duration, host).await,
        Cmd::Forget { host } => {
            let hid = pick_host(store, Some(&host))?;
            store.hosts.remove(&hid.to_hex());
            println!("pairing lokal untuk {hid} dihapus (token di host tetap ada sampai dicabut di sana)");
            Ok(())
        }
    }
}

fn print_host(h: &FoundHost, store: &ProbeStore) {
    let r = &h.response;
    let mine = store.host(r.hid).is_some();
    println!(
        "{}  {:<24} {} {}  {}  fp={}  {}{}",
        r.hid,
        r.name,
        r.os,
        r.osv,
        h.addr,
        &hex(&r.fp)[..16],
        if r.known {
            "[dikenal host]"
        } else {
            "[belum dipasangkan]"
        },
        if mine { " [token lokal ada]" } else { "" },
    );
}

fn pick_host(store: &ProbeStore, prefix: Option<&str>) -> R<Id16> {
    let ids: Vec<&String> = store
        .hosts
        .keys()
        .filter(|k| prefix.is_none_or(|p| k.starts_with(p)))
        .collect();
    match ids.as_slice() {
        [one] => Id16::from_hex(one).ok_or_else(|| "host_id tersimpan rusak".into()),
        [] => Err("belum ada host yang dipasangkan — jalankan `probe pair` dulu".into()),
        _ => Err("lebih dari satu host cocok; beri --host dengan awalan yang lebih panjang".into()),
    }
}

async fn pair(
    store: &mut ProbeStore,
    pin: &str,
    prefix: Option<String>,
    addr: Option<SocketAddr>,
) -> R<()> {
    let dev = store.device_id();
    let found = match addr {
        Some(a) => {
            // Tanpa discovery kita tidak punya kunci publik; ambil lewat unicast ke port discovery.
            let target = SocketAddr::new(a.ip(), tab_protocol::DISCOVERY_PORT);
            tab_probe::discover::discover_at(dev, &[target], Duration::from_millis(1500)).await?
        }
        None => discover(dev, Duration::from_millis(1500)).await?,
    };
    let candidates: Vec<&FoundHost> = found
        .iter()
        .filter(|h| {
            prefix
                .as_deref()
                .is_none_or(|p| h.response.hid.to_hex().starts_with(p))
        })
        .collect();
    let host = match candidates.as_slice() {
        [h] => *h,
        [] => return Err("host tidak ditemukan".into()),
        many => {
            for h in many {
                print_host(h, store);
            }
            return Err("beberapa host cocok; pilih dengan --host <awalan hid>".into());
        }
    };

    // Kunci yang sudah dipin tidak boleh berubah diam-diam.
    if let Some(rec) = store.host(host.response.hid) {
        if rec.public_key != hex(&host.public_key) {
            return Err(format!(
                "KUNCI HOST BERUBAH untuk {}. Bisa jadi host diinstal ulang, bisa juga host palsu. \
                 Jalankan `probe forget --host {}` bila Anda yakin itu host yang benar.",
                host.response.name,
                host.response.hid
            )
            .into());
        }
    }

    println!("menyambung ke {} ({}) …", host.response.name, host.addr);
    let mut c = Client::connect(
        host.addr,
        &host.public_key,
        DeviceInfo::probe(dev),
        Credential::Pair,
    )
    .await?;
    match c.hello().await? {
        Outcome::PairRequired { ttl_ms } => println!("PIN berlaku {} detik lagi", ttl_ms / 1000),
        Outcome::Refused(e) => return Err(format!("{:?}: {}", e.c, e.msg).into()),
        Outcome::Welcome(_) => return Err("host langsung Welcome tanpa PIN (tidak terduga)".into()),
    }
    match c.submit_pin(pin).await? {
        PinOutcome::Paired { token, welcome } => {
            store.remember(
                host.response.hid,
                record(&token, &host.public_key, host.addr, &host.response.name),
            );
            println!("berhasil dipasangkan dengan {}", welcome.name);
            println!("mode tersedia: {:?}", welcome.caps.modes);
            Ok(())
        }
        PinOutcome::Invalid { attempts_left } => {
            Err(format!("PIN salah, sisa {attempts_left} percobaan").into())
        }
        PinOutcome::Refused(e) => Err(format!("{:?}: {}", e.c, e.msg).into()),
    }
}

/// Sambung dengan token tersimpan; bila alamat lama gagal, jalankan discovery (urutan §9).
async fn connect_stored(store: &mut ProbeStore, prefix: Option<&str>) -> R<Client> {
    let hid = pick_host(store, prefix)?;
    let dev = store.device_id();
    let rec = store.host(hid).cloned().ok_or("host tidak ada")?;
    let token: [u8; 32] = from_hex(&rec.token).ok_or("token rusak")?;
    let pk: [u8; 32] = from_hex(&rec.public_key).ok_or("kunci rusak")?;

    let mut addrs: Vec<SocketAddr> = rec.addr.parse().ok().into_iter().collect();
    let mut tried_discovery = false;
    let mut backoff = Duration::from_millis(250);
    for attempt in 0..8 {
        for a in addrs.clone() {
            match Client::connect(a, &pk, DeviceInfo::probe(dev), Credential::Resume(token)).await {
                Ok(mut c) => match c.hello().await {
                    Ok(Outcome::Welcome(_)) => return Ok(c),
                    Ok(other) => return Err(format!("host menolak: {other:?}").into()),
                    Err(ClientError::Closed) | Err(ClientError::Timeout) => {
                        return Err(
                            "host menutup koneksi: token kemungkinan dicabut — pairing ulang"
                                .into(),
                        )
                    }
                    Err(e) => return Err(e.into()),
                },
                Err(e) => eprintln!("percobaan {attempt} ke {a} gagal: {e}"),
            }
        }
        if !tried_discovery {
            tried_discovery = true;
            if let Ok(found) = discover(dev, Duration::from_millis(1200)).await {
                if let Some(h) = found.iter().find(|h| h.response.hid == hid) {
                    if hex(&h.public_key) != rec.public_key {
                        return Err("KUNCI HOST BERUBAH — menolak sambung".into());
                    }
                    addrs = vec![h.addr];
                    continue;
                }
            }
        }
        tokio::time::sleep(backoff).await;
        backoff = (backoff * 2).min(Duration::from_secs(5));
    }
    Err("host tidak terjangkau".into())
}

async fn rtt(store: &mut ProbeStore, secs: u64, ping_hz: f64, host: Option<String>) -> R<()> {
    let mut c = connect_stored(store, host.as_deref()).await?;
    c.send(&Message::SetMode(SetMode { m: Mode::Trackpad }))
        .await?;

    let start = Instant::now();
    let end = start + Duration::from_secs(secs);
    let mut ping_iv = tokio::time::interval(Duration::from_secs_f64(1.0 / ping_hz.max(0.1)));
    let mut input_iv = tokio::time::interval(Duration::from_micros(8333)); // 120 Hz
    ping_iv.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    input_iv.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    let (mut n, mut seq) = (0u64, 0u64);
    let mut rtts_ms: Vec<f64> = Vec::new();
    let mut sent_pings = 0u64;

    while Instant::now() < end {
        tokio::select! {
            _ = ping_iv.tick() => {
                n += 1;
                sent_pings += 1;
                c.send(&Message::Ping(Ping { n, tc: micros_since(start) })).await?;
            }
            _ = input_iv.tick() => {
                seq += 1;
                // Delta nol: menguji jalur tanpa menggeser kursor pengguna.
                c.send(&Message::InputBatch(InputBatch {
                    s: seq,
                    ev: vec![InputEvent::PointerMove { dx: 0.0, dy: 0.0 }],
                })).await?;
            }
            msg = c.recv() => {
                if let Message::Pong(p) = msg? {
                    let now = micros_since(start);
                    rtts_ms.push(now.saturating_sub(p.tc) as f64 / 1000.0);
                }
            }
        }
    }

    println!("durasi {secs}s, {sent_pings} ping, {seq} batch input terkirim");
    match summarize(&rtts_ms) {
        Some(s) => println!(
            "RTT ms: n={} min={:.2} p50={:.2} p95={:.2} p99={:.2} max={:.2} mean={:.2}",
            s.count, s.min, s.p50, s.p95, s.p99, s.max, s.mean
        ),
        None => println!("tidak ada Pong yang diterima"),
    }
    let lost = sent_pings.saturating_sub(rtts_ms.len() as u64);
    println!("Pong hilang: {lost}");
    // Host tidak mengirim ulang seq; celah dihitung di sisi host lewat log. Di sini kita
    // hanya memastikan urutan kirim kita sendiri utuh.
    println!(
        "celah seq lokal: {}",
        count_gaps(&(1..=seq).collect::<Vec<_>>())
    );
    Ok(())
}

async fn input(store: &mut ProbeStore, pattern: Pattern, secs: u64, host: Option<String>) -> R<()> {
    let mut c = connect_stored(store, host.as_deref()).await?;
    c.send(&Message::SetMode(SetMode { m: Mode::Trackpad }))
        .await?;
    println!("menggerakkan kursor host selama {secs}s …");

    let start = Instant::now();
    let mut iv = tokio::time::interval(Duration::from_micros(8333));
    let mut seq = 0u64;
    let mut ping_iv = tokio::time::interval(Duration::from_secs(2));
    let mut prev = (0.0f32, 0.0f32);
    while start.elapsed() < Duration::from_secs(secs) {
        tokio::select! {
            _ = iv.tick() => {
                seq += 1;
                let t = start.elapsed().as_secs_f32();
                let ev = match pattern {
                    Pattern::Circle => {
                        // Lingkaran radius 150 px, 1 putaran/detik: kirim selisih posisi.
                        let (x, y) = (150.0 * (t * std::f32::consts::TAU).cos(), 150.0 * (t * std::f32::consts::TAU).sin());
                        let d = InputEvent::PointerMove { dx: x - prev.0, dy: y - prev.1 };
                        prev = (x, y);
                        d
                    }
                    Pattern::Line => {
                        let x = 200.0 * (t * std::f32::consts::PI).sin();
                        let d = InputEvent::PointerMove { dx: x - prev.0, dy: 0.0 };
                        prev = (x, 0.0);
                        d
                    }
                    Pattern::Scroll => InputEvent::Scroll {
                        dx: 0.0,
                        dy: 6.0 * (t * 2.0).sin(),
                        ph: ScrollPhase::U,
                        mom: false,
                    },
                };
                c.send(&Message::InputBatch(InputBatch { s: seq, ev: vec![ev] })).await?;
            }
            _ = ping_iv.tick() => {
                c.send(&Message::Ping(Ping { n: seq, tc: 0 })).await?;
            }
            msg = c.recv() => { msg?; }
        }
    }
    println!("selesai, {seq} batch");
    Ok(())
}

async fn modes(store: &mut ProbeStore, mode: Mode, secs: u64, host: Option<String>) -> R<()> {
    let mut c = connect_stored(store, host.as_deref()).await?;
    c.send(&Message::SetMode(SetMode { m: mode })).await?;
    if mode == Mode::Monitor {
        c.send(&Message::SubscribeTelemetry(SubscribeTelemetry {
            kinds: vec![TelemetryKind::Metrics],
            iv: 1000,
        }))
        .await?;
    }
    let end = Instant::now() + Duration::from_secs(secs);
    let mut ping_iv = tokio::time::interval(Duration::from_secs(2));
    while Instant::now() < end {
        tokio::select! {
            _ = ping_iv.tick() => { c.send(&Message::Ping(Ping { n: 0, tc: 0 })).await?; }
            msg = c.recv() => match msg? {
                Message::Metrics(m) => println!(
                    "cpu {:.1}% mem {:.1}/{:.1} GiB rx {} KB/s tx {} KB/s",
                    m.cpua, m.mu as f64 / 1073741824.0, m.mt as f64 / 1073741824.0, m.rx / 1024, m.tx / 1024
                ),
                Message::NowPlaying(n) => println!("{} — {} ({}/{} ms) {}", n.artist, n.title, n.pos, n.dur, if n.play {"▶"} else {"⏸"}),
                Message::Pong(_) => {}
                other => println!("{other:?}"),
            }
        }
    }
    Ok(())
}
