//! Uji integrasi di dalam proses: host asli + client dari `tab-probe`, tanpa HP dan tanpa Tauri.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tab_deck::{Action, RecordingRunner};
use tab_host::{HostConfig, HostDeps, HostEvent, HostHandle};
use tab_input::{Call, RecordingInput};
use tab_probe::{Client, ClientError, Credential, DeviceInfo, Outcome, PinOutcome};
use tab_protocol::message::*;
use tab_protocol::Id16;

struct Rig {
    host: HostHandle,
    input_log: Arc<Mutex<Vec<Call>>>,
    ran: Arc<Mutex<Vec<Action>>>,
}

fn cfg() -> HostConfig {
    let mut c = HostConfig::detect();
    c.bind = IpAddr::V4(Ipv4Addr::LOCALHOST);
    c.discovery_port = 0;
    c.session_port = 0;
    c
}

async fn rig_with(cfg: HostConfig) -> Rig {
    let input = RecordingInput::new();
    let input_log = input.log();
    let runner = RecordingRunner::default();
    let ran = Arc::clone(&runner.ran);
    let mut deps = HostDeps::fakes();
    deps.input = Box::new(input);
    deps.runner = Box::new(runner);
    let host = HostHandle::start(cfg, deps).await.unwrap();
    Rig {
        host,
        input_log,
        ran,
    }
}

async fn rig() -> Rig {
    rig_with(cfg()).await
}

impl Rig {
    fn addr(&self) -> SocketAddr {
        self.host.session_addr()
    }

    async fn connect(&self, dev: Id16, cred: Credential) -> Result<Client, ClientError> {
        Client::connect(
            self.addr(),
            &self.host.public_key(),
            DeviceInfo::probe(dev),
            cred,
        )
        .await
    }

    /// Pairing penuh; mengembalikan client yang sudah `Welcome` beserta tokennya.
    async fn pair(&self, dev: Id16) -> (Client, [u8; 32]) {
        let ticket = self.host.begin_pairing().await.unwrap();
        let mut c = self.connect(dev, Credential::Pair).await.unwrap();
        assert!(matches!(
            c.hello().await.unwrap(),
            Outcome::PairRequired { .. }
        ));
        match c.submit_pin(&ticket.pin).await.unwrap() {
            PinOutcome::Paired { token, .. } => (c, token),
            other => panic!("pairing gagal: {other:?}"),
        }
    }

    async fn resume(&self, dev: Id16, token: [u8; 32]) -> Client {
        let mut c = self.connect(dev, Credential::Resume(token)).await.unwrap();
        assert!(matches!(c.hello().await.unwrap(), Outcome::Welcome(_)));
        c
    }
}

async fn expect_closed(c: &mut Client) {
    loop {
        match c.recv_timeout(Duration::from_secs(3)).await {
            Err(ClientError::Closed) => return,
            Err(e) => panic!("seharusnya Closed, dapat {e}"),
            Ok(Message::Error(_)) | Ok(Message::Bye(_)) => continue,
            Ok(_) => continue,
        }
    }
}

/// Baca sampai pesan yang cocok muncul (melewati pesan lain seperti InputSettings).
async fn wait_for<T>(c: &mut Client, mut f: impl FnMut(Message) -> Option<T>) -> T {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        let left = deadline.saturating_duration_since(tokio::time::Instant::now());
        let m = c
            .recv_timeout(left)
            .await
            .expect("pesan yang ditunggu tidak datang");
        if let Some(v) = f(m) {
            return v;
        }
    }
}

// ------------------------------------------------------------------ pairing

