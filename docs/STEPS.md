# Langkah Kerja — Tab

Status: **Fase 0 sebagian selesai.** Tanda `[x]` sudah jalan dan terverifikasi.

Aturan main: setiap langkah punya cara verifikasinya sendiri. Jangan lanjut ke langkah
berikutnya sebelum verifikasi langkah itu hijau.

---

## Fase 0 — Fondasi & protokol

- [x] **0.1** Pasang toolchain Rust (1.98.1, rustup)
- [x] **0.2** Tulis `protocol/PROTOCOL.md` — framing, pola Noise, katalog pesan, resiliensi
- [x] **0.3** Workspace Cargo + crate `tab-protocol` (framing, discovery, pesan, sesi Noise)
      → *verifikasi:* 26 test lulus, clippy `-D warnings` bersih, fmt bersih
- [x] **0.4** Fixture CBOR di `protocol/fixtures/` sebagai acuan uji kontrak Kotlin

- [ ] **0.5** Crate `tab-host` (library, bisa dites headless tanpa Tauri)
      - responder discovery UDP 4179 + rate limit per alamat
      - listener TCP + manajer sesi (maks 8 sesi serentak)
      - state machine pairing: PIN 6 digit CSPRNG, TTL 120 s, maks 5 percobaan, banding waktu-konstan
      - registry perangkat + penyimpanan token & static keypair via crate `keyring`
      - pelepasan sumber daya per sesi: **semua tombol/mouse yang tertahan wajib dilepas** saat sesi tutup
      → *verifikasi:* test integrasi di dalam proses — pairing sukses, PIN salah 5×, PIN kedaluwarsa, token dicabut, reconnect dengan token lama

- [ ] **0.6** Crate `tab-probe` (CLI client uji)
      - `probe discover` — temukan host di LAN
      - `probe pair --pin 048213` — pairing, simpan token ke file lokal
      - `probe rtt --duration 30s` — kirim Ping 2 Hz + input sintetis, cetak p50/p95/p99
      - `probe input --pattern circle` — gerakan pointer sintetis untuk uji kehalusan
      → *verifikasi:* `probe` + `tab-host` bicara end-to-end tanpa HP maupun Tauri

- [ ] **0.7** Scaffold Tauri v2 di `host/`
      - tray/menu bar icon, autostart, jendela dibuka dari tray
      - bungkus `tab-host` sebagai state; expose Tauri command + event ke UI
      - UI minimal: daftar perangkat, dialog pairing (tampilkan PIN), tombol cabut perangkat
      → *verifikasi:* `npm run tauri dev`, `probe` berhasil pairing lewat PIN yang tampil di UI

- [ ] **0.8** Scaffold Android (`client-android/`, Gradle multi-module, minSdk 26)
      - `core/model` — tipe pesan CBOR, `classDiscriminator = "t"`
      - **uji kontrak** `core/model` terhadap `protocol/fixtures/*.cbor`
      - `core/net` — discovery, pairing, sesi Noise, heartbeat, reconnect backoff
      - `app` — daftar host, layar pairing, navigasi swipe antar mode (mode masih kosong)
      → *verifikasi:* `./gradlew test` hijau termasuk uji kontrak; `installDebug` lalu pairing ke host asli

- [ ] **0.9** CI GitHub Actions matrix `macos-latest` + `windows-latest`
      → clippy, cargo test, gradle test, build bundle

---

## Fase 1 — Input & Trackpad (inti)

- [ ] **1.1** `trait PlatformInput` + implementasi macOS (`core-graphics`) & Windows (`SendInput`)
- [ ] **1.2** Pointer & tombol via `enigo`; sensitivitas diterapkan di host
- [ ] **1.3** Scroll halus satuan piksel (turun ke API platform, enigo tidak cukup) + Natural/Inverted
- [ ] **1.4** Modifier (Ctrl/Alt/Cmd/Shift) + keyboard + `Text` dari IME
- [ ] **1.5** Gesture 4-jari untuk Spaces macOS; di Windows `caps.gestures` dikirim kosong
- [ ] **1.6** Clipboard sync dua arah, batas 64 KiB, tidak pernah ditulis ke disk/log
- [ ] **1.7** Cek `AXIsProcessTrusted` + panduan izin Accessibility di UI host
- [ ] **1.8** Client: surface multi-touch Compose → klasifikasi tap/scroll/drag, inersia, slider DPI
- [ ] **1.9** Panduan gesture sekali-tampil untuk pengguna baru
      → *verifikasi:* `probe rtt` p99 < ~15 ms di Wi-Fi lokal; uji manual pointer/scroll/modifier di Mac

