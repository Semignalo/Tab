# Pembagian Kerja Paralel — Tab

Dokumen ini memecah [STEPS.md](STEPS.md) menjadi track yang bisa dikerjakan **bersamaan**
tanpa bertabrakan.

Dua hal yang membuat paralelisme mungkin di sini:

1. **Kontrak sudah beku lebih dulu.** `protocol/PROTOCOL.md` + `protocol/fixtures/*.cbor`
   sudah ada dan teruji, jadi sisi host dan sisi Android bisa dikerjakan tanpa saling menunggu.
2. **Satu concern = satu crate/module = satu pemilik.** File yang dimiliki tiap track tidak
   beririsan, dan setiap track punya cara verifikasi mandiri — tidak ada track yang harus
   menunggu track lain hanya untuk bisa menjalankan test.

> **Penyesuaian dari rencana awal:** semula `modes/*` dan `platform/*` adalah modul di dalam
> `host/src-tauri`. Itu membuat lima pekerjaan berbagi satu crate dan satu `Cargo.toml` —
> sumber konflik dan mustahil dites tanpa membangun Tauri. Karena itu tiap concern dinaikkan
> menjadi crate sendiri. Harganya sedikit boilerplate; imbalannya enam track bisa jalan
> serentak dan masing-masing punya test plus demo binary sendiri.

---

## Gerbang P0 — harus mendarat lebih dulu (blocking, kecil)

Selama P0 belum ada, semua track lain menebak bentuk seam. P0 sengaja hanya berisi
*permukaan*, bukan implementasi: definisi trait, tipe bersama, skeleton modul, dan stub
yang `unimplemented!()`.

| # | Isi | Pemilik |
|---|---|---|
| P0.1 | Crate kosong `tab-host`, `tab-input`, `tab-metrics`, `tab-media`, `tab-deck` + daftar member workspace | integrator |
| P0.2 | Definisi trait: `PlatformInput`, `MetricsSource`, `MediaSource`, `ActionRunner`, `TokenStore`, `DeviceRegistry` | integrator |
| P0.3 | Permukaan publik `tab-host`: `HostConfig`, `HostHandle`, event stream, command — semua stub | integrator |
| P0.4 | Modul Gradle Android + `core/model` (tipe CBOR) + uji kontrak terhadap `protocol/fixtures/` | integrator |
| P0.5 | Workflow CI matrix macOS/Windows yang menjalankan clippy + test seluruh workspace | integrator |

Verifikasi P0: `cargo test --workspace` dan `./gradlew test` hijau dengan seluruh
implementasi masih stub. Setelah itu **`tab-protocol`, `core/model`, dan `protocol/`
berstatus beku** — hanya integrator yang boleh mengubahnya.

---

## Track paralel (semua bisa mulai bersamaan setelah P0)

| ID | Track | File yang dimiliki | Kontrak yang dipegang | Verifikasi mandiri | Ukuran | Butuh |
|---|---|---|---|---|---|---|
| **A** | Transport & pairing host | `crates/tab-host/src/{discovery,listener,session,pairing,store,registry}.rs` | `tab-protocol` | test integrasi dalam proses: pairing sukses, PIN salah 5×, PIN kedaluwarsa, token dicabut, reconnect | L | — |
| **B** | Probe CLI | `crates/tab-probe/**` | `tab-protocol` | `probe discover/pair/rtt/input` terhadap host stub; cetak p50/p95/p99 | M | — |
| **C** | Lapisan input platform | `crates/tab-input/**` | `trait PlatformInput` | `cargo run -p tab-input --example replay -- circle` menggerakkan kursor sungguhan | L | mesin Win untuk sisi Windows |
| **D** | Sumber metrik | `crates/tab-metrics/**` | `trait MetricsSource` | `--example dump` dibandingkan Activity Monitor / Task Manager | S | mesin Win untuk sensor |
| **E** | Sumber media *(SPIKE dulu)* | `crates/tab-media/**` | `trait MediaSource` | `--example watch` mencetak lagu yang sedang diputar; parser `.lrc` punya unit test | M | macOS 15.4+ |
| **F** | Aksi deck & profil | `crates/tab-deck/**` | `trait ActionRunner`, `PlatformInput` | `--example run -- hotkey cmd+c`; test profil persisten; OBS di belakang flag | M | OBS untuk uji scene |
| **G** | Shell Tauri + UI | `host/src/**`, `host/src-tauri/src/main.rs` | permukaan publik `tab-host` | `npm run tauri dev` dengan `tab-host` versi mock | L | — |
| **H** | Android `core/net` | `client-android/core/net/**` | PROTOCOL.md + fixtures | test terhadap host stub Kotlin; test backoff reconnect | L | — |
| **J1** | Android trackpad | `client-android/feature/trackpad/**` | tipe `InputEvent` | **klasifikasi gesture = logika murni**, unit test tanpa jaringan | L | perangkat Android |
| **J2** | Android deck | `client-android/feature/deck/**` | tipe `DeckProfile` | render grid dari fixture profil | M | perangkat Android |
| **J3** | Android monitor | `client-android/feature/monitor/**` | tipe `Metrics` | render gauge dari fixture `metrics.cbor` | S | perangkat Android |
| **J4** | Android music | `client-android/feature/music/**` | tipe `NowPlaying` | render dari fixture + interpolasi posisi | M | perangkat Android |
| **J5** | Android desk clock | `client-android/feature/clock/**` | — | **tanpa jaringan sama sekali**, murni UI | S | perangkat Android |
| **K** | Bundling & rilis | `.github/workflows/**`, config bundler | — | artifact terbangun di kedua OS | M | — |