#[tokio::test]
async fn pairing_succeeds_issues_token_and_reports_honest_capabilities() {
    let r = rig().await;
    let dev = Id16::random();
    let mut events = r.host.events();

    let ticket = r.host.begin_pairing().await.unwrap();
    let mut c = r.connect(dev, Credential::Pair).await.unwrap();
    let Outcome::PairRequired { ttl_ms } = c.hello().await.unwrap() else {
        panic!("harus PairRequired");
    };
    assert!(ttl_ms > 100_000 && ttl_ms <= 120_000);

    let PinOutcome::Paired { token, welcome } = c.submit_pin(&ticket.pin).await.unwrap() else {
        panic!("pairing gagal");
    };
    assert_ne!(token, [0u8; 32]);
    assert_eq!(welcome.hid, r.host.host_id());

    // Kapabilitas jujur: OBS/second screen/gamepad mati, suhu & GPU mengikuti sensor (fake: false).
    let caps = welcome.caps;
    assert!(!caps.obs && !caps.second_screen && !caps.gamepad);
    assert!(!caps.temps && !caps.gpu);
    assert!(caps.modes.contains(&Mode::Trackpad) && caps.modes.contains(&Mode::Clock));

    let devices = r.host.devices().await.unwrap();
    assert_eq!(devices.len(), 1);
    assert_eq!(devices[0].device, dev);
    assert_eq!(devices[0].token, token);

    // Event untuk UI: PIN mulai → sukses.
    let mut saw_started = false;
    let mut saw_ok = false;
    while let Ok(ev) = events.try_recv() {
        match ev {
            HostEvent::PairingStarted { .. } => saw_started = true,
            HostEvent::PairingSucceeded { device, .. } if device == dev => saw_ok = true,
            _ => {}
        }
    }
    assert!(saw_started && saw_ok);
}

#[tokio::test]
async fn pairing_without_open_window_is_busy() {
    let r = rig().await;
    let mut c = r.connect(Id16::random(), Credential::Pair).await.unwrap();
    match c.hello().await.unwrap() {
        Outcome::Refused(e) => assert_eq!(e.c, ErrorCode::PairingBusy),
        other => panic!("{other:?}"),
    }
}

#[tokio::test]
async fn five_wrong_pins_cancel_pairing_and_sixth_is_expired_not_invalid() {
    let r = rig().await;
    let ticket = r.host.begin_pairing().await.unwrap();
    let wrong = if ticket.pin == "000000" {
        "111111"
    } else {
        "000000"
    };

    let mut c = r.connect(Id16::random(), Credential::Pair).await.unwrap();
    assert!(matches!(
        c.hello().await.unwrap(),
        Outcome::PairRequired { .. }
    ));
    for expected_left in (0..5u8).rev() {
        match c.submit_pin(wrong).await.unwrap() {
            PinOutcome::Invalid { attempts_left } => assert_eq!(attempts_left, expected_left),
            other => panic!("{other:?}"),
        }
    }
    // Host menutup sesi setelah percobaan ke-5...
    expect_closed(&mut c).await;

    // ...dan PIN yang benar pun sudah hangus: perangkat berikut dapat PairingBusy (jendela
    // ditutup), bukan kesempatan menebak lagi.
    let mut c2 = r.connect(Id16::random(), Credential::Pair).await.unwrap();
    match c2.hello().await.unwrap() {
        Outcome::Refused(e) => assert_eq!(e.c, ErrorCode::PairingBusy),
        other => panic!("{other:?}"),
    }
    assert!(r.host.devices().await.unwrap().is_empty());
}

#[tokio::test]
async fn pin_is_single_use_across_two_devices() {
    let r = rig().await;
    let ticket = r.host.begin_pairing().await.unwrap();

    let mut a = r.connect(Id16::random(), Credential::Pair).await.unwrap();
    let mut b = r.connect(Id16::random(), Credential::Pair).await.unwrap();
    assert!(matches!(
        a.hello().await.unwrap(),
        Outcome::PairRequired { .. }
    ));
    assert!(matches!(
        b.hello().await.unwrap(),
        Outcome::PairRequired { .. }
    ));

    assert!(matches!(
        a.submit_pin(&ticket.pin).await.unwrap(),
        PinOutcome::Paired { .. }
    ));
    match b.submit_pin(&ticket.pin).await.unwrap() {
        PinOutcome::Refused(e) => assert_eq!(e.c, ErrorCode::PinExpired),
        other => panic!("{other:?}"),
    }
    assert_eq!(r.host.devices().await.unwrap().len(), 1);
}

