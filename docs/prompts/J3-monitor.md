# Track J3 — Android System Monitor

Branch: `track/j3-monitor`

Track kecil dan terisolasi — cocok untuk sesi mana pun. Tidak perlu menunggu track lain.

Baca `docs/prompts/_COMMON.md` lebih dulu.

## File yang kamu miliki

`client-android/feature/monitor/**`.

## Yang harus jalan

1. **Gauge dari `Metrics`**: CPU per core dan rata-rata, memori, swap, disk per volume,
   throughput Rx/Tx, status daya.
2. **Sembunyikan yang tidak tersedia.** Kalau `Capabilities.temps == false`, gauge suhu **tidak
   ditampilkan** — bukan ditampilkan bernilai nol atau "—". Hal yang sama untuk `gpu`. Di v1
   host memang melaporkan keduanya `false`, jadi jalur "tidak tersedia" adalah jalur normal,
   bukan kasus tepi.
3. **Grafik riwayat** pendek (mis. 60 sampel terakhir) untuk CPU dan jaringan, supaya angkanya
   punya konteks.
4. **Rx/Tx sudah berupa byte/detik** dari host — format jadi KB/s atau MB/s, jangan hitung
   selisih lagi.
5. **Langganan** — kirim `SubscribeTelemetry` saat mode dibuka dan berhenti saat ditinggalkan;
   jangan biarkan telemetri mengalir untuk mode yang tidak terlihat.

## Cara kamu menguji tanpa track lain

`protocol/fixtures/metrics.cbor` adalah sampel nyata dari sisi Rust — dan perhatikan: fixture
itu **tidak punya** field `tcpu` dan `gpu`, tepat seperti keadaan v1. Uji: render dari fixture,
render saat `temps`/`gpu` mati (gauge hilang, tata letak tidak berlubang), jumlah core yang
berbeda-beda (4, 8, 16), dan disk tanpa nama.

```bash
./gradlew :feature:monitor:test
```

## Selesai berarti

test hijau, preview menampilkan gauge dari fixture. Centang 3.2 dan 3.4 di `docs/STEPS.md`.

Kalau butuh grafik, muat `dataviz` skill sebelum menulis kode chart pertama.
