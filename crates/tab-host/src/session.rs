//! Satu koneksi TCP = satu sesi: preamble → handshake Noise → Hello → (pairing) → Welcome →
//! loop pesan.
//!
//! Semua kegagalan sebelum sesi terenkripsi berdiri (preamble rusak, dekripsi gagal, token
//! salah) berakhir dengan **menutup koneksi tanpa pesan apa pun**, supaya penyerang tidak
//! mendapat oracle untuk membedakan penyebab kegagalan.

use crate::store::{now_secs, DeviceEntry};
use crate::{HostEvent, Shared};
use rand::{rngs::OsRng, RngCore};
use std::collections::{hash_map::DefaultHasher, HashSet, VecDeque};
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::Duration;
use tab_deck::to_wire;
use tab_media::load_lyrics;
use tab_protocol::frame::{read_frame, FrameWriter};
use tab_protocol::message::*;
use tab_protocol::noise::{Decoded, Handshake, Session};
use tab_protocol::preamble::{self, Intent};
use tab_protocol::{
    Id16, MAX_CLIPBOARD, MAX_INPUT_EVENTS, MIN_TELEMETRY_INTERVAL_MS, PROTOCOL_VERSION,
    SESSION_TIMEOUT,
};
use tokio::net::tcp::OwnedWriteHalf;
use tokio::net::TcpStream;
use tokio::sync::{mpsc, watch};
use tokio::time::{interval, Instant, Interval, MissedTickBehavior};

/// Batas waktu tiap langkah sebelum sesi berdiri (preamble, handshake, Hello).
const STEP_TIMEOUT: Duration = Duration::from_secs(5);
/// Ukuran data per `ArtworkChunk`; dijaga di bawah 8 KiB per frame.
const ART_CHUNK: usize = 7000;
const MAX_TEXT_CHARS: usize = 1024;
const MAX_DELTA: f32 = 10_000.0;

pub(crate) async fn serve(shared: Arc<Shared>, stream: TcpStream, shutdown: watch::Receiver<bool>) {
    let _ = run(shared, stream, shutdown).await;
}

struct Conn {
    noise: Session,
    wr: OwnedWriteHalf,
    fw: FrameWriter,
    frames: mpsc::Receiver<Vec<u8>>,
}

impl Conn {
    async fn send(&mut self, msg: Message) -> Option<()> {
        let ct = self.noise.encrypt(&msg).ok()?;
        self.fw.write(&mut self.wr, &ct).await.ok()
    }

    async fn send_error(&mut self, c: ErrorCode, msg: &str) -> Option<()> {
        self.send(Message::Error(ProtoError {
            c,
            msg: msg.to_owned(),
            attempts_left: None,
        }))
        .await
    }

    /// Pesan sah berikutnya. `None` bila koneksi tutup, timeout, atau dekripsi gagal.
    /// Pesan bertipe tak dikenal dilewati (forward-compat).
    async fn next(&mut self, wait: Duration) -> Option<Message> {
        tokio::time::timeout(wait, async {
            loop {
                let frame = self.frames.recv().await?;
                match self.noise.decrypt(&frame).ok()? {
                    Decoded::Known(m) => return Some(m),
                    Decoded::Unknown(_) => continue,
                }
            }
        })
        .await
        .ok()?
    }
}

async fn read_step(stream: &mut TcpStream, buf: &mut Vec<u8>) -> Option<()> {
    tokio::time::timeout(STEP_TIMEOUT, read_frame(stream, buf))
        .await
        .ok()?
        .ok()
        .map(|_| ())
}

