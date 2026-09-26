//! Kontrak wire format.
//!
//! Test ini juga menghasilkan fixture di `protocol/fixtures/` yang dipakai uji kontrak sisi
//! Kotlin, sehingga kedua implementasi diukur terhadap byte yang sama persis, bukan terhadap
//! penafsiran masing-masing atas dokumen spec.

use serde_bytes::ByteBuf;
use tab_protocol::message::*;
use tab_protocol::{Id16, Message, MAX_INPUT_EVENTS, TELEMETRY_FRAME_LIMIT};

fn cbor(msg: &Message) -> Vec<u8> {
    let mut out = Vec::new();
    ciborium::into_writer(msg, &mut out).expect("serialisasi");
    out
}

fn back(bytes: &[u8]) -> Message {
    ciborium::from_reader(bytes).expect("deserialisasi")
}

fn id() -> Id16 {
    Id16::from_hex("0123456789abcdef0123456789abcdef").unwrap()
}

/// Satu contoh per pesan, dengan nama file untuk fixture.
fn catalog() -> Vec<(&'static str, Message)> {
    vec![
        (
            "hello",
            Message::Hello(Hello {
                pv: 1,
                dev: id(),
                name: "Pixel 8".into(),
                plat: "android".into(),
                platv: "15".into(),
                app: "0.1.0".into(),
                scr: Screen {
                    w: 1080,
                    h: 2400,
                    dpi: 2.625,
                },
            }),
        ),
        (
            "welcome",
            Message::Welcome(Welcome {
                pv: 1,
                hid: id(),
                name: "MacBook Pro".into(),
                os: "macos".into(),
                osv: "15.6".into(),
                app: "0.1.0".into(),
                sid: id(),
                caps: Capabilities {
                    modes: vec![
                        Mode::Trackpad,
                        Mode::Deck,
                        Mode::Monitor,
                        Mode::Music,
                        Mode::Clock,
                    ],
                    clipboard: true,
                    gestures: vec!["spaces_left".into(), "spaces_right".into()],
                    now_playing: true,
                    media_seek: true,
                    media_volume: false,
                    obs: false,
                    temps: false,
                    gpu: false,
                    second_screen: false,
                    gamepad: false,
                    max_frame: tab_protocol::MAX_FRAME as u32,
                },
            }),
        ),
        (
            "pair_required",
            Message::PairRequired(PairRequired { ttl_ms: 120_000 }),
        ),
        (
            "pair_request",
            Message::PairRequest(PairRequest {
                pin: "048213".into(),
            }),
        ),
        (
            "pair_ok",
            Message::PairOk(PairOk {
                tok: ByteBuf::from(vec![0xab; 32]),
            }),
        ),
        (
            "ping",
            Message::Ping(Ping {
                n: 12,
                tc: 1_700_000_000_000,
            }),
        ),
        (
            "pong",
            Message::Pong(Pong {
                n: 12,
                tc: 1_700_000_000_000,
                th: 999,
            }),
        ),
        ("set_mode", Message::SetMode(SetMode { m: Mode::Trackpad })),
        (
            "mode_state",
            Message::ModeState(ModeState {
                m: Mode::Trackpad,
                ok: true,
                msg: None,
            }),
        ),
        (
            "error_pin_invalid",
            Message::Error(ProtoError {
                c: ErrorCode::PinInvalid,
                msg: "PIN salah".into(),
                attempts_left: Some(4),
            }),
        ),
        (
            "input_batch",
            Message::InputBatch(InputBatch {
                s: 7,
                ev: vec![
                    InputEvent::PointerMove {
                        dx: 12.5,
                        dy: -3.25,
                    },
                    InputEvent::PointerAbs { x: 0.5, y: 0.25 },
                    InputEvent::PointerButton {
                        b: Button::L,
                        d: true,
                    },
                    InputEvent::Scroll {
                        dx: 0.0,
                        dy: -40.0,
                        ph: ScrollPhase::U,
                        mom: false,
                    },
                    InputEvent::Gesture {
                        g: "spaces_left".into(),
                        f: 4,
                    },
                    InputEvent::Key {
                        k: "c".into(),
                        d: true,
                    },
                    InputEvent::Text { s: "halo".into() },
                    InputEvent::Modifiers {
                        m: modifiers::CTRL | modifiers::SHIFT,
                    },
                ],
            }),
        ),
        (
            "input_settings",
            Message::InputSettings(InputSettings {
                sens: 1.4,
                natural: true,
            }),
        ),
        (
            "deck_profile",
            Message::DeckProfile(DeckProfile {
                id: "default".into(),
                name: "Streaming".into(),
                cols: 4,
                rows: 3,
                btns: vec![DeckButton {
                    i: 0,
                    lbl: "Mute".into(),
                    ic: Some("mic-off".into()),
                    col: Some("#a62126".into()),
                    aid: "act-mute".into(),
                }],
            }),
        ),
        (
            "deck_press",
            Message::DeckPress(DeckPress {
                aid: "act-mute".into(),
                i: 0,
            }),
        ),
        (
            "metrics",
            Message::Metrics(Metrics {
                cpu: vec![12.5, 30.0, 8.25, 44.0],
                cpua: 23.6875,
                mu: 9_000_000_000,
                mt: 17_179_869_184,
                su: 0,
                st: 0,
                disks: vec![Disk {
                    n: "Macintosh HD".into(),
                    u: 300_000_000_000,
                    t: 994_662_584_320,
                }],
                rx: 128_000,
                tx: 32_000,
                pwr: Power {
                    ac: true,
                    pct: Some(88),
                },
                tcpu: None,
                gpu: None,
            }),
        ),
        (
            "now_playing",
            Message::NowPlaying(NowPlaying {
                title: "Contoh Lagu".into(),
                artist: "Contoh Artis".into(),
                album: "Contoh Album".into(),
                dur: 214_000,
                pos: 61_500,
                play: true,
                art: Some("art-1".into()),
                lyr: false,
            }),
        ),
        (
            "media_command",
            Message::MediaCommand(MediaCommand {
                c: MediaCmd::Toggle,
            }),
        ),
    ]
}