Track yang paling aman dimulai oleh orang/agen baru: **J5, J3, D** (kecil, terisolasi, tidak
menyentuh jaringan). Track paling berisiko dan paling layak dimulai **paling awal**: **E**
(entitlement MediaRemote macOS) dan **C** (scroll piksel + gesture), karena keduanya
mengandung ketidakpastian platform yang sebaiknya ketemu sekarang, bukan di Fase 4.

---

## Graf dependensi

```
                          ┌──────────── P0 (seam) ────────────┐
                          │                                   │
      ┌────────┬──────────┼─────────┬─────────┬────────┐      │
      A        B          C         D         E        F      G ── (mock tab-host)
      │        │          │         │         │        │      │
      └───I1───┘          └─────────┴────┬────┴────────┘      │
   pairing e2e                           │                    │
   (A+B, tanpa HP)                   I2: host lengkap ────────┘
                                         │
                          ┌──────────────┴──────────────┐
                          H ── I3: pairing dari HP (A+H) │
                          │                             │
                  J1 J2 J3 J4 J5 ── I4: mode e2e ───────┘
                                         │
                                        K
```

## Titik integrasi

| # | Menggabungkan | Bukti lulus |
|---|---|---|
| **I1** | A + B | `probe pair` berhasil ke `tab-host` asli; `probe rtt` mencetak angka. Ini **gerbang latensi**: p99 < ~15 ms sebelum fitur apa pun ditambah. |
| **I2** | A + C + D + E + F + G | Host Tauri jalan di tray, semua kapabilitas dilaporkan jujur sesuai yang benar-benar tersedia |
| **I3** | A + H | HP menemukan host tanpa mengetik IP, pairing dengan PIN, reconnect setelah Wi-Fi mati 10 detik |
| **I4** | + J1..J5 | Swipe antar lima mode, masing-masing berfungsi terhadap host asli |

---

## Aturan anti-konflik

File berikut disentuh banyak track, jadi punya aturan khusus:

| File | Aturan |
|---|---|
| `protocol/PROTOCOL.md`, `protocol/fixtures/*` | **Beku.** Perubahan hanya oleh integrator, dan wajib serentak di sisi Rust + Kotlin + regenerasi fixture |
| `Cargo.toml` (workspace) | Dependensi ditulis berurutan alfabetis; satu track menambah satu baris di `[workspace.dependencies]`, tidak menata ulang baris lain |
| `gradle/libs.versions.toml` | Sama: tambah baris, jangan menata ulang |
| `docs/STEPS.md` | Centang hanya baris milik track sendiri |
| `crates/tab-protocol/**`, `core/model/**` | Beku setelah P0 |

Setiap track bekerja di branch sendiri (`track/a-transport`, `track/c-input`, …) dan hanya
mengubah file di kolom "file yang dimiliki". Kalau sebuah track merasa perlu mengubah kontrak,
itu bukan dikerjakan sendiri — dinaikkan ke integrator, karena satu perubahan kontrak
menyentuh dua sisi implementasi sekaligus.

---

## Yang memang tidak bisa diparalelkan

Jujur soal ini lebih berguna daripada membagi semuanya secara paksa:

- **Gerbang latensi (I1)** butuh A dan B sudah bertemu. Tidak ada gunanya menumpuk fitur di
  atas transport yang belum terbukti cepat.
- **Paritas Windows** (bagian dari C, D, F) tidak bisa diselesaikan tanpa mesin Windows.
  Yang bisa paralel: menulis implementasinya dan membiarkan CI `windows-latest`
  mengompilasi; yang tidak: memastikan rasanya benar.
- **Uji resiliensi & multi-device (Fase 5.3–5.4)** per definisi menguji interaksi antar track,
  jadi ia dikerjakan setelah I3, bukan bersamaan.
- **Track E** bisa jadi menemukan bahwa jalur media macOS tidak bisa dipakai sebagaimana
  diharapkan. Karena itu ia dimulai sebagai **spike bertenggat**, dan hasilnya boleh berupa
  `caps.now_playing = false` — bukan menahan track lain.