async fn run(
    shared: Arc<Shared>,
    mut stream: TcpStream,
    mut shutdown: watch::Receiver<bool>,
) -> Option<()> {
    let mut fw = FrameWriter::new();
    let mut buf = Vec::new();

    // 1. Preamble: siapa yang datang dan pola Noise mana yang dipakai.
    read_step(&mut stream, &mut buf).await?;
    let (intent, dev) = preamble::decode(&buf)?;

    let mut hs = match intent {
        Intent::Pair => Handshake::host_pairing(&shared.keys.private).ok()?,
        Intent::Resume => {
            // Perangkat tidak dikenal / token dicabut: tutup diam-diam, client kembali ke pairing.
            let entry = shared.registry.find(dev)?;
            Handshake::host_resume(&shared.keys.private, &entry.token).ok()?
        }
    };

    // 2. Handshake Noise (dua pesan).
    read_step(&mut stream, &mut buf).await?;
    hs.read_message(&buf).ok()?;
    let reply = hs.next_message().ok()?;
    fw.write(&mut stream, &reply).await.ok()?;
    let noise = hs.into_session().ok()?;

    // 3. Pisahkan baca/tulis. Pembaca berjalan di task sendiri karena `read_frame` tidak aman
    //    dibatalkan di tengah `select!`.
    let (mut rd, wr) = stream.into_split();
    let (tx, frames) = mpsc::channel::<Vec<u8>>(128);
    let reader = tokio::spawn(async move {
        let mut buf = Vec::new();
        while read_frame(&mut rd, &mut buf).await.is_ok() {
            if tx.send(buf.clone()).await.is_err() {
                break;
            }
        }
    });
    let mut conn = Conn {
        noise,
        wr,
        fw,
        frames,
    };

    let result = session_body(&shared, &mut conn, intent, dev, &mut shutdown).await;
    reader.abort();
    result
}

async fn session_body(
    shared: &Arc<Shared>,
    conn: &mut Conn,
    intent: Intent,
    dev: Id16,
    shutdown: &mut watch::Receiver<bool>,
) -> Option<()> {
    // 4. Hello.
    let Message::Hello(hello) = conn.next(STEP_TIMEOUT).await? else {
        return None;
    };
    if hello.dev != dev {
        return None;
    }
    if !tab_protocol::version_supported(hello.pv) {
        conn.send_error(
            ErrorCode::ProtocolUnsupported,
            "versi protokol tidak didukung",
        )
        .await;
        return None;
    }

    // 5. Pairing bila perangkat datang tanpa token.
    if intent == Intent::Pair {
        pair(shared, conn, &hello).await?;
    } else {
        shared.registry.touch(dev);
    }

    // 6. Welcome: sesi resmi berdiri. `obs` dihitung saat ini juga: capability itu hanya jujur
    //    bila OBS memang sedang menjawab (uji singkat, di-cache 5 detik).
    let mut caps = shared.caps.clone();
    let obs = Arc::clone(&shared.obs);
    caps.obs = tokio::task::spawn_blocking(move || obs.available())
        .await
        .unwrap_or(false);
    conn.send(Message::Welcome(Welcome {
        pv: PROTOCOL_VERSION,
        hid: shared.host_id,
        name: shared.cfg.name.clone(),
        os: shared.cfg.os.clone(),
        osv: shared.cfg.os_version.clone(),
        app: shared.cfg.app_version.clone(),
        sid: Id16::random(),
        caps,
    }))
    .await?;

    *shared.live_sessions.lock().unwrap() += 1;
    shared.emit(HostEvent::DeviceConnected {
        device: dev,
        name: hello.name.clone(),
    });

    let mut live = Live::new(dev);
    live.main_loop(shared, conn, shutdown).await;
    live.finish(shared);

    shared.emit(HostEvent::DeviceDisconnected { device: dev });
    Some(())
}

