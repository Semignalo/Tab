# Tab Wire Protocol — v1

Sumber kebenaran untuk kedua sisi implementasi (host Rust, client Kotlin).
Setiap perubahan pada dokumen ini harus diikuti perubahan di `host/src-tauri/src/transport/`
dan `client-android/core/model/`, plus fixture di `protocol/fixtures/`.

Protokol ini milik project Tab sendiri dan **tidak** dirancang kompatibel dengan produk lain.

- `PROTOCOL_VERSION = 1`
- Port discovery: **UDP 4179**
- Port sesi: **TCP 4180** (default; nilai sebenarnya selalu diambil dari balasan discovery)
- Byte order: big-endian untuk header, CBOR untuk payload
- Semua string UTF-8

---

## 1. Model koneksi

```
                    UDP 4179 broadcast
client  ─── DiscoverRequest ──────────────────►  host (semua host di subnet)
client  ◄── DiscoverResponse ─────────────────   host (unicast ke pengirim)

                    TCP 4180
client  ─── Noise handshake ─────────────────►  host
        ◄── Noise handshake ─────────────────
client  ─── Hello ──────────────────────────►
        ◄── Welcome | PairRequired ──────────
                  ... sesi ...
```

Satu perangkat = satu koneksi TCP. Host melayani banyak koneksi serentak; setiap sesi punya
mode dan langganan telemetri sendiri.

---

## 2. Discovery (UDP 4179)

Datagram tidak terenkripsi dan **tidak boleh** memuat data sensitif. Maksimum 1200 byte
agar tidak terfragmentasi.

```
+--------+--------+--------+-------------------+
| "TABD" | ver u8 | kind u8| CBOR payload      |
+--------+--------+--------+-------------------+
   4 B      1 B      1 B      sisa datagram
```

`ver` = 1. `kind`: `0x01` = DiscoverRequest, `0x02` = DiscoverResponse.

Datagram dengan magic atau `ver` tidak dikenal **dibuang tanpa balasan**.

### DiscoverRequest (client → broadcast)

| Field | Tipe | Keterangan |
|---|---|---|
| `dev` | bytes(16) | `device_id`, UUIDv4 acak yang dibuat sekali saat install |
| `pv` | uint | `PROTOCOL_VERSION` yang didukung client |

Client mengirim ke alamat broadcast subnet setiap antarmuka aktif (bukan hanya
`255.255.255.255`, karena Android sering memblokirnya), 3 kali dengan jeda 300 ms.

### DiscoverResponse (host → unicast)

| Field | Tipe | Keterangan |
|---|---|---|
| `hid` | bytes(16) | `host_id`, stabil selama instalasi host |
| `name` | text | Nama mesin untuk ditampilkan, ≤ 64 karakter |
| `os` | text | `"macos"` \| `"windows"` |
| `osv` | text | Versi OS, mis. `"15.6"` |
| `app` | text | Versi host, mis. `"0.1.0"` |
| `pmin` | uint | Versi protokol minimum yang diterima |
| `pmax` | uint | Versi protokol maksimum yang diterima |
| `port` | uint | Port TCP sesi |
| `fp` | bytes(32) | BLAKE2s dari static public key X25519 host |
| `known` | bool | `true` bila `dev` sudah terpasang token di host ini |

Aturan host:
- Balas hanya bila `pv` berada dalam `[pmin, pmax]`.
- Rate limit: maksimum 1 balasan per alamat sumber per 200 ms.
- `known` memungkinkan client menandai "sudah dipasangkan" di daftar tanpa membocorkan apa pun.

Client **menyimpan cache** `host_id → (ip, port)` dan mencoba koneksi langsung lebih dulu;
discovery hanya dijalankan bila koneksi langsung gagal. Inilah sebabnya user tidak pernah
mengetik alamat IP.

---

## 3. Framing sesi (TCP)

```
+------------------+---------------------------+
| length u32 BE    | body                      |
+------------------+---------------------------+
```

- `length` = panjang `body` dalam byte. `length == 0` tidak sah.
- Batas: `MAX_FRAME = 65_535` byte (sesuai batas satu pesan Noise). Payload yang lebih besar
  **wajib** dipecah di lapisan aplikasi (lihat artwork, §7.3).
- Socket diset `TCP_NODELAY`. Host juga memasang `SO_KEEPALIVE`.
- Sebelum handshake selesai: `body` = pesan handshake Noise mentah.
- Setelah handshake: `body` = ciphertext Noise dari satu `Envelope` CBOR.

Frame yang melewati `MAX_FRAME`, gagal didekripsi, atau tidak valid sebagai CBOR → koneksi
**ditutup segera** tanpa pesan error (kegagalan dekripsi tidak boleh membocorkan informasi).

