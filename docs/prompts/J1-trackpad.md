# Track J1 — Android Trackpad

Branch: `track/j1-trackpad`

Ini mode yang menentukan kesan pertama produk. Bagian tersulitnya adalah **klasifikasi
gesture**, dan kabar baiknya: itu logika murni yang bisa diuji tanpa jaringan dan tanpa
perangkat.

Baca `docs/prompts/_COMMON.md` lebih dulu.

## File yang kamu miliki

`client-android/feature/trackpad/**`.

## Yang harus jalan

1. **Klasifikasi gesture sebagai fungsi murni** — masukan: urutan titik sentuh dengan timestamp;
   keluaran: `InputEvent` dari `core/model`. Kelas yang harus dibedakan:
   - tap satu jari → `PointerButton` tekan+lepas
   - geser satu jari → `PointerMove`
   - geser dua jari → `Scroll` dengan fase Begin/Update/End
   - tap dua jari → klik kanan
   - tekan-tahan lalu geser → drag
   - geser empat jari kiri/kanan → `Gesture`
   **Kenapa ini penting:** scroll dua jari yang salah terbaca sebagai tap adalah keluhan klasik
   di produk sejenis. Karena klasifikasinya murni, kasus-kasus itu bisa dikunci dengan test.
2. **Inersia scroll** — lanjutkan scroll dengan peluruhan setelah jari diangkat, tandai
   `momentum: true`.
3. **Batching** — kirim satu batch per frame tampilan lewat API dari `core/net`, jangan satu
   pesan per event sentuh.
4. **Keyboard & modifier** — baris modifier lengket (Ctrl/Alt/Cmd/Shift), input teks dari IME
   dikirim sebagai `Text`, bukan per tombol.
5. **Pengaturan** — slider sensitivitas dan toggle Natural/Inverted yang mengirim
   `SetInputSettings`. Nilainya milik host, jadi tampilkan nilai yang dikirim host lewat
   `InputSettings`, jangan simpan versi sendiri.
6. **Panduan gesture sekali-tampil** untuk pengguna baru.
7. **Clipboard** — tombol kirim/ambil clipboard.

## Cara kamu menguji tanpa track H selesai

Klasifikator adalah fungsi murni: uji dengan urutan titik sentuh sintetis, termasuk kasus
batas — dua jari yang menyentuh 20 ms berselang (harus scroll, bukan dua tap), jari yang
bergerak 2 px lalu lepas (harus tap, bukan drag), dan jari ketiga yang masuk di tengah scroll.
Untuk UI, pakai `@Preview` Compose dan fake pengirim event yang mencatat hasilnya.

```bash
./gradlew :feature:trackpad:test
```

## Selesai berarti

test hijau termasuk kasus batas di atas; uji manual di perangkat setelah track H mendarat.
Centang 1.8 dan 1.9 di `docs/STEPS.md`.