---

## Fase 2 — Deck mode

- [ ] **2.1** Tipe aksi: `Hotkey`, `LaunchApp`, `OpenPath`, `OpenUrl`, `MediaKey`, `Multi`
- [ ] **2.2** Profil deck persisten (`profiles.rs`) + `DeckProfiles`/`SelectProfile`
- [ ] **2.3** Editor profil di UI host (grid, label, ikon, warna, drag-and-drop)
- [ ] **2.4** Integrasi OBS via `obws`; koneksi opsional, diumumkan lewat `caps.obs`
- [ ] **2.5** Client: render grid dari profil + haptic feedback + `DeckFeedback`
      → *verifikasi:* tekan tombol → hotkey jalan, app terbuka, scene OBS berganti; aksi gagal memunculkan feedback, bukan diam

---

## Fase 3 — System monitor

- [ ] **3.1** Sampler `sysinfo`: CPU per core, memori, swap, disk, Rx/Tx, status daya
- [ ] **3.2** Langganan telemetri per sesi, interval dinegosiasikan, berhenti saat mode tidak aktif
- [ ] **3.3** Suhu CPU/GPU di belakang `caps.temps`/`caps.gpu` — v1 keduanya `false`
- [ ] **3.4** Client: gauge live, sembunyikan yang kapabilitasnya `false`
      → *verifikasi:* angka cocok dengan Activity Monitor / Task Manager

---

## Fase 4 — Music & Lyrics + Desk Clock

- [ ] **4.1** Windows: `GlobalSystemMediaTransportControlsSessionManager` via crate `windows`
- [ ] **4.2** macOS: sidecar mediaremote-adapter (jalur utama) + fallback AppleScript/JXA
      — **jangan** solusi yang menuntut SIP dimatikan
- [ ] **4.3** Transport playback, scrub, volume — masing-masing di belakang kapabilitasnya
- [ ] **4.4** Artwork: dikirim sekali per lagu dalam chunk ≤ 8 KiB, di-cache client
- [ ] **4.5** Lirik dari file `.lrc` milik user di folder yang ia pilih sendiri; interpolasi posisi di client
- [ ] **4.6** Desk clock murni di client: flip clock landscape, keep-screen-on, peredupan
      → *verifikasi:* putar lagu di Spotify, cek transport/artwork/lirik; cabut izin media → kontrol hilang rapi, bukan error

---

## Fase 5 — Paritas Windows & pengerasan

- [ ] **5.1** Lengkapi `platform/windows.rs` — tidak ada `todo!()` yang tersisa
- [ ] **5.2** Prompt Windows Firewall (Private Network) + preflight check yang bisa diulang dari UI
- [ ] **5.3** Multi-device: beberapa sesi serentak, tiap perangkat mode & state sendiri
- [ ] **5.4** Uji resiliensi otomatis: putus paksa, host restart, token dicabut saat sesi hidup
- [ ] **5.5** Bundling: DMG notarized (macOS), installer + portable (Windows)
      → *verifikasi:* CI matrix hijau di kedua OS; uji manual dua perangkat serentak

---

## Setelah MVP (masing-masing proyek tersendiri)

- [ ] Transport USB: AOA (Android), USBMuxd (iOS) — lapisan `Transport` sudah disiapkan
- [ ] Second Screen: ScreenCaptureKit + virtual display (macOS), driver IddCx (Windows)
- [ ] Gamepad: ViGEmBus (Windows), DriverKit virtual HID (macOS)
- [ ] Client iOS (Swift), wearable, perangkat vintage
