# Track G — Shell Tauri & UI Host

Branch: `track/g-shell`

Kamu membangun aplikasi desktop yang dilihat user: tray/menu bar, daftar perangkat, dialog
pairing, editor profil deck, dan halaman izin. Kamu bekerja **terhadap permukaan publik
`tab-host` yang sudah beku**, dengan implementasi tiruan — jadi tidak perlu menunggu track A.

Baca `docs/prompts/_COMMON.md` lebih dulu.

## File yang kamu miliki

`host/**` kecuali `host/src-tauri/Cargo.toml` bagian dependensi milik orang lain — praktisnya:
`host/package.json`, `host/vite.config.ts`, `host/index.html`, `host/src/**`,
`host/src-tauri/src/main.rs`, `host/src-tauri/tauri.conf.json`, `host/src-tauri/capabilities/**`.

## Yang harus jalan

1. **Scaffold Tauri v2** dengan frontend Vite + TypeScript. Ikuti gaya project
   `/Users/stefanuslo/Projects/Shirushi-GO` untuk setup Vite/TS.
2. **Tray / menu bar** — ikon selalu ada, jendela dibuka dari tray, tutup jendela ≠ keluar
   aplikasi. Plus autostart opsional.
3. **Bungkus `HostHandle`** sebagai state Tauri; teruskan `HostEvent` ke frontend sebagai event
   Tauri, dan expose command: `begin_pairing`, `cancel_pairing`, `devices`, `revoke`.
4. **Layar yang harus ada**
   - **Devices** — daftar perangkat terhubung: nama, platform, mode aktif, RTT terkini, tombol
     cabut. Perangkat tersimpan tapi sedang offline juga tampil.
   - **Pairing** — tombol mulai, **PIN besar dan mudah dibaca dari jarak satu meter** (user
     membacanya sambil memegang HP), hitungan waktu sisa, dan pesan gagal yang menyebut sisa
     percobaan.
   - **Deck editor** — grid drag-and-drop, label, ikon, warna, dan pemilih aksi (langkah 2.3).
   - **Permissions** — status izin Accessibility (macOS) / firewall (Windows) dengan tombol
     yang membuka panel sistem yang tepat, plus preflight check yang bisa dijalankan ulang.
5. **Kapabilitas jujur di UI** — kalau host melaporkan `temps: false` atau `obs: false`,
   kontrolnya tidak ditampilkan. Jangan tampilkan toggle yang tidak ada efeknya.

## Cara kamu menguji tanpa track A selesai

Bikin `MockHost` di `host/src-tauri/src/mock.rs` yang memenuhi permukaan `HostHandle`:
memancarkan `HostEvent` palsu (perangkat tersambung, pairing sukses/gagal, RTT berubah) atas
perintah dari UI dev. Semua layar harus bisa dikembangkan penuh di atas mock ini. Sediakan
flag `--features mock-host` supaya nanti bisa ditukar ke host asli tanpa mengubah UI.

```bash
cd host && npm install && npm run tauri dev
```

## Selesai berarti

`npm run tauri dev` menampilkan aplikasi tray yang lengkap di atas mock; `cargo clippy -p
tab-host-app --all-targets -- -D warnings` dan `cargo fmt --check` hijau. Centang 0.7 dan 2.3
di `docs/STEPS.md`.

## Catatan

Desain visual: gelap, rapat, tenang — ini utilitas yang duduk di menu bar, bukan halaman
pemasaran. Jangan menyalin desain produk lain; buat yang sederhana dan orisinal.
