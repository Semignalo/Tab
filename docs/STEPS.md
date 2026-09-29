# Langkah Kerja — Tab

Status: **semua fase sudah diimplementasikan.** Tanda `[x]` berarti sudah jalan dan diverifikasi di
mesin ini (Windows 11). Tanda `[~]` berarti kodenya ada tetapi **belum bisa diverifikasi di sini**
— alasan dan cara memverifikasinya tertulis di baris itu.

Aturan main: setiap langkah punya cara verifikasinya sendiri.

## Ringkasan verifikasi

| Lapisan | Hasil |
|---|---|
| Rust `cargo test --workspace` | hijau: 24 uji integrasi host, 15 deck (termasuk OBS), 14 media, 12 protokol, dll. |
| Rust `clippy -D warnings` + `fmt` | bersih |
| Android JVM (`./gradlew test testDebugUnitTest`) | 73 uji hijau, termasuk 12 uji **lintas-bahasa** terhadap host Rust sungguhan |
| JS editor deck (`npm test` di `host/`) | 8 uji hijau |
| Latensi loopback `probe rtt` | p50 0,58 ms · p99 1,04 ms · max 2,4 ms (host asli, 10 s, 20 Hz ping + 120 Hz input) |
| Installer | `Tab_0.1.0_x64-setup.exe` (NSIS, 4,9 MB); APK debug 14,7 MB, release 2,5 MB |

---

## Fase 0 — Fondasi & protokol

- [x] **0.1** Toolchain Rust (1.98.1). Di mesin ini: `stable-x86_64-pc-windows-gnu` + mingw (WinLibs).
- [x] **0.2** `protocol/PROTOCOL.md` — dengan amandemen A1–A5 (lihat bagian atas dokumen itu)
- [x] **0.3** Workspace Cargo + crate `tab-protocol`
- [x] **0.4** Fixture CBOR di `protocol/fixtures/`
- [x] **0.5** Crate `tab-host` — discovery + rate limit, listener (maks 8 sesi), pairing PIN,
      registry + keyring, `release_all` saat sesi tutup
      → *verifikasi:* 24 uji integrasi (pairing, PIN salah 5×, PIN habis, token dicabut saat sesi
      hidup, resume, host restart, sesi ke-9 ditolak, timeout senyap, dll.)
      *Catatan:* kedaluwarsa PIN oleh TTL 120 s dites sebagai unit (waktu disuntikkan), bukan
      integrasi, agar suite tidak menunggu 2 menit.
- [x] **0.6** Crate `tab-probe` — `discover`, `pair`, `rtt`, `input`, `modes`, `forget`;
      menolak diam-diam bila kunci host berubah
- [x] **0.7** Shell Tauri v2 di `host/` — tray, autostart, jendela dengan daftar perangkat,
      dialog PIN, cabut perangkat, setelan, preflight
      → *verifikasi:* aplikasi dijalankan, melayani port 4179/4180, UI dirender dan diperiksa
      lewat tangkapan jendela, `probe discover` menemukannya
      *Catatan:* di mesin ini dibangun dengan toolchain GNU (linker mengeluarkan peringatan
      `.rsrc merge failure`); rilis resmi dibangun CI dengan MSVC.
- [x] **0.8** Android `client-android/` — `core:model` (codec CBOR **byte-identik** dengan
      ciborium, uji kontrak terhadap 17 fixture), `core:net` (Noise, discovery, pairing,
      reconnect + backoff, pemulihan mode), `app` + 5 modul fitur
      → *verifikasi:* `./gradlew test` hijau. **Belum:** `installDebug` + pairing ke host asli
      di HP — HP terdeteksi adb tetapi menunggu izin USB debugging (lihat “Yang perlu Anda
      lakukan” di README/laporan).
- [x] **0.9** CI GitHub Actions matrix macOS + Windows (`.github/workflows/ci.yml`) + rilis
      (`release.yml`). *Belum dijalankan di GitHub* — baru terbukti setelah push.

---

## Fase 1 — Input & Trackpad

- [x] **1.1** `PlatformInput` + Windows (`SendInput`)
      → *verifikasi:* `cargo run -p tab-input --example replay -- check` menggeser kursor dan
      membaca posisinya kembali lewat `GetCursorPos`
- [~] **1.1** macOS (`CGEvent`) — ditulis, **tidak terverifikasi**: tidak ada mesin macOS. CI macOS
      akan mengompilasinya; uji manual di Mac tetap perlu (izin Accessibility).
- [x] **1.2** Pointer & tombol; sensitivitas diterapkan di host
- [x] **1.3** Scroll halus satuan piksel + Natural/Inverted (Windows: kelipatan pecahan roda).
      [~] arah `dx` horizontal di macOS belum diverifikasi.
- [x] **1.4** Modifier + keyboard + `Text` dari IME (Unicode langsung, bukan per tombol)
- [~] **1.5** Gesture 4 jari → pintasan Spaces/Mission Control macOS; Windows melaporkan daftar
      kosong (jujur). Sisi macOS tidak terverifikasi.