#[tokio::test]
async fn cancel_pairing_invalidates_pin() {
    let r = rig().await;
    let ticket = r.host.begin_pairing().await.unwrap();
    r.host.cancel_pairing().await.unwrap();
    let mut c = r.connect(Id16::random(), Credential::Pair).await.unwrap();
    match c.hello().await.unwrap() {
        Outcome::Refused(e) => assert_eq!(e.c, ErrorCode::PairingBusy),
        other => panic!("{other:?} (pin {})", ticket.pin.len()),
    }
}

// ----------------------------------------------------------------- resume

#[tokio::test]
async fn reconnect_with_stored_token_needs_no_pin() {
    let r = rig().await;
    let dev = Id16::random();
    let (first, token) = r.pair(dev).await;
    drop(first);

    let mut again = r.resume(dev, token).await;
    again
        .send(&Message::Ping(Ping { n: 1, tc: 42 }))
        .await
        .unwrap();
    let pong = wait_for(&mut again, |m| match m {
        Message::Pong(p) => Some(p),
        _ => None,
    })
    .await;
    assert_eq!((pong.n, pong.tc), (1, 42), "tc dipantulkan apa adanya");
}

#[tokio::test]
async fn reconnect_with_revoked_token_fails_and_client_must_pair_again() {
    let r = rig().await;
    let dev = Id16::random();
    let (first, token) = r.pair(dev).await;
    drop(first);
    r.host.revoke(dev).await.unwrap();

    // Host tidak mengenal perangkat lagi: koneksi ditutup tanpa penjelasan.
    let res = r.connect(dev, Credential::Resume(token)).await;
    match res {
        Err(_) => {}
        Ok(mut c) => assert!(
            c.hello().await.is_err(),
            "resume dengan token dicabut harus gagal"
        ),
    }

    // Jalur pairing ulang tetap terbuka.
    let (_c, new_token) = r.pair(dev).await;
    assert_ne!(new_token, token);
}

#[tokio::test]
async fn wrong_token_for_known_device_never_reaches_hello() {
    let r = rig().await;
    let dev = Id16::random();
    let (first, _token) = r.pair(dev).await;
    drop(first);

    let res = r.connect(dev, Credential::Resume([9u8; 32])).await;
    match res {
        Err(_) => {}
        Ok(mut c) => assert!(c.hello().await.is_err()),
    }
}

#[tokio::test]
async fn revoking_a_live_session_closes_it() {
    let r = rig().await;
    let dev = Id16::random();
    let (mut c, _t) = r.pair(dev).await;
    r.host.revoke(dev).await.unwrap();

    let mut saw_revoked = false;
    loop {
        match c.recv_timeout(Duration::from_secs(3)).await {
            Ok(Message::Error(e)) if e.c == ErrorCode::TokenRevoked => saw_revoked = true,
            Ok(_) => {}
            Err(ClientError::Closed) => break,
            Err(e) => panic!("{e}"),
        }
    }
    assert!(saw_revoked, "client harus diberi tahu alasannya");
}

// ------------------------------------------------------------------ batas

#[tokio::test]
async fn ninth_concurrent_session_is_refused() {
    let r = rig().await;
    let mut live = Vec::new();
    for _ in 0..8 {
        let dev = Id16::random();
        let (c, _) = r.pair(dev).await;
        live.push(c);
    }
    // Slot terakhir sudah terpakai: koneksi ke-9 ditutup sebelum handshake selesai.
    let res = r.connect(Id16::random(), Credential::Pair).await;
    assert!(res.is_err(), "sesi ke-9 harus ditolak");

    // Melepas satu sesi membebaskan slot.
    live.pop();
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(r.connect(Id16::random(), Credential::Pair).await.is_ok());
}

