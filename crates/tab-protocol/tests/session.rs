//! Uji handshake dan sesi terenkripsi dari kedua sisi sekaligus, karena kesalahan pola
//! Noise hanya muncul saat initiator dan responder benar-benar dipertemukan.

use tab_protocol::message::*;
use tab_protocol::noise::{fingerprint, generate_static, Decoded, Handshake, NoiseError};
use tab_protocol::{Id16, Message, TOKEN_LEN};

/// Jalankan handshake sampai selesai dan kembalikan sesi kedua sisi.
fn complete(
    mut client: Handshake,
    mut host: Handshake,
) -> Result<(tab_protocol::noise::Session, tab_protocol::noise::Session), NoiseError> {
    let msg1 = client.next_message()?;
    host.read_message(&msg1)?;
    let msg2 = host.next_message()?;
    client.read_message(&msg2)?;
    assert!(
        client.is_finished() && host.is_finished(),
        "NK selesai dalam 2 pesan"
    );
    Ok((client.into_session()?, host.into_session()?))
}

fn hello() -> Message {
    Message::Hello(Hello {
        pv: tab_protocol::PROTOCOL_VERSION,
        dev: Id16::random(),
        name: "Pixel 8".into(),
        plat: "android".into(),
        platv: "15".into(),
        app: "0.1.0".into(),
        scr: Screen {
            w: 1080,
            h: 2400,
            dpi: 2.625,
        },
    })
}

#[test]
fn pairing_handshake_then_traffic_both_directions() {
    let host_keys = generate_static().unwrap();
    let client = Handshake::client_pairing(&host_keys.public).unwrap();
    let host = Handshake::host_pairing(&host_keys.private).unwrap();
    let (mut cs, mut hs) = complete(client, host).unwrap();

    let sent = hello();
    let frame = cs.encrypt(&sent).unwrap();
    assert_ne!(frame, Vec::<u8>::new());
    assert_eq!(hs.decrypt(&frame).unwrap(), Decoded::Known(sent));

    let reply = Message::PairRequired(PairRequired { ttl_ms: 120_000 });
    let frame = hs.encrypt(&reply).unwrap();
    assert_eq!(cs.decrypt(&frame).unwrap(), Decoded::Known(reply));
}

#[test]
fn many_messages_keep_nonce_in_step() {
    let host_keys = generate_static().unwrap();
    let (mut cs, mut hs) = complete(
        Handshake::client_pairing(&host_keys.public).unwrap(),
        Handshake::host_pairing(&host_keys.private).unwrap(),
    )
    .unwrap();

    // Jalur input mengirim ribuan frame per menit; urutan nonce harus tetap sejalan.
    for s in 0..500u64 {
        let msg = Message::InputBatch(InputBatch {
            s,
            ev: vec![InputEvent::PointerMove { dx: 1.5, dy: -2.0 }],
        });
        let frame = cs.encrypt(&msg).unwrap();
        assert_eq!(hs.decrypt(&frame).unwrap(), Decoded::Known(msg));
    }
}

#[test]
fn resume_with_matching_token_succeeds() {
    let host_keys = generate_static().unwrap();
    let token = [9u8; TOKEN_LEN];
    let (mut cs, mut hs) = complete(
        Handshake::client_resume(&host_keys.public, &token).unwrap(),
        Handshake::host_resume(&host_keys.private, &token).unwrap(),
    )
    .unwrap();

    let msg = Message::Ping(Ping { n: 1, tc: 42 });
    let frame = cs.encrypt(&msg).unwrap();
    assert_eq!(hs.decrypt(&frame).unwrap(), Decoded::Known(msg));
}

#[test]
fn resume_with_revoked_token_fails_before_any_payload() {
    let host_keys = generate_static().unwrap();
    let client_token = [9u8; TOKEN_LEN];
    let host_token = [8u8; TOKEN_LEN]; // token sudah dicabut / diganti di host

    let mut client = Handshake::client_resume(&host_keys.public, &client_token).unwrap();
    let mut host = Handshake::host_resume(&host_keys.private, &host_token).unwrap();

    let msg1 = client.next_message().unwrap();
    host.read_message(&msg1).unwrap();
    let msg2 = host.next_message().unwrap();
    // PSK bercampur pada pesan kedua, jadi client-lah yang mendeteksi ketidakcocokan —
    // dan itu terjadi sebelum satu byte payload pun dikirim.
    assert!(client.read_message(&msg2).is_err());
}

