# Track D — Sumber Metrik

Branch: `track/d-metrics`

Track kecil dan terisolasi: tidak menyentuh jaringan, tidak bergantung track lain. Cocok
dijalankan sesi mana pun.

Baca `docs/prompts/_COMMON.md` lebih dulu.

## File yang kamu miliki

`crates/tab-metrics/**` — `lib.rs` (trait beku dari P0), `sysinfo_source.rs`, `macos.rs`,
`windows.rs`, `examples/dump.rs`.

## Yang harus jalan

1. **Sampler `sysinfo`**: CPU per core (0..100), rata-rata, memori terpakai/total, swap, disk
   terpakai/total per volume, throughput jaringan Rx/Tx dalam **byte per detik**, status daya
   (AC + persentase baterai).
2. **Rx/Tx adalah laju, bukan akumulasi.** `sysinfo` memberi total kumulatif, jadi hitung
   selisih terhadap sampel sebelumnya dibagi waktu nyata yang berlalu. Sampel pertama tidak
   punya pembanding — jangan laporkan angka karangan untuk sampel itu.
3. **CPU butuh dua sampel** berjarak waktu sebelum angkanya berarti. Hormati
   `MINIMUM_CPU_UPDATE_INTERVAL` dari `sysinfo`.
4. **Suhu dan GPU: laporkan `false` di v1.** macOS butuh IOReport/SMC, Windows butuh driver
   sensor terpisah. `MetricsCaps { temps: false, gpu: false }` dan field `tcpu`/`gpu`
   **dihilangkan sepenuhnya** dari `Metrics`. Jangan mengirim nol — ada test di `tab-protocol`
   (`absent_capabilities_are_omitted_entirely`) yang memang menjaga aturan ini.
   Kalau kamu berhasil membaca suhu dengan cara yang tidak menuntut driver tambahan atau hak
   administrator, silakan nyalakan `temps` — tapi itu bonus, bukan syarat selesai.

## Cara kamu menguji

```bash
cargo run -p tab-metrics --example dump          # cetak satu sampel per detik
```

Bandingkan dengan Activity Monitor (macOS) atau Task Manager (Windows) dan catat hasilnya di
`crates/tab-metrics/README.md`. Unit test untuk: perhitungan laju Rx/Tx dari dua sampel
buatan, penanganan sampel pertama, dan `MetricsCaps` yang konsisten dengan field yang benar-
benar terisi.

## Selesai berarti

test + clippy + fmt hijau, angka di `--example dump` cocok dengan alat bawaan OS. Centang
3.1 dan 3.3 di `docs/STEPS.md`.
