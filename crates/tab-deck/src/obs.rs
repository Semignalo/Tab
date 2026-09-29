//! Klien obs-websocket v5 (OBS 28+): cukup untuk berganti scene.
//!
//! Koneksi bersifat opsional. Kapabilitas `obs` diumumkan **hanya** bila OBS benar-benar
//! menjawab handshake; kalau tidak, client menyembunyikan aksi OBS (bukan menampilkannya lalu
//! gagal). Koneksi dibuka per pemanggilan — di localhost murah, dan tidak ada state koneksi
//! yang bisa basi saat OBS dinyalakan ulang.

use base64::{engine::general_purpose::STANDARD as B64, Engine};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tungstenite::{client::IntoClientRequest, Message};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObsConfig {
    pub host: String,
    pub port: u16,
    /// Kosong bila autentikasi OBS dimatikan.
    pub password: String,
}

impl Default for ObsConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".into(),
            port: 4455,
            password: String::new(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ObsError {
    #[error("tidak bisa tersambung ke OBS: {0}")]
    Connect(String),
    #[error("OBS menolak autentikasi (password salah?)")]
    Auth,
    #[error("protokol OBS tidak terduga: {0}")]
    Protocol(String),
    #[error("OBS menolak permintaan: {0}")]
    Request(String),
}

/// `base64(sha256(base64(sha256(password + salt)) + challenge))` — rumus autentikasi v5.
pub fn auth_string(password: &str, salt: &str, challenge: &str) -> String {
    let secret = B64.encode(Sha256::digest(format!("{password}{salt}")));
    B64.encode(Sha256::digest(format!("{secret}{challenge}")))
}

const TIMEOUT: Duration = Duration::from_millis(1500);

type Socket = tungstenite::WebSocket<TcpStream>;

fn read_json(ws: &mut Socket) -> Result<Value, ObsError> {
    loop {
        match ws.read().map_err(|e| ObsError::Connect(e.to_string()))? {
            Message::Text(t) => {
                return serde_json::from_str(t.as_str())
                    .map_err(|e| ObsError::Protocol(e.to_string()))
            }
            Message::Close(_) => return Err(ObsError::Auth),
            _ => continue,
        }
    }
}

/// Buka koneksi dan selesaikan Hello/Identify.
fn connect(cfg: &ObsConfig) -> Result<Socket, ObsError> {
    let addr = (cfg.host.as_str(), cfg.port)
        .to_socket_addrs()
        .map_err(|e| ObsError::Connect(e.to_string()))?
        .next()
        .ok_or_else(|| ObsError::Connect("alamat tidak ditemukan".into()))?;
    let stream =
        TcpStream::connect_timeout(&addr, TIMEOUT).map_err(|e| ObsError::Connect(e.to_string()))?;
    stream.set_read_timeout(Some(TIMEOUT)).ok();
    stream.set_write_timeout(Some(TIMEOUT)).ok();
    stream.set_nodelay(true).ok();

    let req = format!("ws://{}:{}/", cfg.host, cfg.port)
        .into_client_request()
        .map_err(|e| ObsError::Connect(e.to_string()))?;
    let (mut ws, _) =
        tungstenite::client(req, stream).map_err(|e| ObsError::Connect(e.to_string()))?;

    let hello = read_json(&mut ws)?;
    if hello["op"] != 0 {
        return Err(ObsError::Protocol("Hello tidak diterima".into()));
    }
    let mut identify = json!({ "rpcVersion": 1, "eventSubscriptions": 0 });
    if let Some(auth) = hello["d"].get("authentication").filter(|a| !a.is_null()) {
        let salt = auth["salt"].as_str().unwrap_or_default();
        let challenge = auth["challenge"].as_str().unwrap_or_default();
        identify["authentication"] = json!(auth_string(&cfg.password, salt, challenge));
    }
    ws.send(Message::text(json!({ "op": 1, "d": identify }).to_string()))
        .map_err(|e| ObsError::Connect(e.to_string()))?;
    let identified = read_json(&mut ws)?;
    if identified["op"] != 2 {
        return Err(ObsError::Auth);
    }
    Ok(ws)
}

fn request(ws: &mut Socket, kind: &str, data: Value) -> Result<Value, ObsError> {
    let id = format!("tab-{kind}");
    ws.send(Message::text(
        json!({ "op": 6, "d": { "requestType": kind, "requestId": id, "requestData": data } })
            .to_string(),
    ))
    .map_err(|e| ObsError::Connect(e.to_string()))?;
    loop {
        let m = read_json(ws)?;
        if m["op"] == 7 && m["d"]["requestId"] == id {
            let status = &m["d"]["requestStatus"];
            return if status["result"] == true {
                Ok(m["d"]["responseData"].clone())
            } else {
                Err(ObsError::Request(
                    status["comment"].as_str().unwrap_or("gagal").to_owned(),
                ))
            };
        }
        // op lain (event yang terlanjur dilanggan) dilewati.
    }
}

/// Uji cepat: tersambung dan terautentikasi.
pub fn ping(cfg: &ObsConfig) -> Result<(), ObsError> {
    let mut ws = connect(cfg)?;
    let _ = ws.close(None);
    Ok(())
}

pub fn set_scene(cfg: &ObsConfig, scene: &str) -> Result<(), ObsError> {
    let mut ws = connect(cfg)?;
    let res = request(
        &mut ws,
        "SetCurrentProgramScene",
        json!({ "sceneName": scene }),
    );
    let _ = ws.close(None);
    res.map(|_| ())
}

/// Pegangan bersama untuk host dan runner: konfigurasi + hasil uji ketersediaan (di-cache
/// singkat supaya setiap `Welcome` tidak selalu membuka koneksi ke OBS).
#[derive(Default)]
pub struct ObsHandle {
    cfg: Mutex<Option<ObsConfig>>,
    probe: Mutex<Option<(Instant, bool)>>,
}

const PROBE_TTL: Duration = Duration::from_secs(5);

impl ObsHandle {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn configure(&self, cfg: Option<ObsConfig>) {
        *self.cfg.lock().unwrap() = cfg;
        *self.probe.lock().unwrap() = None;
    }

    pub fn is_configured(&self) -> bool {
        self.cfg.lock().unwrap().is_some()
    }

    /// `true` hanya bila OBS dikonfigurasi **dan** menjawab sekarang.
    pub fn available(&self) -> bool {
        let Some(cfg) = self.cfg.lock().unwrap().clone() else {
            return false;
        };
        if let Some((at, ok)) = *self.probe.lock().unwrap() {
            if at.elapsed() < PROBE_TTL {
                return ok;
            }
        }
        let ok = ping(&cfg).is_ok();
        *self.probe.lock().unwrap() = Some((Instant::now(), ok));
        ok
    }

    pub fn set_scene(&self, scene: &str) -> Result<(), ObsError> {
        let cfg = self
            .cfg
            .lock()
            .unwrap()
            .clone()
            .ok_or_else(|| ObsError::Connect("OBS belum dikonfigurasi".into()))?;
        set_scene(&cfg, scene)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::thread;

    /// Server OBS palsu: menjalankan Hello → Identify → satu permintaan, lalu melaporkan yang ia lihat.
    fn fake_obs(
        password: Option<&'static str>,
        scene_ok: bool,
    ) -> (u16, thread::JoinHandle<Vec<Value>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let h = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut ws = tungstenite::accept(stream).unwrap();
            let mut seen = Vec::new();
            let mut hello = json!({ "obsWebSocketVersion": "5.5.0", "rpcVersion": 1 });
            if password.is_some() {
                hello["authentication"] = json!({ "challenge": "ch4llenge", "salt": "s4lt" });
            }
            ws.send(Message::text(json!({ "op": 0, "d": hello }).to_string()))
                .unwrap();

            let identify: Value =
                serde_json::from_str(ws.read().unwrap().to_text().unwrap()).unwrap();
            seen.push(identify.clone());
            if let Some(pw) = password {
                let expected = auth_string(pw, "s4lt", "ch4llenge");
                if identify["d"]["authentication"] != json!(expected) {
                    ws.close(None).ok();
                    return seen;
                }
            }
            ws.send(Message::text(
                json!({ "op": 2, "d": { "negotiatedRpcVersion": 1 } }).to_string(),
            ))
            .unwrap();

            while let Ok(m) = ws.read() {
                let Message::Text(t) = m else { break };
                let v: Value = serde_json::from_str(t.as_str()).unwrap();
                seen.push(v.clone());
                let id = v["d"]["requestId"].clone();
                let ty = v["d"]["requestType"].clone();
                let status = if scene_ok {
                    json!({ "result": true, "code": 100 })
                } else {
                    json!({ "result": false, "code": 600, "comment": "No source was found by the provided sceneName" })
                };
                ws.send(Message::text(
                    json!({ "op": 7, "d": { "requestType": ty, "requestId": id, "requestStatus": status } }).to_string(),
                ))
                .unwrap();
            }
            seen
        });
        (port, h)
    }

    fn cfg(port: u16, password: &str) -> ObsConfig {
        ObsConfig {
            host: "127.0.0.1".into(),
            port,
            password: password.into(),
        }
    }

    #[test]
    fn auth_string_matches_the_documented_construction() {
        // Dihitung manual sesuai spesifikasi obs-websocket v5 (dua tahap SHA-256 + base64).
        let secret = B64.encode(Sha256::digest(b"supersecretpasswordsalt"));
        let expected = B64.encode(Sha256::digest(format!("{secret}challenge")));
        assert_eq!(
            auth_string("supersecretpassword", "salt", "challenge"),
            expected
        );
        assert_eq!(auth_string("a", "b", "c").len(), 44, "base64 dari 32 byte");
        assert_ne!(auth_string("a", "b", "c"), auth_string("a", "b", "d"));
    }

    #[test]
    fn switches_scene_with_password() {
        let (port, server) = fake_obs(Some("rahasia"), true);
        set_scene(&cfg(port, "rahasia"), "Gaming").unwrap();
        let seen = server.join().unwrap();
        assert_eq!(seen[0]["op"], 1);
        assert_eq!(seen[0]["d"]["rpcVersion"], 1);
        assert_eq!(seen[1]["d"]["requestType"], "SetCurrentProgramScene");
        assert_eq!(seen[1]["d"]["requestData"]["sceneName"], "Gaming");
    }

    #[test]
    fn works_without_password_when_obs_has_auth_disabled() {
        let (port, server) = fake_obs(None, true);
        set_scene(&cfg(port, ""), "Chat").unwrap();
        let seen = server.join().unwrap();
        assert!(
            seen[0]["d"].get("authentication").is_none(),
            "tanpa tantangan → tanpa autentikasi"
        );
    }

    #[test]
    fn wrong_password_is_reported_as_auth_error() {
        let (port, server) = fake_obs(Some("benar"), true);
        let err = set_scene(&cfg(port, "salah"), "X").unwrap_err();
        assert!(matches!(err, ObsError::Auth), "{err:?}");
        server.join().unwrap();
    }

    #[test]
    fn unknown_scene_surfaces_obs_comment() {
        let (port, server) = fake_obs(None, false);
        let err = set_scene(&cfg(port, ""), "Tidak Ada").unwrap_err();
        assert!(
            matches!(&err, ObsError::Request(c) if c.contains("sceneName")),
            "{err:?}"
        );
        server.join().unwrap();
    }

    #[test]
    fn handle_reports_unavailable_when_unconfigured_or_unreachable() {
        let h = ObsHandle::new();
        assert!(!h.available(), "belum dikonfigurasi");
        assert!(h.set_scene("x").is_err());
        // Port yang tidak ada yang mendengarkan.
        let free = TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        h.configure(Some(cfg(free, "")));
        assert!(
            !h.available(),
            "OBS tidak menjawab → capability tidak boleh true"
        );
    }

    #[test]
    fn handle_is_available_when_obs_answers() {
        let (port, server) = fake_obs(None, true);
        let h = ObsHandle::new();
        h.configure(Some(cfg(port, "")));
        assert!(h.available());
        // Hasil di-cache: pemanggilan kedua tidak membuka koneksi baru (server sudah selesai).
        assert!(h.available());
        drop(server);
    }
}
