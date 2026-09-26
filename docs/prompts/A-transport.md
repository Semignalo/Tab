# Track A — Transport & Pairing Host

Branch: `track/a-transport`

Kamu membangun jantung host: discovery, listener TCP, manajer sesi, dan state machine
pairing. Track lain bergantung pada seam yang sudah beku, jadi kamu **hanya** mengisi
`crates/tab-host` — tidak menyentuh crate lain.

Baca `docs/prompts/_COMMON.md` dan `protocol/PROTOCOL.md` (§2, §3, §4, §9) lebih dulu.

## File yang kamu miliki

`crates/tab-host/src/{lib,discovery,listener,session,pairing,store,registry}.rs` dan
`crates/tab-host/tests/**`.

## Yang harus jalan

1. **Responder discovery UDP 4179** — balas hanya bila `should_answer()`, rate limit 1 balasan
   per alamat sumber per 200 ms, isi `known` sesuai ada/tidaknya token untuk `device_id` itu.
   Datagram asing dibuang tanpa balasan (`DiscoveryError::Foreign` sudah membedakannya).
2. **Listener TCP** — `TCP_NODELAY` + `SO_KEEPALIVE`, maksimum `MAX_SESSIONS` (8) sesi
   serentak, koneksi ke-9 ditolak dengan sopan.
3. **Handshake** — coba `Noise_NKpsk2` bila `device_id` punya token, jatuh ke `Noise_NK` bila
   belum. Kegagalan dekripsi → **tutup koneksi tanpa pesan error** (jangan bocorkan informasi).
4. **State machine pairing** — PIN 6 digit dari CSPRNG, TTL 120 s, sekali pakai, maksimum 5
   percobaan salah lalu PIN dibatalkan. Banding PIN harus **waktu-konstan**. PIN tidak pernah
   masuk log.
5. **Sesi** — heartbeat `Ping`/`Pong`, timeout 6 s, `SetMode`, langganan telemetri per sesi
   (berhenti saat mode tidak aktif), penerusan `InputBatch`/`DeckPress` ke dependensi.
6. **Pembersihan** — saat sesi tutup, panggil `PlatformInput::release_all()`. Ini bukan detail:
   modifier yang nyangkut setelah Wi-Fi mati adalah bug yang paling terasa user.
7. **`TokenStore` asli** lewat crate `keyring` (Keychain / Windows Credential Manager), dengan
   `MemoryTokenStore` tetap tersedia untuk test.

## Cara kamu menguji tanpa track lain

Pakai stub dari P0 (`NoopInput`, `FakeMetrics`, `FakeMedia`, `MemoryTokenStore`) dan jalankan
client-nya **di dalam test yang sama** memakai `tab_protocol::noise::Handshake` langsung —
tidak perlu `tab-probe` maupun perangkat Android.

Minimal test ini harus ada:

- pairing sukses → token diterbitkan → `Welcome` berisi `Capabilities` yang jujur
- PIN salah 5× → PIN dibatalkan, percobaan ke-6 dapat `PinExpired`, bukan `PinInvalid`
- PIN kedaluwarsa setelah TTL
- pairing tanpa `begin_pairing()` → `PairingBusy`
- token dicabut saat sesi hidup → koneksi tertutup
- reconnect dengan token lama → sukses tanpa PIN
- reconnect dengan token yang sudah dicabut → gagal, client harus pairing lagi
- 9 koneksi serentak → yang ke-9 ditolak
- sesi tutup → `release_all()` terpanggil (buktikan dengan spy di stub input)
- pesan bertipe tak dikenal → diabaikan, sesi tetap hidup

## Selesai berarti

`cargo test -p tab-host`, `cargo clippy -p tab-host --all-targets -- -D warnings`, dan
`cargo fmt --check -p tab-host` hijau. Centang 0.5 di `docs/STEPS.md`.

## Jebakan yang sudah diketahui

- Nonce Noise maju setiap pesan, jadi frame yang sama tidak bisa didekripsi dua kali. Jangan
  pernah men-decrypt ulang buffer yang sama saat menangani error.
- `Noise_NKpsk2` mencampur PSK pada pesan **kedua**, jadi token yang salah terdeteksi oleh
  *client*, bukan host. Perlakukan handshake yang mati di tengah sebagai hal normal.
- Jangan menaruh logika pairing di dalam task per-koneksi: PIN itu state milik host, satu
  untuk semua koneksi, dan harus tahan dua perangkat mencoba bersamaan.