### Head-of-line blocking

Satu koneksi TCP dipakai untuk semua pesan, jadi payload besar bisa menahan event input.
Karena itu: setiap frame dibatasi ≤ 8 KiB untuk pesan telemetri/artwork, dan writer host
memakai dua queue dengan prioritas — `input`/`control` selalu mendahului `telemetry`.

---

## 4. Kriptografi & pairing

Host punya satu keypair statis X25519 jangka panjang, dibuat saat pertama dijalankan dan
disimpan di keyring OS. Fingerprint-nya (`fp`) disiarkan lewat discovery dan **disematkan
(pinned)** oleh client saat pairing berhasil.

| Fase | Pola Noise |
|---|---|
| Pairing (perangkat baru) | `Noise_NK_25519_ChaChaPoly_BLAKE2s` |
| Resume (sudah punya token) | `Noise_NKpsk2_25519_ChaChaPoly_BLAKE2s`, `psk` = token 32 byte |

`NK` berarti client sudah mengetahui static key host di muka, jadi host terautentikasi
terhadap kunci yang dipin. Pada koneksi pertama kunci itu baru berasal dari discovery yang
bisa dipalsukan — **PIN-lah yang menutup celah itu**, karena PIN hanya tampil di layar host.

`NKpsk2` mengikat token ke handshake, sehingga token tidak pernah dikirim sebagai payload dan
tidak bisa diputar ulang ke host lain.

### PIN

- 6 digit desimal dari CSPRNG, ditampilkan di UI host saat user menekan "Pair new device".
- Berlaku **120 detik**, sekali pakai.
- Maksimum **5** percobaan salah, lalu PIN dibatalkan dan user harus membuat yang baru.
- Dibandingkan dengan perbandingan waktu-konstan.
- Tidak pernah dicatat ke log.

### Token

- 32 byte acak dari CSPRNG per perangkat.
- Host menyimpan `device_id → (token, device_name, platform, paired_at, last_seen)`.
- Client menyimpan `host_id → (token, host_static_pubkey, ip, port)`.
- Penyimpanan: host lewat crate `keyring` (Keychain / Windows Credential Manager),
  client lewat `EncryptedSharedPreferences` (Android Keystore).
- User dapat mencabut token per perangkat dari UI host; koneksi aktif langsung ditutup.

### Alur pairing

```
client                                        host
  │── Noise_NK handshake ──────────────────────►│
  │── Hello{pv, device} ───────────────────────►│
  │◄── PairRequired{ttl_ms} ────────────────────│   (device_id belum dikenal)
  │      (user membaca PIN di layar host)        │
  │── PairRequest{pin} ────────────────────────►│
  │◄── PairOk{token, host} ─────────────────────│   atau Error{PinInvalid, attempts_left}
  │◄── Welcome{host, capabilities, session} ────│
```

Bila `device_id` sudah dikenal namun handshake `NKpsk2` gagal (token dicabut atau basi),
host menutup koneksi; client menghapus token lokal lalu jatuh kembali ke alur pairing.

---

## 5. Envelope

Setiap pesan adalah satu map CBOR dengan diskriminator internal `"t"`.

```
{ "t": "<NamaPesan>", ...field pesan }
```

Rust: `#[serde(tag = "t")]` pada enum `Message`.
Kotlin: `Cbor { classDiscriminator = "t" }` dengan `sealed interface Message`.

Nama field disingkat dan **selalu** dituliskan eksplisit (`#[serde(rename)]` /
`@SerialName`) agar rename di kode tidak memecah wire format.

Pesan yang `t`-nya tidak dikenal **diabaikan** (bukan error) — ini yang membuat versi
berbeda tetap bisa berjalan bersama selama versi mayor sama.

---

## 6. Control

### Hello (client → host)
| Field | Tipe |
|---|---|
| `pv` | uint — versi protokol |
| `dev` | bytes(16) — device_id |
| `name` | text — nama perangkat |
| `plat` | text — `"android"` \| `"ios"` \| `"probe"` |
| `platv` | text — versi OS |
| `app` | text — versi client |
| `scr` | `{w, h, dpi}` — ukuran layar logis |

### Welcome (host → client)
| Field | Tipe |
|---|---|
| `pv` | uint — versi yang dipakai (hasil negosiasi) |
| `hid` | bytes(16) |
| `name` | text |
| `os`, `osv`, `app` | text |
| `sid` | bytes(16) — session id, baru setiap koneksi |
| `caps` | Capabilities |