/// Alur PIN. Mengembalikan `Some(())` hanya bila token sudah diterbitkan dan `PairOk` terkirim.
async fn pair(shared: &Arc<Shared>, conn: &mut Conn, hello: &Hello) -> Option<()> {
    let ttl = shared
        .pairing
        .lock()
        .unwrap()
        .remaining(std::time::Instant::now());
    let Some(ttl) = ttl else {
        conn.send_error(
            ErrorCode::PairingBusy,
            "tidak ada sesi pairing terbuka di host — tekan 'Pair new device' dulu",
        )
        .await;
        return None;
    };
    conn.send(Message::PairRequired(PairRequired {
        ttl_ms: ttl.as_millis() as u32,
    }))
    .await?;

    let deadline = Instant::now() + ttl + STEP_TIMEOUT;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        match conn.next(left).await? {
            Message::PairRequest(req) => {
                let verdict = shared
                    .pairing
                    .lock()
                    .unwrap()
                    .verify(&req.pin, std::time::Instant::now());
                match verdict {
                    crate::pairing::Verify::Ok => break,
                    crate::pairing::Verify::Invalid { attempts_left } => {
                        shared.emit(HostEvent::PairingFailed {
                            reason: "PIN salah".into(),
                            attempts_left,
                        });
                        conn.send(Message::Error(ProtoError {
                            c: ErrorCode::PinInvalid,
                            msg: "PIN salah".into(),
                            attempts_left: Some(attempts_left),
                        }))
                        .await?;
                        if attempts_left == 0 {
                            shared.emit(HostEvent::PairingEnded);
                            return None;
                        }
                    }
                    crate::pairing::Verify::Expired => {
                        conn.send_error(
                            ErrorCode::PinExpired,
                            "PIN kedaluwarsa atau sudah terpakai",
                        )
                        .await;
                        return None;
                    }
                }
            }
            Message::Bye(_) => return None,
            _ => {}
        }
    }

    let mut token = [0u8; 32];
    OsRng.fill_bytes(&mut token);
    let now = now_secs();
    shared
        .registry
        .upsert(&DeviceEntry {
            device: hello.dev,
            token,
            name: hello.name.chars().take(64).collect(),
            platform: hello.plat.clone(),
            paired_at: now,
            last_seen: now,
        })
        .ok()?;
    conn.send(Message::PairOk(PairOk {
        tok: serde_bytes::ByteBuf::from(token.to_vec()),
    }))
    .await?;
    shared.emit(HostEvent::PairingSucceeded {
        device: hello.dev,
        name: hello.name.clone(),
    });
    shared.emit(HostEvent::PairingEnded);
    Some(())
}

// ============================================================ sesi hidup

struct Live {
    dev: Id16,
    mode: Mode,
    sub_metrics: Option<Duration>,
    metric_iv: Option<Interval>,
    media_iv: Option<Interval>,
    clip_iv: Option<Interval>,
    art_queue: VecDeque<Message>,
    // Status input milik sesi ini, untuk dilepas saat sesi berakhir.
    held_keys: HashSet<String>,
    held_buttons: Vec<Button>,
    mods: u8,
    last_seq: Option<u64>,
    deck_active: Option<String>,
    last_clip: Option<u64>,
    lyrics_key: Option<String>,
    lyrics: Option<Arc<Vec<LyricLine>>>,
}

