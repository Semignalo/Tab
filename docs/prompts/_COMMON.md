# Aturan Bersama Semua Track

Dibaca oleh setiap sesi sebelum menulis kode. Track prompt merujuk file ini, jadi isinya
tidak diulang 14 kali.

## Konteks project

**Tab** adalah sistem yang mengubah HP/tablet menjadi control surface untuk komputer: deck
makro, trackpad, monitor sistem, remote musik, desk clock. Prinsipnya *local-first* — tanpa
akun, tanpa cloud, trafik hanya lewat Wi-Fi lokal atau USB.

Susunannya: host desktop (Tauri v2 + Rust, macOS & Windows) ⟷ client Android (Kotlin +
Compose), disatukan satu protokol LAN terenkripsi dengan pairing PIN sekali pakai.

## Wajib dibaca sebelum mulai

1. `protocol/PROTOCOL.md` — spec wire. **Ini sumber kebenaran**, bukan tebakan dari kode.
2. `docs/PARALLEL.md` — pembagian track, file yang dimiliki siapa, titik integrasi.
3. `docs/STEPS.md` — daftar langkah dan statusnya.
4. `crates/tab-protocol/src/message.rs` — katalog pesan yang sudah ada dan teruji.

## File beku — jangan diubah

- `protocol/PROTOCOL.md`, `protocol/fixtures/*`
- `crates/tab-protocol/**`
- `client-android/core/model/**`

Kalau pekerjaanmu terasa menuntut perubahan kontrak: **jangan lakukan sendiri.** Satu
perubahan kontrak menyentuh dua sisi implementasi sekaligus. Berhenti, tulis apa yang kurang
beserta alasannya, dan laporkan ke user sebagai temuan.

## File yang disentuh banyak track

- `Cargo.toml` root: menambah dependensi berarti **menambah satu baris** di
  `[workspace.dependencies]` secara alfabetis. Jangan menata ulang baris lain.
- `gradle/libs.versions.toml`: sama.
- `docs/STEPS.md`: centang **hanya** baris milik track sendiri.

## Gerbang kualitas

Sebelum menyatakan selesai, semuanya harus hijau:

```bash
cargo test -p <crate-mu>
cargo clippy -p <crate-mu> --all-targets -- -D warnings
cargo fmt --check -p <crate-mu>
```

Untuk track Android:

```bash
cd client-android && ./gradlew :<module>:test :<module>:lint
```

## Konvensi

- Komentar dan pesan error **bahasa Indonesia**, nama identifier bahasa Inggris — ikuti gaya
  `crates/tab-protocol/`.
- Komentar menjelaskan **mengapa**, bukan mengulang apa yang sudah jelas dari kode.
- Setiap unit punya test yang bisa jalan **tanpa** track lain selesai. Kalau butuh track lain,
  bikin stub/mock di dalam crate sendiri.
- `unwrap()` boleh di test, tidak di kode produksi.
- Tidak ada rahasia yang masuk log: token, PIN, isi clipboard, dan kunci privat tidak pernah
  dicetak. `tab-protocol` sudah punya test yang menjaga ini (`debug_output_never_leaks_private_key`).

## Git

- Bekerja di branch sendiri (nama ada di prompt masing-masing).
- Commit ke branch itu silakan. **Jangan** push, merge ke main, atau buka PR tanpa diminta user.
- Jangan sentuh file di luar kolom "file yang kamu miliki" pada prompt-mu.

## Kapasitas yang jujur

Kalau sebuah kapabilitas tidak bisa disediakan di platform tertentu, laporkan `false` lewat
`Capabilities` dan biarkan client menyembunyikan kontrolnya. **Jangan** mengisi nilai
placeholder, nol, atau angka tebakan — gauge yang bohong lebih buruk daripada gauge yang
tidak ada. Ini aturan yang sudah dijaga test di `tab-protocol`.
