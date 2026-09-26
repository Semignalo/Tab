# Track J5 — Android Desk Clock

Branch: `track/j5-clock`

Track paling terisolasi di seluruh project: **tidak menyentuh jaringan sama sekali** dan tidak
bergantung pada track mana pun. Titik awal yang bagus untuk sesi baru.

Baca `docs/prompts/_COMMON.md` lebih dulu.

## File yang kamu miliki

`client-android/feature/clock/**`.

## Yang harus jalan

1. **Flip clock landscape** — jam besar, terbaca dari seberang meja, dengan animasi flip yang
   halus. Buat desain orisinal; jangan meniru tampilan aplikasi jam tertentu.
2. **Keep-screen-on** selama mode ini aktif, dan dilepas begitu ditinggalkan — perangkat idle
   tidak boleh menahan layar menyala setelah user pindah mode.
3. **Peredupan otomatis** — setelah beberapa menit tanpa interaksi, turunkan kecerahan; sentuhan
   mengembalikannya. Tujuannya alat ini bisa ditinggal menyala semalam di atas meja.
4. **Hemat daya** — perbarui tepat sekali per detik (atau per menit bila detik tidak
   ditampilkan). Jangan menggambar ulang setiap frame; ini mode yang menyala berjam-jam.
5. **Format 12/24 jam** mengikuti setelan sistem, plus tanggal opsional.

## Cara kamu menguji

Logika waktu dipisahkan dari UI supaya bisa diuji dengan jam yang bisa dikontrol: uji peralihan
23:59→00:00, tengah hari pada format 12 jam, dan bahwa tidak ada pembaruan lebih sering dari
sekali per detik.

```bash
./gradlew :feature:clock:test
```

## Selesai berarti

test hijau, preview Compose landscape terlihat benar. Centang 4.6 di `docs/STEPS.md`.