#[tokio::test]
async fn silent_client_is_timed_out() {
    let r = rig().await;
    let (mut c, _t) = r.pair(Id16::random()).await;
    // Tanpa Ping sama sekali, host menutup setelah SESSION_TIMEOUT (6 s).
    let start = std::time::Instant::now();
    loop {
        match c.recv_timeout(Duration::from_secs(10)).await {
            Err(ClientError::Closed) => break,
            Ok(_) => {}
            Err(e) => panic!("{e}"),
        }
    }
    let waited = start.elapsed();
    assert!(
        waited >= Duration::from_secs(5) && waited < Duration::from_secs(8),
        "{waited:?}"
    );
}

#[tokio::test]
async fn garbage_before_handshake_is_dropped_silently() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let r = rig().await;
    let mut s = tokio::net::TcpStream::connect(r.addr()).await.unwrap();
    // Frame sah secara framing tapi bukan preamble.
    s.write_all(&[0, 0, 0, 3, 1, 2, 3]).await.unwrap();
    let mut buf = [0u8; 16];
    let n = tokio::time::timeout(Duration::from_secs(2), s.read(&mut buf))
        .await
        .unwrap()
        .unwrap_or(0);
    assert_eq!(n, 0, "tidak boleh ada balasan apa pun");
}

// --------------------------------------------------------------- sesi hidup

#[tokio::test]
async fn closing_session_releases_all_held_input() {
    let r = rig().await;
    let (mut c, _t) = r.pair(Id16::random()).await;
    c.send(&Message::InputBatch(InputBatch {
        s: 1,
        ev: vec![
            InputEvent::Modifiers { m: modifiers::CTRL },
            InputEvent::PointerButton {
                b: Button::L,
                d: true,
            },
        ],
    }))
    .await
    .unwrap();
    // Beri waktu host memproses sebelum koneksi diputus paksa (tanpa Bye).
    tokio::time::sleep(Duration::from_millis(200)).await;
    drop(c);
    tokio::time::sleep(Duration::from_millis(500)).await;

    let log = r.input_log.lock().unwrap().clone();
    assert!(log.contains(&Call::Key("ctrl".into(), true)));
    assert!(log.contains(&Call::Button(Button::L, true)));
    assert_eq!(
        log.last(),
        Some(&Call::ReleaseAll),
        "release_all harus terpanggil: {log:?}"
    );
}

#[tokio::test]
async fn unknown_message_type_is_ignored_and_session_survives() {
    let r = rig().await;
    let (mut c, _t) = r.pair(Id16::random()).await;

    let mut future = Vec::new();
    ciborium_map(&mut future);
    c.send_raw(&future).await.unwrap();

    c.send(&Message::Ping(Ping { n: 5, tc: 1 })).await.unwrap();
    let p = wait_for(&mut c, |m| match m {
        Message::Pong(p) => Some(p),
        _ => None,
    })
    .await;
    assert_eq!(p.n, 5);
}

/// `{"t": "SecondScreenFrame", "w": 1920}` dalam CBOR, ditulis manual agar test tidak perlu
/// dependensi tambahan.
fn ciborium_map(out: &mut Vec<u8>) {
    out.clear();
    out.push(0xa2);
    out.extend_from_slice(&[0x61, b't']);
    let name = b"SecondScreenFrame";
    out.push(0x60 | name.len() as u8);
    out.extend_from_slice(name);
    out.extend_from_slice(&[0x61, b'w']);
    out.extend_from_slice(&[0x19, 0x07, 0x80]); // uint 1920
}