#[test]
fn every_message_survives_a_roundtrip() {
    for (name, msg) in catalog() {
        let bytes = cbor(&msg);
        assert_eq!(back(&bytes), msg, "roundtrip gagal untuk {name}");
    }
}

#[test]
fn discriminator_is_the_literal_t_key() {
    // Sisi Kotlin memakai classDiscriminator = "t"; bila ini berubah, kedua sisi berhenti
    // saling mengerti.
    let value: ciborium::Value =
        ciborium::from_reader(cbor(&Message::Ping(Ping { n: 1, tc: 2 })).as_slice()).unwrap();
    let map = value.as_map().expect("pesan adalah map CBOR");
    let tag = map
        .iter()
        .find(|(k, _)| k.as_text() == Some("t"))
        .map(|(_, v)| v.as_text().unwrap().to_string());
    assert_eq!(tag.as_deref(), Some("Ping"));
}

#[test]
fn ids_are_cbor_byte_strings_not_number_arrays() {
    let value: ciborium::Value =
        ciborium::from_reader(cbor(&Message::SetMode(SetMode { m: Mode::Deck })).as_slice())
            .unwrap();
    assert!(value.as_map().is_some());

    let hello = catalog()
        .into_iter()
        .find(|(n, _)| *n == "hello")
        .unwrap()
        .1;
    let value: ciborium::Value = ciborium::from_reader(cbor(&hello).as_slice()).unwrap();
    let dev = value
        .as_map()
        .unwrap()
        .iter()
        .find(|(k, _)| k.as_text() == Some("dev"))
        .map(|(_, v)| v.clone())
        .unwrap();
    assert!(
        dev.as_bytes().is_some(),
        "device id harus byte string, bukan array angka"
    );
    assert_eq!(dev.as_bytes().unwrap().len(), 16);
}

#[test]
fn absent_capabilities_are_omitted_entirely() {
    // Suhu dan GPU yang tidak tersedia tidak boleh muncul sebagai nilai nol.
    let metrics = catalog()
        .into_iter()
        .find(|(n, _)| *n == "metrics")
        .unwrap()
        .1;
    let value: ciborium::Value = ciborium::from_reader(cbor(&metrics).as_slice()).unwrap();
    let keys: Vec<String> = value
        .as_map()
        .unwrap()
        .iter()
        .filter_map(|(k, _)| k.as_text().map(str::to_string))
        .collect();
    assert!(
        !keys.contains(&"tcpu".to_string()),
        "field suhu seharusnya hilang: {keys:?}"
    );
    assert!(
        !keys.contains(&"gpu".to_string()),
        "field gpu seharusnya hilang: {keys:?}"
    );
}

#[test]
fn a_full_input_batch_fits_comfortably_in_one_small_frame() {
    // Batas 64 event per batch dipilih agar satu batch tetap jauh di bawah batas frame,
    // sehingga jalur input tidak pernah terpaksa dipecah.
    let msg = Message::InputBatch(InputBatch {
        s: u64::MAX,
        ev: (0..MAX_INPUT_EVENTS)
            .map(|i| InputEvent::PointerMove {
                dx: i as f32,
                dy: -(i as f32),
            })
            .collect(),
    });
    let size = cbor(&msg).len();
    assert!(size < TELEMETRY_FRAME_LIMIT, "batch penuh {size} byte");
}

#[test]
fn fixtures_for_the_kotlin_contract_test_are_current() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../protocol/fixtures")
        .canonicalize()
        .expect("folder protocol/fixtures ada");

    let writing = std::env::var("TAB_WRITE_FIXTURES").is_ok();
    for (name, msg) in catalog() {
        let path = dir.join(format!("{name}.cbor"));
        let bytes = cbor(&msg);
        if writing {
            std::fs::write(&path, &bytes).expect("tulis fixture");
            continue;
        }
        let stored = std::fs::read(&path).unwrap_or_else(|_| {
            panic!("fixture {name} belum ada — jalankan TAB_WRITE_FIXTURES=1 cargo test")
        });
        assert_eq!(
            stored, bytes,
            "fixture {name} tidak sesuai kode; bila perubahan ini memang disengaja, \
             perbarui PROTOCOL.md dan sisi Kotlin lalu regenerasi fixture"
        );
        assert_eq!(
            back(&stored),
            msg,
            "fixture {name} tidak bisa dibaca kembali"
        );
    }
}
