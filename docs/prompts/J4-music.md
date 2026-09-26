# Track J4 — Android Music & Lyrics

Branch: `track/j4-music`

Baca `docs/prompts/_COMMON.md` lebih dulu.

## File yang kamu miliki

`client-android/feature/music/**`.

## Yang harus jalan

1. **Tampilan Now Playing** — judul, artis, album, artwork, timeline.
2. **Interpolasi posisi di client.** Host mengirim `pos` paling sering 1 Hz; timeline dan baris
   lirik harus bergerak halus di antaranya, dihitung dari jam lokal dan status `play`. Jangan
   minta host mengirim lebih sering — itu memboroskan bandwidth untuk hal yang bisa dihitung.
3. **Transport** — play/pause/next/prev lewat `MediaCommand`; scrub lewat `MediaSeek`; volume
   lewat `MediaVolume`. Masing-masing **hanya ditampilkan** bila kapabilitasnya `true`
   (`media_seek`, `media_volume`, `now_playing`).
4. **Artwork** — minta lewat `GetArtwork`, terima sebagai chunk `ArtworkChunk` (`i` dari `n`),
   susun kembali, lalu **cache per `art` id**. Jangan meminta ulang artwork untuk lagu yang sama.
5. **Lirik** — `LyricsDoc` berisi baris bertimestamp; sorot baris aktif dan auto-scroll halus.
   Kalau `lyr == false`, bagian lirik tidak ditampilkan sama sekali.

## Cara kamu menguji tanpa track lain

`protocol/fixtures/now_playing.cbor` dan `media_command.cbor` adalah sampel nyata dari sisi
Rust. Uji: interpolasi posisi (maju saat `play`, berhenti saat pause, tidak melewati `dur`),
penyusunan artwork dari chunk yang datang tidak berurutan, chunk yang hilang (harus gagal
rapi, bukan menampilkan gambar rusak), dan sorot baris lirik pada posisi tertentu.

**Untuk test lirik, pakai teks buatan sendiri** (`"baris satu"`, `"baris dua"`). Jangan
memasukkan lirik lagu sungguhan ke repo, termasuk di fixture atau preview.

```bash
./gradlew :feature:music:test
```

## Selesai berarti

test hijau, preview menampilkan pemutar dari fixture. Centang 4.4 dan 4.5 sisi client di
`docs/STEPS.md`.