#[tokio::test]
async fn pointer_input_is_scaled_by_host_sensitivity_and_scroll_follows_direction_setting() {
    let r = rig().await;
    let (mut c, _t) = r.pair(Id16::random()).await;

    c.send(&Message::SetInputSettings(SetInputSettings {
        sens: Some(2.0),
        natural: Some(false),
    }))
    .await
    .unwrap();
    // Tunggu setelan diterapkan (host memancarkannya kembali).
    wait_for(&mut c, |m| match m {
        Message::InputSettings(s) if s.sens == 2.0 && !s.natural => Some(()),
        _ => None,
    })
    .await;

    c.send(&Message::InputBatch(InputBatch {
        s: 1,
        ev: vec![
            InputEvent::PointerMove { dx: 3.0, dy: -1.0 },
            InputEvent::Scroll {
                dx: 0.0,
                dy: 10.0,
                ph: ScrollPhase::U,
                mom: false,
            },
            InputEvent::PointerMove {
                dx: f32::NAN,
                dy: 0.0,
            },
        ],
    }))
    .await
    .unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;

    let log = r.input_log.lock().unwrap().clone();
    assert!(log.contains(&Call::Move(6.0, -2.0)), "sens 2.0: {log:?}");
    assert!(
        log.contains(&Call::Scroll(0.0, -10.0, ScrollPhase::U, false)),
        "inverted membalik arah: {log:?}"
    );
    assert_eq!(
        log.iter().filter(|c| matches!(c, Call::Move(..))).count(),
        1,
        "NaN harus dibuang"
    );
}

#[tokio::test]
async fn telemetry_flows_only_while_monitor_mode_is_active() {
    let r = rig().await;
    let (mut c, _t) = r.pair(Id16::random()).await;

    c.send(&Message::SubscribeTelemetry(SubscribeTelemetry {
        kinds: vec![TelemetryKind::Metrics],
        iv: 250,
    }))
    .await
    .unwrap();
    // Belum di mode Monitor: tidak ada Metrics.
    assert!(
        tokio::time::timeout(Duration::from_millis(700), async {
            loop {
                if let Ok(Message::Metrics(_)) = c.recv().await {
                    return;
                }
            }
        })
        .await
        .is_err(),
        "telemetri tidak boleh mengalir di luar mode Monitor"
    );

    c.send(&Message::SetMode(SetMode { m: Mode::Monitor }))
        .await
        .unwrap();
    let m = wait_for(&mut c, |m| match m {
        Message::Metrics(m) => Some(m),
        _ => None,
    })
    .await;
    assert!(!m.cpu.is_empty());
    assert!(
        m.tcpu.is_none() && m.gpu.is_none(),
        "tidak ada nilai placeholder"
    );

    // Pindah mode → telemetri berhenti.
    c.send(&Message::SetMode(SetMode { m: Mode::Clock }))
        .await
        .unwrap();
    wait_for(&mut c, |m| match m {
        Message::ModeState(s) if s.m == Mode::Clock && s.ok => Some(()),
        _ => None,
    })
    .await;
    while c.recv_timeout(Duration::from_millis(100)).await.is_ok() {}
    assert!(tokio::time::timeout(Duration::from_millis(700), async {
        loop {
            if let Ok(Message::Metrics(_)) = c.recv().await {
                return;
            }
        }
    })
    .await
    .is_err());
}

#[tokio::test]
async fn unavailable_mode_is_rejected_honestly() {
    let r = rig_with(cfg()).await;
    let (mut c, _t) = r.pair(Id16::random()).await;
    // FakeMedia melaporkan now_playing=true, jadi Music ada; Idle selalu boleh.
    c.send(&Message::SetMode(SetMode { m: Mode::Idle }))
        .await
        .unwrap();
    let ok = wait_for(&mut c, |m| match m {
        Message::ModeState(s) => Some(s.ok),
        _ => None,
    })
    .await;
    assert!(ok);
}

