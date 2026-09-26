# Track B — Probe CLI

Branch: `track/b-probe`

Kamu membangun client uji berbasis baris perintah. Nilainya: seluruh jalur pairing dan
**pengukuran latensi** bisa diverifikasi tanpa HP, tanpa Tauri, dan tanpa Android. Ini yang
menjadi alat ukur untuk semua fase berikutnya, jadi angkanya harus bisa dipercaya.

Baca `docs/prompts/_COMMON.md` dan `protocol/PROTOCOL.md` lebih dulu.

## File yang kamu miliki

`crates/tab-probe/**`.

## Perintah yang harus ada

```bash
probe discover                       # broadcast, daftar host + fingerprint + status known
probe pair --host <hid> --pin 048213 # pairing, simpan token ke ~/.config/tab-probe/
probe rtt --duration 30s             # Ping 2 Hz + InputBatch 120 Hz, cetak p50/p95/p99 + drop
probe input --pattern circle|line|scroll --duration 10s
probe modes --mode monitor           # berlangganan telemetri, cetak yang masuk
```

## Yang penting benar

1. **Discovery** kirim ke alamat broadcast **setiap antarmuka aktif**, bukan hanya
   `255.255.255.255` — 3 kali dengan jeda 300 ms, lalu kumpulkan balasan.
2. **Token persisten** di file lokal, plus **pin fingerprint** host. Kalau fingerprint host
   berubah, tolak koneksi dan katakan kenapa — jangan diam-diam pairing ulang.
3. **Pengukuran RTT** memakai jam monotonik (`Instant`), bukan waktu dinding. Laporkan
   p50/p95/p99 dan jumlah batch yang hilang (celah pada `InputBatch.s`).
4. **Reconnect** dengan backoff sesuai §9 spec, supaya `probe` juga berfungsi sebagai alat uji
   resiliensi: matikan host, nyalakan lagi, `probe` harus pulih sendiri.

## Cara kamu menguji tanpa track A selesai

Sediakan `probe serve --fake` di dalam crate ini: host tiruan minimal yang hanya melakukan
handshake, pairing, dan menjawab Ping. Dengan itu `probe rtt` bisa diuji end-to-end di dalam
satu proses. Saat track A mendarat, arahkan ke host asli tanpa mengubah apa pun.

## Selesai berarti

`cargo test -p tab-probe`, clippy, dan fmt hijau; `probe rtt` terhadap `probe serve --fake`
mencetak persentil yang masuk akal. Centang 0.6 di `docs/STEPS.md`.

## Catatan

Ini alat ukur, jadi jangan "memperbaiki" angka. Kalau p99 buruk, laporkan apa adanya — itu
justru fungsi utamanya. Gerbang I1 menuntut p99 < ~15 ms di Wi-Fi lokal, dan gerbang itu
hanya berarti bila alat ukurnya jujur.