#[test]
fn resume_rejects_wrong_host_key() {
    let real = generate_static().unwrap();
    let impostor = generate_static().unwrap();
    let token = [9u8; TOKEN_LEN];

    let mut client = Handshake::client_resume(&real.public, &token).unwrap();
    let mut host = Handshake::host_resume(&impostor.private, &token).unwrap();

    let msg1 = client.next_message().unwrap();
    // Host penipu tidak memiliki kunci privat yang dipin client, jadi handshake mati di sini.
    assert!(host.read_message(&msg1).is_err());
}

#[test]
fn token_length_is_enforced() {
    let keys = generate_static().unwrap();
    assert!(matches!(
        Handshake::client_resume(&keys.public, &[1u8; 16]),
        Err(NoiseError::TokenLength(16))
    ));
    assert!(matches!(
        Handshake::host_resume(&keys.private, &[]),
        Err(NoiseError::TokenLength(0))
    ));
}

#[test]
fn unknown_message_type_is_ignored_not_fatal() {
    let host_keys = generate_static().unwrap();
    let (mut cs, mut hs) = complete(
        Handshake::client_pairing(&host_keys.public).unwrap(),
        Handshake::host_pairing(&host_keys.private).unwrap(),
    )
    .unwrap();

    // Pesan dari versi protokol yang lebih baru.
    let mut future = Vec::new();
    ciborium::into_writer(
        &ciborium::Value::Map(vec![
            (
                ciborium::Value::Text("t".into()),
                ciborium::Value::Text("SecondScreenFrame".into()),
            ),
            (
                ciborium::Value::Text("w".into()),
                ciborium::Value::Integer(1920.into()),
            ),
        ]),
        &mut future,
    )
    .unwrap();

    let frame = cs.encrypt_raw(&future).unwrap();
    assert_eq!(
        hs.decrypt(&frame).unwrap(),
        Decoded::Unknown("SecondScreenFrame".into())
    );

    // Sesi tetap hidup setelah pesan tak dikenal.
    let msg = Message::Ping(Ping { n: 7, tc: 1 });
    let frame = cs.encrypt(&msg).unwrap();
    assert_eq!(hs.decrypt(&frame).unwrap(), Decoded::Known(msg));
}

#[test]
fn known_type_with_broken_fields_is_an_error() {
    let host_keys = generate_static().unwrap();
    let (mut cs, mut hs) = complete(
        Handshake::client_pairing(&host_keys.public).unwrap(),
        Handshake::host_pairing(&host_keys.private).unwrap(),
    )
    .unwrap();

    // Tag dikenal tetapi field wajibnya tidak ada: ini rusak, bukan "versi lebih baru".
    let mut broken = Vec::new();
    ciborium::into_writer(
        &ciborium::Value::Map(vec![(
            ciborium::Value::Text("t".into()),
            ciborium::Value::Text("Hello".into()),
        )]),
        &mut broken,
    )
    .unwrap();

    let frame = cs.encrypt_raw(&broken).unwrap();
    assert!(matches!(hs.decrypt(&frame), Err(NoiseError::Malformed(_))));
}

#[test]
fn tampered_frame_is_rejected() {
    let host_keys = generate_static().unwrap();
    let (mut cs, mut hs) = complete(
        Handshake::client_pairing(&host_keys.public).unwrap(),
        Handshake::host_pairing(&host_keys.private).unwrap(),
    )
    .unwrap();

    let mut frame = cs.encrypt(&hello()).unwrap();
    let last = frame.len() - 1;
    frame[last] ^= 0xff;
    assert!(hs.decrypt(&frame).is_err());
}

#[test]
fn replayed_frame_is_rejected() {
    let host_keys = generate_static().unwrap();
    let (mut cs, mut hs) = complete(
        Handshake::client_pairing(&host_keys.public).unwrap(),
        Handshake::host_pairing(&host_keys.private).unwrap(),
    )
    .unwrap();

    let frame = cs.encrypt(&Message::Ping(Ping { n: 1, tc: 1 })).unwrap();
    assert!(hs.decrypt(&frame).is_ok());
    // Nonce sudah maju: frame yang sama tidak bisa dipakai dua kali.
    assert!(hs.decrypt(&frame).is_err());
}

#[test]
fn fingerprint_is_stable_and_key_specific() {
    let a = generate_static().unwrap();
    let b = generate_static().unwrap();
    assert_eq!(fingerprint(&a.public), fingerprint(&a.public));
    assert_ne!(fingerprint(&a.public), fingerprint(&b.public));
    assert_eq!(fingerprint(&a.public).len(), 32);
}

#[test]
fn debug_output_never_leaks_private_key() {
    let keys = generate_static().unwrap();
    let rendered = format!("{keys:?}");
    let private_hex = tab_protocol::noise::hex(&keys.private);
    assert!(
        !rendered.contains(&private_hex),
        "kunci privat bocor ke Debug"
    );
}
