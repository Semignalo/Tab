# Track C — Lapisan Input Platform

Branch: `track/c-input`

Kamu membangun bagian yang menentukan apakah produk ini terasa instan atau terasa murah.
**Mulai lebih awal**: di sinilah ketidakpastian platform paling banyak.

Baca `docs/prompts/_COMMON.md` lebih dulu. Track ini **tidak menyentuh jaringan sama sekali**.

## File yang kamu miliki

`crates/tab-input/**` — `lib.rs` (trait sudah beku dari P0), `macos.rs`, `windows.rs`,
`examples/replay.rs`.

## Yang harus jalan

1. **`enigo` sebagai baseline** untuk pointer, tombol, dan keyboard.
2. **Scroll halus satuan piksel — enigo tidak cukup.** Turun ke API platform:
   `CGEventCreateScrollWheelEvent` dengan unit piksel di macOS (crate `core-graphics`),
   `SendInput` + `WHEEL_DELTA` di Windows. Hormati fase `Begin`/`Update`/`End` dan flag
   momentum supaya inersia terasa wajar.
3. **Peta nama tombol kanonik** dari `protocol/PROTOCOL.md` §8 ke keycode tiap OS. `meta` →
   Command di macOS, Windows key di Windows.
4. **Gesture** empat jari untuk pindah Spaces di macOS. Di Windows, `supported_gestures()`
   mengembalikan slice kosong — itu jawaban yang benar, bukan kekurangan yang harus ditutupi.
5. **Izin macOS**: `PermissionState` dari `AXIsProcessTrusted`, dan
   `open_permission_settings()` membuka System Settings → Privacy & Security → Accessibility.
   Tanpa izin ini macOS **membuang event tanpa error**, jadi deteksinya harus eksplisit.
6. **`release_all()`** melepas semua tombol dan tombol mouse yang tercatat masih tertekan.
   Simpan state tombol yang ditekan di dalam crate ini.
7. **`Clipboard`** untuk kedua OS, batas 64 KiB. Isi clipboard tidak pernah masuk log.

## Cara kamu menguji

```bash
cargo run -p tab-input --example replay -- circle    # kursor bergerak melingkar sungguhan
cargo run -p tab-input --example replay -- scroll
cargo run -p tab-input --example replay -- keys
```

Unit test untuk yang bisa diuji tanpa efek samping: peta nama tombol, pelacakan tombol
tertekan, dan `release_all()` melepas tepat yang tercatat. Untuk hal yang harus dilihat mata,
tulis checklist manual singkat di `crates/tab-input/README.md`.

## Selesai berarti

clippy + fmt + test hijau di macOS; `windows.rs` **ikut dikompilasi** (biarkan CI
`windows-latest` membuktikannya) walau belum bisa kamu uji dengan tangan. Centang 1.1–1.7 di
`docs/STEPS.md` untuk bagian yang benar-benar terverifikasi, dan tulis terang mana yang belum
teruji di mesin Windows.

## Jebakan yang sudah diketahui

- Tanpa izin Accessibility, macOS diam saja — bukan error. Kalau kamu tidak memeriksanya
  duluan, kamu akan mengejar bug yang sebenarnya cuma izin.
- Scroll baris (line-based) terasa tersendat. Yang dituju adalah scroll piksel.
- Delta pointer datang sudah di-coalesce dari client; jangan menumpuk smoothing lagi di sini
  atau kursor jadi terasa melayang.
