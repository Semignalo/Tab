# Track F — Aksi Deck & Profil

Branch: `track/f-deck`

Kamu membangun mesin aksi deck: apa yang terjadi saat tombol di HP ditekan.

Baca `docs/prompts/_COMMON.md` lebih dulu.

## File yang kamu miliki

`crates/tab-deck/**` — `lib.rs` (tipe & trait beku dari P0), `runner.rs`, `profiles.rs`,
`obs.rs`, `examples/run.rs`.

## Yang harus jalan

1. **`ActionRunner`** untuk setiap varian `Action`:
   - `Hotkey` — kombinasi tombol lewat `PlatformInput` (crate `tab-input`); tekan modifier,
     tekan tombol, lepas dalam urutan terbalik.
   - `LaunchApp`, `OpenPath`, `OpenUrl` — lewat mekanisme OS (`open` di macOS, `ShellExecute`
     di Windows).
   - `MediaKey` — tombol media sistem.
   - `ObsScene` — ganti scene lewat obs-websocket v5 (crate `obws`).
   - `Multi` — jalankan berurutan; **satu langkah gagal berarti berhenti** dan laporkan langkah
     mana yang gagal, jangan lanjut membabi buta.
2. **`ProfileStore` persisten** — simpan profil sebagai file di folder konfigurasi host, dengan
   penulisan atomik (tulis ke file sementara lalu rename) supaya profil tidak rusak kalau host
   mati saat menyimpan.
3. **`to_wire()`** — buang seluruh definisi aksi, sisakan `action_id`. Ada alasan keamanan di
   sini: perangkat yang dipasangkan boleh **memicu** aksi milik user, tapi tidak boleh bisa
   **menyusun** perintah baru. Tulis test yang memastikan tidak ada jejak `Action` di hasil
   `to_wire()`.
4. **OBS opsional** — kalau obs-websocket tidak tersambung, `ObsScene` mengembalikan error yang
   bisa dibaca user, dan kapabilitas `obs` dilaporkan `false`. Host tidak boleh gagal start
   hanya karena OBS mati.

## Cara kamu menguji tanpa track C selesai

Pakai `NoopInput` dari P0 sebagai `PlatformInput`, dan spy yang mencatat urutan panggilan.
Dengan itu `Hotkey` dan `Multi` bisa diuji sepenuhnya tanpa menggerakkan komputer sungguhan:

- `Hotkey{["cmd","c"]}` → urutan tekan/lepas yang benar, modifier dilepas terbalik
- `Multi` berhenti di langkah yang gagal dan melaporkan indeksnya
- `to_wire()` tidak membocorkan definisi aksi
- profil bertahan setelah simpan → muat ulang
- `ProfileStore` menolak `action_id` yang tidak punya `ActionDef` (profil tidak konsisten
  lebih baik ditolak saat disimpan daripada gagal saat ditekan)

```bash
cargo run -p tab-deck --example run -- hotkey cmd+c
cargo run -p tab-deck --example run -- url https://example.com
```

## Selesai berarti

test + clippy + fmt hijau. Centang 2.1, 2.2, 2.4 di `docs/STEPS.md` (2.3 editor UI milik track
G, 2.5 milik track J2).
