# Tab

Ubah HP Android menjadi trackpad, deck tombol, monitor sistem, remote musik, dan jam meja untuk
komputer Anda. *Local-first*: tanpa akun, tanpa cloud — trafik hanya lewat Wi-Fi lokal, terenkripsi
(Noise), dengan pairing PIN sekali pakai.

```
host desktop (Tauri v2 + Rust)  ⟷  klien Android (Kotlin + Compose)
        macOS · Windows            protokol LAN: docs → protocol/PROTOCOL.md
```

## Menjalankan

**Komputer (Windows/macOS)**

```powershell
cargo build --release -p tab-desktop      # atau: cd host; npx tauri build
target\release\tab.exe                    # ikon muncul di tray; jendela = daftar perangkat
```

Windows: saat dialog Firewall muncul pilih **Private network** (atau klik “Perbaiki” di kartu
*Koneksi & izin*). Tanpa itu HP tidak akan menemukan komputer.

Tanpa UI (server/tanpa layar): `cargo run -p tab-host --bin tab-hostd -- --pair`.

**HP Android**

```powershell
cd client-android
.\gradlew.bat :app:installDebug           # HP: aktifkan USB debugging & izinkan komputer ini
```

1. Di komputer klik **Pair perangkat baru** → muncul PIN 6 digit (berlaku 120 detik).
2. Di HP buka Tab → pilih komputer → ketik PIN. Selesai; pairing berikutnya otomatis.

## Alat ukur

```powershell
probe discover                    # host di LAN + status
probe pair --pin 048213
probe rtt --duration 30 --ping-hz 20   # p50/p95/p99 RTT sambil mengirim input 120 Hz
```

## Struktur

| Jalur | Isi |
|---|---|
| `protocol/` | spesifikasi wire + fixture CBOR (sumber kebenaran kedua sisi) |
| `crates/tab-protocol` | pesan, framing, discovery, sesi Noise |
| `crates/tab-host` | discovery, listener, sesi, pairing, registry (library + `tab-hostd`) |
| `crates/tab-input` `tab-metrics` `tab-media` `tab-deck` | input OS, metrik, media/lirik, aksi deck + OBS |
| `crates/tab-probe` | client uji baris perintah |
| `host/` | shell Tauri: tray, pairing, perangkat, editor deck, setelan |
| `client-android/` | `core:model`, `core:net`, `feature:*`, `app` |

## Menguji

```powershell
cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings
cargo build -p tab-host --bins            # host uji untuk uji lintas-bahasa
cd client-android; .\gradlew.bat test testDebugUnitTest
cd ..\host; npm test
```

Status per langkah — dan apa yang **belum** terverifikasi (macOS, OBS asli, uji manual di HP) —
ada di [docs/STEPS.md](docs/STEPS.md).
