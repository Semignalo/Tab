# Track E — Sumber Media (SPIKE lebih dulu)

Branch: `track/e-media`

**Mulai paling awal.** Ini track dengan ketidakpastian terbesar di seluruh project, dan lebih
baik ketemu sekarang daripada di fase akhir.

Baca `docs/prompts/_COMMON.md` lebih dulu.

## File yang kamu miliki

`crates/tab-media/**` — `lib.rs` (trait beku dari P0), `macos.rs`, `windows.rs`, `lrc.rs`,
`examples/watch.rs`.

## Fase 1: spike bertenggat (kerjakan ini dulu, lapor hasilnya)

**Masalahnya sudah diketahui:** sejak macOS 15.4, daemon `mediaremoted` memverifikasi
entitlement, sehingga proses biasa yang memanggil MediaRemote **ditolak** — `NowPlaying`
kembali nil. Jadi jangan mulai dengan memanggil framework itu langsung.

Urutan yang harus dicoba:

1. **Pendekatan adapter** — jalankan perantara yang memang sudah ter-entitle oleh sistem
   (pola yang dipakai proyek `mediaremote-adapter`), lalu baca hasilnya sebagai sidecar dari
   Rust. Ini jalur utama.
2. **AppleScript/JXA** sebagai fallback untuk Music dan Spotify. Lebih terbatas (tidak
   mencakup media berbasis browser) tapi tidak menuntut apa pun dari user.
3. **Windows**: `GlobalSystemMediaTransportControlsSessionManager` lewat crate `windows`. Ini
   jalur resmi dan mencakup Spotify, Apple Music, dan media di browser.

**Batas keras: jangan pilih solusi apa pun yang menuntut SIP dimatikan, atau yang
menyuntikkan kode ke proses sistem.** Kalau ternyata tidak ada jalan yang bersih di macOS,
hasil yang benar adalah `MediaCaps { now_playing: false, .. }` dan laporan singkat tentang
kenapa — bukan menahan track lain, dan bukan menurunkan keamanan mesin user.

Tulis temuanmu di `crates/tab-media/README.md`: apa yang bekerja, di versi macOS berapa, dan
apa yang harus user izinkan.

## Fase 2: implementasi

1. `now_playing()` — judul, artis, album, durasi, posisi, status putar. Posisi cukup di-refresh
   1 Hz; client yang menginterpolasi.
2. `artwork()` — kembalikan byte gambar; host yang memecahnya jadi chunk ≤ 8 KiB. Artwork
   dikirim **sekali per lagu**, bukan setiap sampel telemetri.
3. `command()`, `seek()`, `volume()` — masing-masing hanya diaktifkan bila backend benar-benar
   mendukungnya, dan itu tercermin di `MediaCaps`.
4. **`parse_lrc()`** — fungsi murni, tanpa I/O, mudah diuji: parse `[mm:ss.xx]` termasuk
   beberapa timestamp pada satu baris, baris metadata (`[ar:]`, `[ti:]`) diabaikan, urutkan
   menurut waktu, dan bertahan terhadap file yang cacat tanpa panic.
5. **`LyricsLibrary`** — cari file `.lrc` **milik user** di folder yang ia tentukan sendiri,
   dicocokkan dengan judul/artis lagu yang sedang diputar.

**Lirik tidak pernah diambil dari layanan pihak ketiga.** Satu-satunya sumber adalah file
`.lrc` yang sudah ada di komputer user. Untuk test, pakai teks buatan sendiri (`"baris satu"`,
`"baris dua"`) — jangan menaruh lirik lagu sungguhan ke dalam repo, termasuk di fixture.

## Cara kamu menguji

```bash
cargo run -p tab-media --example watch     # cetak lagu yang sedang diputar setiap detik
```

Unit test penuh untuk `parse_lrc()` (ini bagian yang paling mudah diuji dan paling mudah
salah). Untuk jalur platform, checklist manual di README.

## Selesai berarti

test + clippy + fmt hijau, `--example watch` menampilkan lagu yang benar di macOS **atau**
laporan jelas bahwa `now_playing` harus `false` beserta alasannya. Centang 4.1–4.5 sesuai apa
yang benar-benar tercapai.