#[tokio::test]
async fn deck_only_fires_actions_wired_to_buttons_of_the_active_profile() {
    let r = rig().await;
    let (mut c, _t) = r.pair(Id16::random()).await;

    c.send(&Message::SetMode(SetMode { m: Mode::Deck }))
        .await
        .unwrap();
    let profile = wait_for(&mut c, |m| match m {
        Message::DeckProfile(p) => Some(p),
        _ => None,
    })
    .await;
    assert_eq!(profile.btns.len(), 6);
    let play = profile.btns.iter().find(|b| b.aid == "play").unwrap();

    c.send(&Message::DeckPress(DeckPress {
        aid: "play".into(),
        i: play.i,
    }))
    .await
    .unwrap();
    let fb = wait_for(&mut c, |m| match m {
        Message::DeckFeedback(f) => Some(f),
        _ => None,
    })
    .await;
    assert!(fb.ok);
    assert_eq!(
        r.ran.lock().unwrap().as_slice(),
        [Action::MediaKey {
            key: "media_play".into()
        }]
    );

    // aid yang tidak ada di profil, dan aid benar dengan indeks sel salah, keduanya ditolak.
    for (aid, i) in [("rm-rf", 0u16), ("play", 5u16)] {
        c.send(&Message::DeckPress(DeckPress { aid: aid.into(), i }))
            .await
            .unwrap();
        let fb = wait_for(&mut c, |m| match m {
            Message::DeckFeedback(f) => Some(f),
            _ => None,
        })
        .await;
        assert!(!fb.ok, "{aid}/{i} tidak boleh jalan");
    }
    assert_eq!(r.ran.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn artwork_is_chunked_within_the_frame_budget() {
    let r = rig().await;
    let (mut c, _t) = r.pair(Id16::random()).await;
    c.send(&Message::GetArtwork(GetArtwork {
        id: "fake-art".into(),
    }))
    .await
    .unwrap();

    let mut total = 0usize;
    let mut expected_n = None;
    let mut next_i = 0u16;
    while expected_n.is_none_or(|n| next_i < n) {
        let ch = wait_for(&mut c, |m| match m {
            Message::ArtworkChunk(c) => Some(c),
            _ => None,
        })
        .await;
        assert!(ch.b.len() <= 8 * 1024);
        assert_eq!(ch.i, next_i, "chunk harus berurutan");
        expected_n = Some(ch.n);
        total += ch.b.len();
        next_i += 1;
    }
    assert_eq!(total, 20_000);
}

// --------------------------------------------------------------- discovery

#[tokio::test]
async fn discovery_answers_valid_requests_and_marks_known_devices() {
    let r = rig().await;
    let target = r.host.discovery_addr();
    let dev = Id16::random();

    let found = tab_probe::discover::discover_at(dev, &[target], Duration::from_millis(800))
        .await
        .unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].response.hid, r.host.host_id());
    assert!(!found[0].response.known);
    assert_eq!(found[0].addr.port(), r.addr().port());
    assert_eq!(found[0].public_key, r.host.public_key());

    let (_c, _t) = r.pair(dev).await;
    let found = tab_probe::discover::discover_at(dev, &[target], Duration::from_millis(800))
        .await
        .unwrap();
    assert!(
        found[0].response.known,
        "perangkat yang sudah dipasangkan ditandai"
    );

    let other =
        tab_probe::discover::discover_at(Id16::random(), &[target], Duration::from_millis(800))
            .await
            .unwrap();
    assert!(
        !other[0].response.known,
        "known tidak bocor ke perangkat lain"
    );
}

#[tokio::test]
async fn discovery_ignores_foreign_and_wrong_version_datagrams() {
    let r = rig().await;
    let sock = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let target = r.host.discovery_addr();

    sock.send_to(b"hello world", target).await.unwrap();
    let mut bad_version =
        tab_protocol::discovery::encode_request(&tab_protocol::discovery::DiscoverRequest {
            dev: Id16::random(),
            pv: 99,
        })
        .unwrap();
    sock.send_to(&bad_version, target).await.unwrap();
    bad_version[4] = 77; // versi wire tak dikenal
    sock.send_to(&bad_version, target).await.unwrap();

    let mut buf = [0u8; 1500];
    let got = tokio::time::timeout(Duration::from_millis(500), sock.recv_from(&mut buf)).await;
    assert!(got.is_err(), "tidak boleh ada balasan untuk trafik asing");
}

#[tokio::test]
async fn discovery_is_rate_limited_per_source() {
    let r = rig().await;
    let sock = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let target = r.host.discovery_addr();
    let req = tab_protocol::discovery::encode_request(&tab_protocol::discovery::DiscoverRequest {
        dev: Id16::random(),
        pv: 1,
    })
    .unwrap();
    for _ in 0..5 {
        sock.send_to(&req, target).await.unwrap();
    }
    let mut replies = 0;
    let mut buf = [0u8; 1500];
    while tokio::time::timeout(Duration::from_millis(400), sock.recv_from(&mut buf))
        .await
        .is_ok()
    {
        replies += 1;
    }
    assert_eq!(replies, 1, "5 permintaan beruntun dalam 200 ms → 1 balasan");
}

