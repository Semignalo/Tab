# Track J2 — Android Deck

Branch: `track/j2-deck`

Baca `docs/prompts/_COMMON.md` lebih dulu.

## File yang kamu miliki

`client-android/feature/deck/**`.

## Yang harus jalan

1. **Render grid dari `DeckProfile`** yang dikirim host: `cols` × `rows`, label, ikon, warna.
   Client **hanya tahu `action_id`** — ia tidak tahu dan tidak perlu tahu aksi apa yang
   dijalankan. Jangan membuat UI yang mencoba menyusun aksi di sisi HP.
2. **Tekan/lepas** → `DeckPress`/`DeckRelease`, dengan haptic feedback saat tekan dan state
   visual "sedang ditekan" yang langsung terasa (jangan menunggu balasan host).
3. **`DeckFeedback`** → tampilkan hasil: sukses sekejap, gagal dengan pesan yang bisa dibaca.
   Aksi yang gagal tidak boleh diam.
4. **Pemilih profil** dari `DeckProfiles` → kirim `SelectProfile`.
5. **Tata letak adaptif** — grid harus enak dipakai di HP maupun tablet, portrait dan landscape.

## Cara kamu menguji tanpa track H/F selesai

Baca `protocol/fixtures/deck_profile.cbor` lewat `core/model` dan render dari situ — fixture itu
dihasilkan sisi Rust, jadi ia mewakili bentuk data yang sebenarnya. Uji: grid 4×3 yang sel
kosongnya tidak diisi tombol hantu, label panjang yang tidak memecah tata letak, profil dengan
`cols`/`rows` di luar dugaan, dan `DeckFeedback` gagal yang tampil sebagai pesan.

```bash
./gradlew :feature:deck:test
```

## Selesai berarti

test hijau, preview Compose menampilkan grid dari fixture. Centang 2.5 di `docs/STEPS.md`.
