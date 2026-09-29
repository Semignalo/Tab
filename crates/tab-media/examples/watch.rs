//! Cetak lagu yang sedang diputar tiap detik. Putar sesuatu di Spotify/browser lalu jalankan.

fn main() {
    let mut media = tab_media::platform_media();
    println!("caps: {:?}", media.capabilities());
    for _ in 0..10 {
        match media.now_playing() {
            Ok(Some(np)) => println!(
                "{} — {} [{}] {}/{} ms {} art={:?}",
                np.artist,
                np.title,
                np.album,
                np.pos,
                np.dur,
                if np.play { "▶" } else { "⏸" },
                np.art
            ),
            Ok(None) => println!("(tidak ada yang diputar)"),
            Err(e) => println!("error: {e}"),
        }
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}
