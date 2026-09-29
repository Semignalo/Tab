//! Verifikasi input sungguhan: `cargo run -p tab-input --example replay -- circle|check`.
//!
//! `circle` menggerakkan kursor melingkar; `check` (Windows) menggeser kursor 60 px lalu
//! mengembalikannya dan **membaca posisinya dari OS**, sehingga terbukti kursor benar-benar
//! bergerak, bukan sekadar panggilan API yang tidak error.

use std::time::Duration;
use tab_input::platform_input;

#[cfg(windows)]
fn cursor() -> (i32, i32) {
    use windows_sys::Win32::Foundation::POINT;
    use windows_sys::Win32::UI::WindowsAndMessaging::GetCursorPos;
    let mut p = POINT { x: 0, y: 0 };
    // SAFETY: `p` valid dan diisi oleh API.
    unsafe { GetCursorPos(&mut p) };
    (p.x, p.y)
}

fn main() {
    let mode = std::env::args().nth(1).unwrap_or_else(|| "check".into());
    let mut input = platform_input();
    println!("izin: {:?}", input.permission());

    match mode.as_str() {
        "circle" => {
            let mut prev = (0.0f32, 0.0f32);
            for i in 0..=240 {
                let t = i as f32 / 60.0;
                let (x, y) = (
                    120.0 * (t * std::f32::consts::TAU).cos(),
                    120.0 * (t * std::f32::consts::TAU).sin(),
                );
                input.move_pointer(x - prev.0, y - prev.1).unwrap();
                prev = (x, y);
                std::thread::sleep(Duration::from_millis(16));
            }
            input.move_pointer(-prev.0, -prev.1).unwrap();
        }
        _ => {
            #[cfg(windows)]
            {
                let before = cursor();
                // Beberapa langkah pecahan: menguji juga akumulasi sisa piksel.
                for _ in 0..600 {
                    input.move_pointer(0.1, 0.0).unwrap();
                }
                let after = cursor();
                println!("kursor {before:?} → {after:?}");
                let dx = after.0 - before.0;
                // Percepatan pointer Windows bisa mengubah jarak; yang dibuktikan di sini adalah
                // bahwa kursor bergerak searah dan bukan nol.
                assert!(dx > 0, "kursor tidak bergerak (dx={dx})");
                for _ in 0..600 {
                    input.move_pointer(-0.1, 0.0).unwrap();
                }
                println!("kembali ke {:?}", cursor());
                // Modifier ditekan lalu dilepas lewat release_all.
                input.key("shift", true).unwrap();
                input.release_all().unwrap();
                println!("OK: gerak kursor dan release_all bekerja");
            }
            #[cfg(not(windows))]
            println!("mode check hanya untuk Windows; gunakan `circle`");
        }
    }
}