#[tokio::test]
async fn shutdown_closes_sessions_politely() {
    let r = rig().await;
    let (mut c, _t) = r.pair(Id16::random()).await;
    let Rig { host, .. } = r;
    host.shutdown().await.unwrap();
    let mut saw_bye = false;
    loop {
        match c.recv_timeout(Duration::from_secs(3)).await {
            Ok(Message::Bye(_)) => saw_bye = true,
            Ok(_) => {}
            Err(ClientError::Closed) => break,
            Err(e) => panic!("{e}"),
        }
    }
    assert!(saw_bye);
}

// ---------------------------------------------------------- restart host

/// `TokenStore` yang bisa dibagi dua instans host, meniru keyring OS yang bertahan antar proses.
#[derive(Clone)]
struct SharedStore(Arc<Mutex<tab_host::MemoryTokenStore>>);

impl tab_host::TokenStore for SharedStore {
    fn keypair(&self) -> Result<tab_protocol::noise::StaticKeypair, tab_host::StoreError> {
        self.0.lock().unwrap().keypair()
    }
    fn devices(&self) -> Result<Vec<tab_host::DeviceEntry>, tab_host::StoreError> {
        self.0.lock().unwrap().devices()
    }
    fn upsert(&mut self, e: &tab_host::DeviceEntry) -> Result<(), tab_host::StoreError> {
        self.0.lock().unwrap().upsert(e)
    }
    fn remove(&mut self, d: Id16) -> Result<(), tab_host::StoreError> {
        self.0.lock().unwrap().remove(d)
    }
}

#[tokio::test]
async fn host_restart_keeps_pairing_and_client_resumes_without_pin() {
    let store = SharedStore(Arc::new(Mutex::new(tab_host::MemoryTokenStore::new())));
    let mut deps = HostDeps::fakes();
    deps.store = Box::new(store.clone());
    let host = HostHandle::start(cfg(), deps).await.unwrap();
    let port = host.session_addr().port();
    let pk = host.public_key();
    let hid = host.host_id();

    // Pairing di host pertama.
    let dev = Id16::random();
    let ticket = host.begin_pairing().await.unwrap();
    let mut c = Client::connect(
        host.session_addr(),
        &pk,
        DeviceInfo::probe(dev),
        Credential::Pair,
    )
    .await
    .unwrap();
    assert!(matches!(
        c.hello().await.unwrap(),
        Outcome::PairRequired { .. }
    ));
    let PinOutcome::Paired { token, .. } = c.submit_pin(&ticket.pin).await.unwrap() else {
        panic!("pairing gagal");
    };

    // Host mati (sesi hidup diputus sopan), lalu hidup lagi di port yang sama.
    host.shutdown().await.unwrap();
    let mut saw_close = false;
    while let Ok(m) = c.recv_timeout(Duration::from_secs(2)).await {
        saw_close |= matches!(m, Message::Bye(_));
    }
    assert!(saw_close, "sesi lama harus ditutup dengan Bye");

    let mut cfg2 = cfg();
    cfg2.session_port = port;
    let mut deps2 = HostDeps::fakes();
    deps2.store = Box::new(store);
    let host2 = HostHandle::start(cfg2, deps2).await.unwrap();
    assert_eq!(host2.public_key(), pk, "kunci host stabil antar restart");
    assert_eq!(host2.host_id(), hid);

    // Client memakai token lama: tanpa PIN, tanpa pairing ulang.
    let mut again = Client::connect(
        host2.session_addr(),
        &pk,
        DeviceInfo::probe(dev),
        Credential::Resume(token),
    )
    .await
    .unwrap();
    assert!(matches!(again.hello().await.unwrap(), Outcome::Welcome(_)));
}
