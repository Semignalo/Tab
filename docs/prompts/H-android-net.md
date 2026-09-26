# Track H — Android `core/net`

Branch: `track/h-android-net`

Kamu membangun sisi client dari protokol: discovery, pairing, sesi terenkripsi, heartbeat, dan
reconnect. Ini pasangan track A di seberang kabel, dan keduanya dikerjakan **tanpa saling
menunggu** karena kontraknya sudah beku.

Baca `docs/prompts/_COMMON.md` dan `protocol/PROTOCOL.md` (seluruhnya) lebih dulu.

## File yang kamu miliki

`client-android/core/net/**`.

`client-android/core/model/**` sudah beku dari P0 — pakai, jangan ubah.

## Yang harus jalan

1. **Discovery** — kirim datagram ke alamat broadcast **setiap antarmuka aktif**. Android
   sering membuang `255.255.255.255`, jadi hitung broadcast per subnet dari
   `NetworkInterface.getInterfaceAddresses()`. Kirim 3 kali berjeda 300 ms, kumpulkan balasan,
   tampilkan beserta status `known`.
2. **Cache host** — simpan `host_id → (ip, port)` dan **coba koneksi langsung lebih dulu**,
   discovery hanya bila itu gagal. Inilah yang membuat user tidak pernah mengetik alamat IP.
3. **Noise** — `Noise_NK` untuk pairing, `Noise_NKpsk2` untuk resume. Pakai library Noise JVM
   yang mapan; pastikan suite-nya tepat `25519_ChaChaPoly_BLAKE2s`. Verifikasi bahwa
   fingerprint host cocok dengan yang **dipin** saat pairing; kalau berubah, tolak dan
   jelaskan — jangan diam-diam pairing ulang.
4. **Penyimpanan** — token dan static public key host di `EncryptedSharedPreferences`
   (Android Keystore). `device_id` dibuat sekali saat install dan tetap.
5. **Framing** — `u32` big-endian + body, `TCP_NODELAY` menyala, batas `MAX_FRAME`.
6. **Heartbeat & reconnect** — Ping 2 s, timeout 6 s, backoff 250 ms→500→1 s→2 s→4 s batas 5 s
   dengan jitter ±20%. Saat tersambung kembali, **pulihkan sendiri** mode terakhir dan
   langganan telemetri. User tidak boleh diminta pairing ulang karena Wi-Fi sempat mati.
7. **Pesan bertipe tak dikenal diabaikan**, sesi tetap hidup.
8. **Batching input** — sediakan API yang mengumpulkan `InputEvent` dan mengirim satu
   `InputBatch` per frame tampilan (maksimum 64 event), bukan satu frame per sentuhan. Track J1
   yang memasok event-nya.

## Cara kamu menguji tanpa track A selesai

Bikin host tiruan di `src/test/` — server socket Kotlin yang melakukan handshake Noise dan
menjawab Ping — lalu uji seluruh alur di JVM tanpa emulator maupun host asli:

- pairing sukses → token tersimpan → sesi hidup
- PIN salah → pesan error menyebut sisa percobaan
- resume dengan token tersimpan → tanpa PIN
- token ditolak host → token lokal dihapus, kembali ke alur pairing
- fingerprint host berubah → koneksi ditolak
- host mati di tengah → backoff sesuai spec (uji dengan jam virtual, bukan `Thread.sleep`)
- mode & langganan pulih setelah reconnect
- batching: 200 event masuk → keluar sebagai batch-batch ≤ 64

Serialisasi sudah dijamin uji kontrak `core/model` terhadap `protocol/fixtures/`.

## Selesai berarti

`./gradlew :core:net:test` hijau. Centang 0.8 di `docs/STEPS.md`.