- [x] **1.6** Clipboard dua arah, batas 64 KiB; host tidak pernah mencatat isi pesan ke log (sisi Kotlin juga menyembunyikannya di `toString`)
- [~] **1.7** `AXIsProcessTrusted` + kartu izin di UI host — ditulis, tidak terverifikasi tanpa Mac
- [x] **1.8** Client: `GestureEngine` murni (16 uji), surface multi-touch Compose, inersia, slider
- [x] **1.9** Panduan gesture sekali-tampil
      → *verifikasi:* uji unit + `probe rtt`. **Belum:** uji manual pointer/scroll dari HP asli.

## Fase 2 — Deck mode

- [x] **2.1** Aksi `Hotkey`, `LaunchApp`, `OpenPath`, `OpenUrl` (hanya http/https/mailto),
      `MediaKey`, `ObsScene`, `Multi` (berhenti di langkah gagal, kedalaman dibatasi)
- [x] **2.2** Profil persisten (`JsonProfileStore`, tulis atomik, file rusak dilewati) + `DeckProfiles`/`SelectProfile`
- [x] **2.3** Editor profil di UI host: grid, label, ikon, warna, seret-untuk-tukar, semua tipe aksi
- [x] **2.4** OBS lewat obs-websocket v5 (klien sendiri, bukan `obws`); `caps.obs` hanya `true`
      bila OBS menjawab saat `Welcome`. Diuji dengan server OBS palsu (dengan/tanpa password,
      scene tak ada). [~] belum diuji terhadap OBS sungguhan.
- [x] **2.5** Client: grid dari profil, haptic, banner `DeckFeedback` (kegagalan tampil, tidak diam)
      → *Catatan keamanan:* `DeckPress` hanya menjalankan aksi yang terpasang pada tombol di
      profil aktif — diuji.

## Fase 3 — System monitor

- [x] **3.1** Sampler `sysinfo`: CPU per core, memori, swap, disk, Rx/Tx, daya
- [x] **3.2** Telemetri per sesi, berhenti saat mode tidak aktif (diuji lintas-bahasa)
- [x] **3.3** Suhu/GPU di belakang `caps.temps`/`caps.gpu` — v1 keduanya `false`, field dihilangkan
- [x] **3.4** Client: gauge live, kartu suhu/GPU tersembunyi bila kapabilitas `false`
      → *verifikasi:* `cargo run -p tab-metrics --example dump`; uji live memastikan tidak ada
      nilai placeholder. Perbandingan visual dengan Task Manager belum dilakukan.

## Fase 4 — Music & Lyrics + Desk Clock

- [x] **4.1** Windows: SMTC via crate `windows`. *Diuji sebatas API berjalan; belum diuji dengan
      lagu yang benar-benar diputar.*
- [~] **4.2** macOS: fallback AppleScript/JXA (Spotify + Music.app; tanpa SIP). Parser dan skrip
      diuji lintas-platform; eksekusi `osascript` tidak terverifikasi. **Sidecar mediaremote-adapter
      belum dibuat**, jadi artwork macOS tidak tersedia (dilaporkan jujur: `art = None`).
- [x] **4.3** Transport, seek, volume di belakang kapabilitas masing-masing (Windows: volume `false`)
- [x] **4.4** Artwork chunk ≤ 7000 byte, prioritas terendah, dirakit + di-cache di client (diuji lintas-bahasa)
- [x] **4.5** Lirik `.lrc` dari folder pilihan user (parser 8 uji); interpolasi posisi di client
- [x] **4.6** Desk clock: flip clock, landscape, keep-screen-on, peredupan

## Fase 5 — Paritas Windows & pengerasan

- [x] **5.1** `windows.rs` lengkap — tidak ada `todo!()`/`unimplemented!()` di seluruh workspace
- [x] **5.2** Preflight Firewall (aturan Private/Domain saja) + tombol “Perbaiki” (UAC) yang bisa diulang
- [x] **5.3** Multi-device: 8 sesi serentak diuji; tiap sesi punya mode/langganan/tombol tertahan sendiri
- [x] **5.4** Resiliensi otomatis: putus paksa (Kotlin), host restart + resume, token dicabut saat
      sesi hidup dan saat peluncuran berikutnya, sesi diam 6 s
- [x] **5.5** Bundling Windows: installer NSIS dibangun. [~] DMG notarized macOS dan installer
      MSVC dibangun CI (`release.yml`) — perlu secret Apple; belum dijalankan.

---

## Setelah MVP (masing-masing proyek tersendiri)

- [ ] Transport USB: AOA (Android), USBMuxd (iOS)
- [ ] Second Screen: ScreenCaptureKit + virtual display (macOS), driver IddCx (Windows)
- [ ] Gamepad: ViGEmBus (Windows), DriverKit virtual HID (macOS)
- [ ] Client iOS (Swift), wearable, perangkat vintage