### Capabilities
| Field | Tipe | Catatan |
|---|---|---|
| `modes` | array text | subset dari `trackpad`, `deck`, `monitor`, `music`, `clock` |
| `clipboard` | bool | |
| `gestures` | array text | mis. `["spaces_left","spaces_right","mission_control"]`, kosong di Windows v1 |
| `now_playing` | bool | false bila jalur media host tidak tersedia |
| `media_seek`, `media_volume` | bool | tidak semua backend mendukung |
| `obs` | bool | true hanya bila obs-websocket tersambung |
| `temps` | bool | suhu CPU |
| `gpu` | bool | metrik GPU |
| `second_screen`, `gamepad` | bool | selalu `false` di v1 |
| `max_frame` | uint | |

Client **wajib** menyembunyikan kontrol untuk kapabilitas yang `false`. Lebih baik fitur
tidak tampil daripada tampil lalu gagal atau menampilkan angka palsu.

### Ping / Pong
`Ping{n: uint, tc: uint}` → `Pong{n, tc, th: uint}` — `tc` jam monotonik client (µs),
`th` jam monotonik host (µs). RTT = `now_client - tc`. Client mengirim Ping setiap **2 s**.

### SetMode / ModeState
`SetMode{m: text}` → `ModeState{m, ok: bool, msg?: text}`.
Host hanya mengirim telemetri untuk mode yang aktif pada sesi itu.

### Bye / Error
`Bye{r: text}` — penutupan sopan dari kedua sisi.
`Error{c: text, msg: text, extra?: map}` dengan `c` salah satu:

| Kode | Arti |
|---|---|
| `ProtocolUnsupported` | versi di luar `[pmin, pmax]` |
| `PairRequired` | perlu pairing |
| `PinInvalid` | PIN salah (`extra.attempts_left`) |
| `PinExpired` | PIN kedaluwarsa atau sudah terpakai |
| `PairingBusy` | tidak ada sesi pairing yang terbuka di host |
| `TokenRevoked` | token dicabut user |
| `RateLimited` | terlalu banyak permintaan |
| `Unsupported` | pesan sah tetapi kapabilitasnya mati |
| `Internal` | kegagalan di sisi host |

---

## 7. Mode

### 7.1 Trackpad & Input (client → host)

`InputBatch{s: uint, ev: array<InputEvent>}` — `s` naik monoton per sesi agar drop
terdeteksi. Maksimum **64** event per batch; client mengirim satu batch per frame tampilan
(~8 ms pada 120 Hz), bukan satu frame per sentuhan.

Klasifikasi gesture (tap vs scroll dua jari vs drag) dilakukan **di client**, karena di
sanalah data sentuhan mentah berada. Host menerima niat yang sudah jelas, bukan koordinat
mentah — ini yang mencegah scroll dua jari salah terbaca sebagai klik.

| InputEvent `t` | Field |
|---|---|
| `PointerMove` | `dx`, `dy` (float, piksel logis; sensitivitas diterapkan host) |
| `PointerAbs` | `x`, `y` (float 0..1 relatif area trackpad) — dipakai mode absolut |
| `PointerButton` | `b`: `l`\|`r`\|`m`; `d`: bool (down) |
| `Scroll` | `dx`, `dy` (float piksel), `ph`: `b`\|`u`\|`e` (begin/update/end), `mom`: bool |
| `Gesture` | `g`: text (lihat `caps.gestures`), `f`: uint jumlah jari |
| `Key` | `k`: text (nama tombol kanonik, §8), `d`: bool |
| `Text` | `s`: text — hasil IME, dikirim utuh bukan per tombol |
| `Modifiers` | `m`: uint bitflag — `1` shift, `2` ctrl, `4` alt, `8` meta |

Arah scroll (Natural/Inverted) dan sensitivitas adalah **setelan host**, dikirim ke client
lewat `InputSettings{sens: float, natural: bool}` supaya konsisten di semua perangkat, dan
dapat diubah client lewat `SetInputSettings{...}`.

Clipboard: `ClipboardPush{s: text}` (client → host) dan `ClipboardUpdate{s: text}`
(host → client, hanya saat mode trackpad aktif). Batas 64 KiB, dipecah bila perlu; konten
clipboard tidak pernah ditulis ke disk maupun log.

### 7.2 Deck

Host → client:
- `DeckProfiles{list: array<{id, name}>, active: id}`
- `DeckProfile{id, name, cols: uint, rows: uint, btns: array<DeckButton>}`
  - `DeckButton{i: uint (indeks sel), lbl: text, ic?: text (nama ikon), col?: text (hex), aid: text}`
- `DeckFeedback{aid, ok: bool, msg?: text}`

Client → host:
- `SelectProfile{id}`
- `DeckPress{aid, i}` / `DeckRelease{aid, i}`