fn hash_str(s: &str) -> u64 {
    let mut h = DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

async fn tick(iv: &mut Option<Interval>) {
    match iv {
        Some(i) => {
            i.tick().await;
        }
        None => std::future::pending::<()>().await,
    }
}

/// Clipboard OS bisa memblokir puluhan ms (jendela lain memegangnya); jangan di thread async.
async fn clip_get(shared: &Arc<Shared>) -> Option<String> {
    let sh = Arc::clone(shared);
    tokio::task::spawn_blocking(move || sh.clipboard.lock().unwrap().get().ok())
        .await
        .ok()
        .flatten()
}

fn new_interval(d: Duration) -> Interval {
    let mut i = interval(d);
    i.set_missed_tick_behavior(MissedTickBehavior::Skip);
    i
}

impl Live {
    fn new(dev: Id16) -> Self {
        Self {
            dev,
            mode: Mode::Idle,
            sub_metrics: None,
            metric_iv: None,
            media_iv: None,
            clip_iv: None,
            art_queue: VecDeque::new(),
            held_keys: HashSet::new(),
            held_buttons: Vec::new(),
            mods: 0,
            last_seq: None,
            deck_active: None,
            last_clip: None,
            lyrics_key: None,
            lyrics: None,
        }
    }

    async fn main_loop(
        &mut self,
        shared: &Arc<Shared>,
        conn: &mut Conn,
        shutdown: &mut watch::Receiver<bool>,
    ) {
        let mut revoked = shared.registry.subscribe_revoked();
        let mut settings = shared.settings.subscribe();
        // Timeout dihitung dari frame terakhir apa pun: client mengirim Ping tiap 2 s.
        let mut deadline = Instant::now() + SESSION_TIMEOUT;

        loop {
            tokio::select! {
                biased;

                _ = shutdown.changed() => {
                    conn.send(Message::Bye(Bye { r: "host berhenti".into() })).await;
                    return;
                }
                r = revoked.recv() => match r {
                    Ok(d) if d == self.dev => {
                        conn.send_error(ErrorCode::TokenRevoked, "perangkat dicabut di host").await;
                        return;
                    }
                    Ok(_) => {}
                    // Ketinggalan pengumuman: pastikan kita belum dicabut.
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                        if !shared.registry.is_known(self.dev) {
                            conn.send_error(ErrorCode::TokenRevoked, "perangkat dicabut di host").await;
                            return;
                        }
                    }
                    Err(_) => return,
                },
                frame = conn.frames.recv() => {
                    let Some(frame) = frame else { return };
                    match conn.noise.decrypt(&frame) {
                        Ok(Decoded::Known(msg)) => {
                            deadline = Instant::now() + SESSION_TIMEOUT;
                            if !self.handle(shared, conn, msg).await { return }
                        }
                        Ok(Decoded::Unknown(t)) => {
                            deadline = Instant::now() + SESSION_TIMEOUT;
                            tracing::debug!("pesan tak dikenal diabaikan: {t}");
                        }
                        // Dekripsi/parse gagal: tutup tanpa penjelasan.
                        Err(_) => return,
                    }
                }
                _ = tokio::time::sleep_until(deadline) => {
                    tracing::debug!("sesi timeout tanpa Ping");
                    return;
                }
                changed = settings.changed() => {
                    if changed.is_err() { return }
                    let s = *settings.borrow_and_update();
                    if conn.send(Message::InputSettings(s)).await.is_none() { return }
                }
                _ = tick(&mut self.metric_iv) => {
                    if !self.send_metrics(shared, conn).await { return }
                }
                _ = tick(&mut self.media_iv) => {
                    if !self.send_now_playing(shared, conn).await { return }
                }
                _ = tick(&mut self.clip_iv) => {
                    if !self.poll_clipboard(shared, conn).await { return }
                }
                // Prioritas terendah: chunk artwork hanya lewat saat tidak ada yang lain siap,
                // jadi gambar besar tidak menahan input.
                _ = std::future::ready(()), if !self.art_queue.is_empty() => {
                    if let Some(m) = self.art_queue.pop_front() {
                        if conn.send(m).await.is_none() { return }
                    }
                }
            }
        }
    }

    /// Kembalikan `false` untuk menutup sesi.
    async fn handle(&mut self, shared: &Arc<Shared>, conn: &mut Conn, msg: Message) -> bool {
        match msg {
            Message::Ping(p) => {
                let th = shared.start.elapsed().as_micros() as u64;
                conn.send(Message::Pong(Pong {
                    n: p.n,
                    tc: p.tc,
                    th,
                }))
                .await
                .is_some()
            }
            Message::Bye(_) => false,
            Message::SetMode(s) => self.set_mode(shared, conn, s.m).await,
            Message::SubscribeTelemetry(s) => {
                self.sub_metrics = s
                    .kinds
                    .contains(&TelemetryKind::Metrics)
                    .then(|| Duration::from_millis(s.iv.max(MIN_TELEMETRY_INTERVAL_MS) as u64));
                self.sync_timers();
                true
            }
            Message::InputBatch(b) => {
                if b.ev.len() > MAX_INPUT_EVENTS {
                    tracing::debug!("batch terlalu besar dibuang: {}", b.ev.len());
                    return true;
                }
                if let Some(prev) = self.last_seq {
                    if b.s > prev + 1 {
                        tracing::debug!("batch hilang: {} → {}", prev, b.s);
                    }
                }
                self.last_seq = Some(b.s);
                let settings = *shared.settings.borrow();
                for ev in b.ev {
                    self.apply_input(shared, ev, settings);
                }
                true
            }
            Message::SetInputSettings(s) => {
                shared.settings.send_modify(|cur| {
                    if let Some(v) = s.sens {
                        if v.is_finite() {
                            cur.sens = v.clamp(0.1, 5.0);
                        }
                    }
                    if let Some(n) = s.natural {
                        cur.natural = n;
                    }
                });
                true
            }
            Message::ClipboardPush(c) => {
                if c.s.len() <= MAX_CLIPBOARD {
                    self.last_clip = Some(hash_str(&c.s));
                    let sh = Arc::clone(shared);
                    let text = c.s;
                    let _ = tokio::task::spawn_blocking(move || {
                        sh.clipboard.lock().unwrap().set(&text)
                    })
                    .await;
                }
                true
            }
            Message::SelectProfile(s) => self.select_profile(shared, conn, s.id).await,
            Message::DeckPress(p) => self.deck_press(shared, conn, p).await,
            Message::DeckRelease(_) => true,
            Message::MediaCommand(c) => self.media_op(shared, conn, move |m| m.command(c.c)).await,
            Message::MediaSeek(s) => {
                if !shared.caps.media_seek {
                    return conn
                        .send_error(ErrorCode::Unsupported, "seek tidak didukung")
                        .await
                        .is_some();
                }
                self.media_op(shared, conn, move |m| m.seek(s.ms)).await
            }
            Message::MediaVolume(v) => {
                if !shared.caps.media_volume {
                    return conn
                        .send_error(ErrorCode::Unsupported, "volume tidak didukung")
                        .await
                        .is_some();
                }
                let level = v.v.clamp(0.0, 1.0);
                self.media_op(shared, conn, move |m| m.volume(level)).await
            }
            Message::GetArtwork(g) => self.queue_artwork(shared, conn, g.id).await,
            Message::GetLyrics(g) => self.send_lyrics(shared, conn, g.id).await,
            // Pesan arah host→client atau kontrol yang sudah lewat: abaikan.
            _ => true,
        }
    }

    // ------------------------------------------------------------ mode

    async fn set_mode(&mut self, shared: &Arc<Shared>, conn: &mut Conn, m: Mode) -> bool {
        let allowed = m == Mode::Idle || shared.caps.modes.contains(&m);
        if !allowed {
            return conn
                .send(Message::ModeState(ModeState {
                    m,
                    ok: false,
                    msg: Some("mode tidak tersedia di host ini".into()),
                }))
                .await
                .is_some();
        }
        self.mode = m;
        self.art_queue.clear();
        self.sync_timers();
        shared.emit(HostEvent::ModeChanged {
            device: self.dev,
            mode: m,
        });
        if conn
            .send(Message::ModeState(ModeState {
                m,
                ok: true,
                msg: None,
            }))
            .await
            .is_none()
        {
            return false;
        }
        match m {
            Mode::Trackpad => {
                self.last_clip = clip_get(shared).await.map(|s| hash_str(&s));
                let s = *shared.settings.borrow();
                conn.send(Message::InputSettings(s)).await.is_some()
            }
            Mode::Deck => self.send_deck(shared, conn).await,
            _ => true,
        }
    }

    /// Telemetri hanya berjalan untuk mode yang aktif; pindah mode = timer lama berhenti.
    fn sync_timers(&mut self) {
        self.metric_iv = match (self.mode, self.sub_metrics) {
            (Mode::Monitor, Some(d)) => Some(new_interval(d)),
            _ => None,
        };
        self.media_iv = (self.mode == Mode::Music).then(|| new_interval(Duration::from_secs(1)));
        self.clip_iv = (self.mode == Mode::Trackpad).then(|| new_interval(Duration::from_secs(1)));
    }

    // ---------------------------------------------------------- input

    fn apply_input(&mut self, shared: &Arc<Shared>, ev: InputEvent, s: InputSettings) {
        let ok = |v: f32| v.is_finite() && v.abs() <= MAX_DELTA;
        let mut inp = shared.input.lock().unwrap();
        let res = match ev {
            InputEvent::PointerMove { dx, dy } if ok(dx) && ok(dy) => {
                inp.move_pointer(dx * s.sens, dy * s.sens)
            }
            InputEvent::PointerAbs { x, y } if x.is_finite() && y.is_finite() => {
                inp.move_pointer_abs(x, y)
            }
            InputEvent::PointerButton { b, d } => {
                if d {
                    if !self.held_buttons.contains(&b) {
                        self.held_buttons.push(b);
                    }
                } else {
                    self.held_buttons.retain(|x| *x != b);
                }
                inp.button(b, d)
            }
            InputEvent::Scroll { dx, dy, ph, mom } if ok(dx) && ok(dy) => {
                // Natural: konten mengikuti jari (jari turun → konten turun).
                let sign = if s.natural { 1.0 } else { -1.0 };
                inp.scroll(dx * sign, dy * sign, ph, mom)
            }
            InputEvent::Gesture { g, f } => {
                if inp.supported_gestures().contains(&g.as_str()) {
                    inp.gesture(&g, f)
                } else {
                    Ok(())
                }
            }
            InputEvent::Key { k, d } => {
                let r = inp.key(&k, d);
                if r.is_ok() {
                    if d {
                        self.held_keys.insert(k);
                    } else {
                        self.held_keys.remove(&k);
                    }
                }
                r
            }
            InputEvent::Text { s: text } => {
                let clipped: String = text.chars().take(MAX_TEXT_CHARS).collect();
                inp.text(&clipped)
            }
            InputEvent::Modifiers { m } => {
                let mut r = Ok(());
                for (bit, name) in [
                    (modifiers::SHIFT, "shift"),
                    (modifiers::CTRL, "ctrl"),
                    (modifiers::ALT, "alt"),
                    (modifiers::META, "meta"),
                ] {
                    let was = self.mods & bit != 0;
                    let now = m & bit != 0;
                    if was != now {
                        r = inp.key(name, now);
                        if now {
                            self.held_keys.insert(name.to_owned());
                        } else {
                            self.held_keys.remove(name);
                        }
                    }
                }
                self.mods = m;
                r
            }
            // NaN/berlebihan: dibuang.
            _ => Ok(()),
        };
        if let Err(e) = res {
            tracing::debug!("input gagal: {e}");
        }
    }

    /// Lepas semua yang masih tertahan oleh sesi ini. Bila ini sesi terakhir, `release_all`
    /// menyapu bersih; bila masih ada perangkat lain, hanya milik sendiri yang dilepas supaya
    /// modifier perangkat lain tidak ikut terlepas.
    fn finish(&mut self, shared: &Arc<Shared>) {
        let last = {
            let mut n = shared.live_sessions.lock().unwrap();
            *n = n.saturating_sub(1);
            *n == 0
        };
        let mut inp = shared.input.lock().unwrap();
        if last {
            let _ = inp.release_all();
        } else {
            for k in self.held_keys.drain() {
                let _ = inp.key(&k, false);
            }
            for b in self.held_buttons.drain(..) {
                let _ = inp.button(b, false);
            }
        }
    }

    async fn poll_clipboard(&mut self, shared: &Arc<Shared>, conn: &mut Conn) -> bool {
        let Some(text) = clip_get(shared).await else {
            return true;
        };
        let h = hash_str(&text);
        if self.last_clip == Some(h) {
            return true;
        }
        self.last_clip = Some(h);
        if text.is_empty() || text.len() > MAX_CLIPBOARD {
            return true;
        }
        conn.send(Message::ClipboardUpdate(ClipboardUpdate { s: text }))
            .await
            .is_some()
    }

    // ------------------------------------------------------- telemetri

    async fn send_metrics(&mut self, shared: &Arc<Shared>, conn: &mut Conn) -> bool {
        let sh = Arc::clone(shared);
        let sample = tokio::task::spawn_blocking(move || sh.metrics.lock().unwrap().sample()).await;
        match sample {
            Ok(Ok(m)) => conn.send(Message::Metrics(m)).await.is_some(),
            _ => true,
        }
    }

    // ----------------------------------------------------------- musik

    async fn send_now_playing(&mut self, shared: &Arc<Shared>, conn: &mut Conn) -> bool {
        let sh = Arc::clone(shared);
        let np = tokio::task::spawn_blocking(move || sh.media.lock().unwrap().now_playing()).await;
        let Ok(Ok(Some(mut np))) = np else {
            return true;
        };
        np.lyr = self.lyrics_for(shared, &np.artist, &np.title).is_some();
        conn.send(Message::NowPlaying(np)).await.is_some()
    }

    fn lyrics_for(
        &mut self,
        shared: &Arc<Shared>,
        artist: &str,
        title: &str,
    ) -> Option<Arc<Vec<LyricLine>>> {
        let dir = shared.lyrics_dir.lock().unwrap().clone()?;
        let key = format!("{artist}\u{0}{title}");
        if self.lyrics_key.as_deref() != Some(key.as_str()) {
            self.lyrics = load_lyrics(&dir, artist, title).map(Arc::new);
            self.lyrics_key = Some(key);
        }
        self.lyrics.clone()
    }

    async fn media_op<F>(&mut self, shared: &Arc<Shared>, conn: &mut Conn, f: F) -> bool
    where
        F: FnOnce(&mut dyn tab_media::MediaSource) -> Result<(), tab_media::MediaError>
            + Send
            + 'static,
    {
        let sh = Arc::clone(shared);
        let res = tokio::task::spawn_blocking(move || {
            let mut m = sh.media.lock().unwrap();
            f(m.as_mut())
        })
        .await;
        match res {
            Ok(Ok(())) => {
                // Segarkan tampilan secepatnya alih-alih menunggu tick berikutnya.
                if let Some(iv) = self.media_iv.as_mut() {
                    iv.reset_immediately();
                }
                true
            }
            Ok(Err(e)) => conn
                .send_error(ErrorCode::Unsupported, &e.to_string())
                .await
                .is_some(),
            Err(_) => conn
                .send_error(ErrorCode::Internal, "media gagal")
                .await
                .is_some(),
        }
    }

    async fn queue_artwork(&mut self, shared: &Arc<Shared>, conn: &mut Conn, id: String) -> bool {
        let sh = Arc::clone(shared);
        let id2 = id.clone();
        let bytes = tokio::task::spawn_blocking(move || sh.media.lock().unwrap().artwork(&id2))
            .await
            .ok()
            .and_then(|r| r.ok())
            .flatten();
        let Some(bytes) = bytes else {
            return conn
                .send_error(ErrorCode::Unsupported, "artwork tidak tersedia")
                .await
                .is_some();
        };
        let n = bytes.chunks(ART_CHUNK).count().min(u16::MAX as usize) as u16;
        for (i, part) in bytes.chunks(ART_CHUNK).take(n as usize).enumerate() {
            self.art_queue
                .push_back(Message::ArtworkChunk(ArtworkChunk {
                    id: id.clone(),
                    i: i as u16,
                    n,
                    b: serde_bytes::ByteBuf::from(part.to_vec()),
                }));
        }
        true
    }

    async fn send_lyrics(&mut self, shared: &Arc<Shared>, conn: &mut Conn, id: String) -> bool {
        // `id` hanya dipantulkan balik; host selalu menjawab untuk lagu yang sedang diputar.
        let sh = Arc::clone(shared);
        let np = tokio::task::spawn_blocking(move || sh.media.lock().unwrap().now_playing())
            .await
            .ok()
            .and_then(|r| r.ok())
            .flatten();
        let lines = np.and_then(|n| self.lyrics_for(shared, &n.artist, &n.title));
        match lines {
            Some(lines) => {
                let doc = Message::LyricsDoc(LyricsDoc {
                    id,
                    lines: lines.as_ref().clone(),
                });
                match conn.send(doc).await {
                    Some(()) => true,
                    // Gagal menyandi biasanya berarti dokumen melebihi satu frame.
                    None => conn
                        .send_error(ErrorCode::Internal, "lirik terlalu besar")
                        .await
                        .is_some(),
                }
            }
            None => conn
                .send_error(ErrorCode::Unsupported, "lirik tidak ditemukan")
                .await
                .is_some(),
        }
    }

    // ------------------------------------------------------------ deck

    async fn send_deck(&mut self, shared: &Arc<Shared>, conn: &mut Conn) -> bool {
        let list = shared.deck.lock().unwrap().list().unwrap_or_default();
        if list.is_empty() {
            return true;
        }
        let active = self
            .deck_active
            .clone()
            .filter(|a| list.iter().any(|p| p.id == *a))
            .unwrap_or_else(|| list[0].id.clone());
        self.deck_active = Some(active.clone());
        let refs = list
            .iter()
            .map(|p| ProfileRef {
                id: p.id.clone(),
                name: p.name.clone(),
            })
            .collect();
        if conn
            .send(Message::DeckProfiles(DeckProfiles {
                list: refs,
                active: active.clone(),
            }))
            .await
            .is_none()
        {
            return false;
        }
        match list.iter().find(|p| p.id == active) {
            Some(p) => conn.send(Message::DeckProfile(to_wire(p))).await.is_some(),
            None => true,
        }
    }

    async fn select_profile(&mut self, shared: &Arc<Shared>, conn: &mut Conn, id: String) -> bool {
        let profile = shared.deck.lock().unwrap().load(&id).ok().flatten();
        match profile {
            Some(p) => {
                self.deck_active = Some(p.id.clone());
                self.send_deck(shared, conn).await
            }
            None => conn
                .send_error(ErrorCode::Unsupported, "profil tidak ada")
                .await
                .is_some(),
        }
    }

    async fn deck_press(&mut self, shared: &Arc<Shared>, conn: &mut Conn, p: DeckPress) -> bool {
        let action = self.deck_active.as_deref().and_then(|id| {
            let profile = shared.deck.lock().unwrap().load(id).ok().flatten()?;
            // Hanya aksi yang benar-benar dipasang pada tombol di profil aktif yang boleh
            // dipicu: perangkat tidak bisa menembak `aid` sembarang.
            let wired = profile
                .buttons
                .iter()
                .any(|b| b.index == p.i && b.action_id == p.aid);
            wired.then(|| profile.action(&p.aid).cloned()).flatten()
        });
        let Some(action) = action else {
            return conn
                .send(Message::DeckFeedback(DeckFeedback {
                    aid: p.aid,
                    ok: false,
                    msg: Some("aksi tidak dikenal".into()),
                }))
                .await
                .is_some();
        };
        let sh = Arc::clone(shared);
        let res = tokio::task::spawn_blocking(move || sh.runner.lock().unwrap().run(&action)).await;
        let (ok, msg) = match res {
            Ok(Ok(())) => (true, None),
            Ok(Err(e)) => (false, Some(e.to_string())),
            Err(_) => (false, Some("aksi gagal".into())),
        };
        conn.send(Message::DeckFeedback(DeckFeedback {
            aid: p.aid,
            ok,
            msg,
        }))
        .await
        .is_some()
    }
}
