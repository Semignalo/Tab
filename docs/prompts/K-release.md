# Track K — Bundling & Rilis

Branch: `track/k-release`

Baca `docs/prompts/_COMMON.md` lebih dulu.

## File yang kamu miliki

`.github/workflows/**` (selain `ci.yml` milik P0), konfigurasi bundler di
`host/src-tauri/tauri.conf.json` bagian `bundle`, dan `docs/RELEASE.md`.

## Yang harus jalan

1. **Bundle macOS** — DMG universal (x86_64 + aarch64). Siapkan jalur penandatanganan Developer
   ID dan notarization lewat secret repo, tapi **jangan pernah menaruh sertifikat, kunci, atau
   kredensial ke dalam repo**. Kalau secret tidak tersedia, build tetap jalan dan menghasilkan
   artifact tak bertanda tangan dengan peringatan jelas di log.
2. **Bundle Windows** — installer NSIS/MSI + versi portable, x64 (dan x86 bila mudah).
3. **Workflow rilis** — terpicu oleh tag `v*`, membangun di `macos-latest` dan
   `windows-latest`, lalu mengunggah artifact ke GitHub Release sebagai **draft** (biar user
   yang menekan publish).
4. **Checksum SHA-256** untuk setiap artifact, ditulis ke berkas checksum di rilis. Kalau
   build Windows tidak ditandatangani, sebutkan itu terang-terangan di catatan rilis beserta
   cara memverifikasi checksum dengan `Get-FileHash` — jangan menyarankan user mematikan
   SmartScreen.
5. **APK Android** — build debug di CI; jalur release ditandatangani dengan keystore dari
   secret. Keystore tidak pernah masuk repo.
6. **`docs/RELEASE.md`** — langkah rilis, secret apa saja yang dibutuhkan, dan cara
   memverifikasi artifact.

## Selesai berarti

Workflow berhasil membangun artifact kedua OS pada dry-run (tag uji di branch sendiri),
checksum tercatat, dan tidak ada satu pun rahasia di dalam repo. Centang 5.5 di `docs/STEPS.md`.