Definisi aksi (`Hotkey`, `LaunchApp`, `OpenPath`, `OpenUrl`, `MediaKey`, `ObsScene`, `Multi`)
**tidak pernah dikirim ke client** — client hanya tahu `aid`. Ini menjaga agar perangkat
yang dipasangkan tidak bisa menyusun perintah arbitrer; ia hanya bisa memicu aksi yang
sudah dibuat user di host.

### 7.3 System monitor

`SubscribeTelemetry{kinds: array<text>, iv: uint (ms, minimum 250)}` →
host mengirim `Metrics` berkala:

| Field | Tipe |
|---|---|
| `cpu` | array float — per core, 0..100 |
| `cpua` | float — rata-rata |
| `mu`, `mt` | uint — memori terpakai / total (byte) |
| `su`, `st` | uint — swap |
| `disks` | array `{n: text, u: uint, t: uint}` |
| `rx`, `tx` | uint — byte/detik |
| `pwr` | `{ac: bool, pct?: uint}` |
| `tcpu` | float? — suhu CPU, **hanya bila `caps.temps`** |
| `gpu` | `{u?: float, t?: float, mu?: uint, mt?: uint}`? — hanya bila `caps.gpu` |

Field suhu/GPU dihilangkan sepenuhnya bila tidak tersedia. Tidak ada nilai placeholder.

### 7.4 Music & Lyrics

Host → client:
- `NowPlaying{title, artist, album, dur: uint ms, pos: uint ms, play: bool, art?: text (artwork_id), lyr: bool}`
- `ArtworkChunk{id: text, i: uint, n: uint, b: bytes}` — maksimum 8 KiB per chunk
- `LyricsDoc{id: text, lines: array<{t: uint ms, s: text}>}`

Client → host:
- `MediaCommand{c: text}` — `play`, `pause`, `toggle`, `next`, `prev`
- `MediaSeek{ms: uint}` (perlu `caps.media_seek`)
- `MediaVolume{v: float 0..1}` (perlu `caps.media_volume`)
- `GetArtwork{id}`, `GetLyrics{id}`

`pos` dikirim ulang paling sering 1 Hz; client melakukan interpolasi lokal agar timeline dan
baris lirik bergerak halus tanpa menambah trafik.

Lirik berasal dari file `.lrc` milik user di folder yang ia tentukan sendiri di host.
Host **tidak** mengambil lirik dari layanan pihak ketiga.

### 7.5 Desk clock

Sepenuhnya di client (jam, orientasi landscape, keep-screen-on, peredupan otomatis).
Host tidak mengirim apa pun; sesi hanya menjaga heartbeat agar reconnect tetap instan.

---

## 8. Nama tombol kanonik

Nama tombol dikirim sebagai string agar tidak bergantung pada layout fisik:
`a`..`z`, `0`..`9`, `f1`..`f24`, `esc`, `tab`, `capslock`, `space`, `enter`, `backspace`,
`delete`, `insert`, `home`, `end`, `pageup`, `pagedown`, `up`, `down`, `left`, `right`,
`shift`, `ctrl`, `alt`, `meta`, `minus`, `equal`, `bracketleft`, `bracketright`,
`backslash`, `semicolon`, `quote`, `comma`, `period`, `slash`, `grave`,
`media_play`, `media_next`, `media_prev`, `volume_up`, `volume_down`, `mute`.

`meta` dipetakan ke Command di macOS dan Windows key di Windows.

---

## 9. Resiliensi

| Parameter | Nilai |
|---|---|
| Interval Ping | 2 s |
| Timeout sesi | 6 s tanpa Pong |
| Backoff reconnect | 250 ms → 500 ms → 1 s → 2 s → 4 s, batas 5 s, jitter ±20% |
| Urutan reconnect | koneksi langsung ke IP cache → discovery → koneksi |

Saat reconnect berhasil, client **memulihkan sendiri** mode terakhir dan langganan
telemetri-nya. Dari sisi user, gangguan Wi-Fi sesaat tidak menyebabkan keluar dari mode dan
tidak pernah meminta pairing ulang.

Host melepas sumber daya per sesi (sampler telemetri, tombol yang tertahan) saat koneksi
tertutup. **Semua tombol dan tombol mouse yang masih tertekan wajib dilepas** saat sesi
berakhir, agar modifier tidak "nyangkut" setelah koneksi putus.

---

## 10. Batasan

| Nama | Nilai |
|---|---|
| `MAX_FRAME` | 65_535 B |
| Frame telemetri/artwork | ≤ 8 KiB |
| Event per `InputBatch` | 64 |
| Clipboard | 64 KiB |
| Datagram discovery | 1200 B |
| Sesi serentak per host | 8 |
