# Prompt Peluncuran Sesi Paralel

Satu file di folder ini = satu prompt yang bisa di-paste utuh ke sesi Claude Code baru.
Semua sesi bekerja di repo yang sama (`/Users/stefanuslo/Projects/Tab`) tetapi di branch
berbeda dan file yang tidak beririsan.

## Cara pakai

```bash
cd /Users/stefanuslo/Projects/Tab
git checkout -b track/c-input      # nama branch ada di masing-masing prompt
claude                              # lalu paste isi file prompt track itu
```

## Urutan peluncuran

**Gelombang 0 — sendirian, harus selesai lebih dulu**

| Prompt | Track |
|---|---|
| [P0-seam.md](P0-seam.md) | Kontrak, crate kosong, modul Gradle, CI |

Gerbang lulus: `cargo test --workspace` dan `./gradlew test` hijau dengan semua implementasi
masih stub. Jangan luncurkan gelombang 1 sebelum ini hijau — kalau tidak, 14 sesi akan
menebak bentuk seam masing-masing.

**Gelombang 1 — 14 sesi serentak**

| Prompt | Track | Ukuran | Catatan |
|---|---|---|---|
| [E-media.md](E-media.md) | Sumber media *(spike)* | M | **mulai paling awal** — paling banyak ketidakpastian |
| [C-input.md](C-input.md) | Input platform | L | **mulai paling awal** — scroll piksel & gesture |
| [A-transport.md](A-transport.md) | Transport & pairing host | L | jalur utama |
| [B-probe.md](B-probe.md) | Probe CLI | M | pasangan A untuk gerbang latensi |
| [D-metrics.md](D-metrics.md) | Sumber metrik | S | terisolasi |
| [F-deck.md](F-deck.md) | Aksi deck & profil | M | |
| [G-shell.md](G-shell.md) | Shell Tauri + UI | L | pakai mock `tab-host` |
| [H-android-net.md](H-android-net.md) | Android `core/net` | L | jalur utama |
| [J1-trackpad.md](J1-trackpad.md) | Android trackpad | L | klasifikasi gesture = logika murni |
| [J2-deck.md](J2-deck.md) | Android deck | M | |
| [J3-monitor.md](J3-monitor.md) | Android monitor | S | terisolasi |
| [J4-music.md](J4-music.md) | Android music | M | |
| [J5-clock.md](J5-clock.md) | Android desk clock | S | tanpa jaringan sama sekali |
| [K-release.md](K-release.md) | Bundling & rilis | M | |

**Gelombang 2 — integrasi, bukan paralel**

I1 (A+B, gerbang latensi) → I2 (host lengkap) → I3 (pairing dari HP) → I4 (lima mode e2e).
Lihat [../PARALLEL.md](../PARALLEL.md).

## Kalau hanya ada beberapa sesi

Prioritas: **E, C, A, B** lebih dulu (dua yang paling tidak pasti + gerbang latensi), lalu
**H**, lalu sisanya.
