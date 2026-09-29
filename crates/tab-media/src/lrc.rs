//! Parser `.lrc` dan pencarian file lirik.

use std::path::{Path, PathBuf};
use tab_protocol::message::LyricLine;

/// Fungsi murni: teks `.lrc` → baris berurutan menurut waktu.
///
/// Mendukung beberapa penanda waktu per baris (`[00:12.00][01:30.50]teks`), pecahan detik
/// 1–3 digit, dan tag `[offset:±ms]`. Tag metadata (`[ar:]`, `[ti:]`, ...) diabaikan.
pub fn parse_lrc(src: &str) -> Vec<LyricLine> {
    let mut offset: i64 = 0;
    let mut out: Vec<LyricLine> = Vec::new();

    for raw in src.lines() {
        let mut rest = raw.trim_start_matches('\u{feff}').trim();
        let mut stamps: Vec<u64> = Vec::new();

        while let Some(inner) = rest.strip_prefix('[') {
            let Some(end) = inner.find(']') else { break };
            let tag = &inner[..end];
            rest = &inner[end + 1..];
            if let Some(ms) = parse_stamp(tag) {
                stamps.push(ms);
            } else if let Some(v) = tag.strip_prefix("offset:") {
                offset = v.trim().parse().unwrap_or(0);
            }
            // tag lain (ar/ti/al/by/...) dilewati
        }

        let text = rest.trim();
        for ms in stamps {
            let shifted = (ms as i64 - offset).max(0) as u64;
            out.push(LyricLine {
                t: shifted,
                s: text.to_owned(),
            });
        }
    }

    out.sort_by_key(|l| l.t);
    out
}

fn parse_stamp(tag: &str) -> Option<u64> {
    let (min, rest) = tag.split_once(':')?;
    let min: u64 = min.parse().ok()?;
    let (sec, frac) = match rest.split_once(['.', ':']) {
        Some((s, f)) => (s, f),
        None => (rest, ""),
    };
    let sec: u64 = sec.parse().ok()?;
    if sec >= 60 || !frac.chars().all(|c| c.is_ascii_digit()) || frac.len() > 3 {
        return None;
    }
    // "5" berarti 0,5 detik, bukan 5 ms: skala menurut jumlah digit.
    let millis = match frac.len() {
        0 => 0,
        1 => frac.parse::<u64>().ok()? * 100,
        2 => frac.parse::<u64>().ok()? * 10,
        _ => frac.parse::<u64>().ok()?,
    };
    Some(min * 60_000 + sec * 1000 + millis)
}

/// Cari `.lrc` untuk sebuah lagu di `dir` (rekursif satu tingkat).
///
/// Urutan: `Artis - Judul.lrc`, lalu `Judul.lrc`, keduanya tanpa membedakan huruf besar/kecil.
pub fn find_lrc(dir: &Path, artist: &str, title: &str) -> Option<PathBuf> {
    let title = normalize(title);
    if title.is_empty() {
        return None;
    }
    let combined = format!("{} - {}", normalize(artist), title);

    let mut candidates: Vec<(String, PathBuf)> = Vec::new();
    collect(dir, 0, &mut candidates);

    candidates
        .iter()
        .find(|(stem, _)| *stem == combined)
        .or_else(|| candidates.iter().find(|(stem, _)| *stem == title))
        .map(|(_, p)| p.clone())
}

fn collect(dir: &Path, depth: u8, out: &mut Vec<(String, PathBuf)>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in rd.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if depth < 1 {
                collect(&path, depth + 1, out);
            }
        } else if path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("lrc"))
        {
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                out.push((normalize(stem), path));
            }
        }
    }
}

fn normalize(s: &str) -> String {
    s.trim().to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_basic_lines_in_order() {
        let lines = parse_lrc("[00:12.50]Halo\n[00:05.00]Awal\n[01:00.00]Akhir");
        let got: Vec<_> = lines.iter().map(|l| (l.t, l.s.as_str())).collect();
        assert_eq!(got, [(5000, "Awal"), (12_500, "Halo"), (60_000, "Akhir")]);
    }

    #[test]
    fn fraction_digits_are_scaled_not_taken_literally() {
        let l = parse_lrc("[00:01.5]a\n[00:01.05]b\n[00:01.005]c");
        let times: Vec<_> = l.iter().map(|x| x.t).collect();
        assert_eq!(times, [1005, 1050, 1500]);
    }

    #[test]
    fn multiple_stamps_share_one_text() {
        let l = parse_lrc("[00:10.00][00:40.00]Reff");
        assert_eq!(l.len(), 2);
        assert!(l.iter().all(|x| x.s == "Reff"));
        assert_eq!((l[0].t, l[1].t), (10_000, 40_000));
    }

    #[test]
    fn metadata_tags_are_ignored_and_offset_applies() {
        let src = "[ar:Seseorang]\n[ti:Judul]\n[offset:500]\n[00:10.00]Baris";
        let l = parse_lrc(src);
        assert_eq!(l.len(), 1);
        assert_eq!(l[0].t, 9_500, "offset positif memajukan lirik");
    }

    #[test]
    fn offset_never_goes_below_zero() {
        let l = parse_lrc("[offset:5000]\n[00:01.00]x");
        assert_eq!(l[0].t, 0);
    }

    #[test]
    fn garbage_and_bom_do_not_panic() {
        assert!(parse_lrc("").is_empty());
        assert!(parse_lrc("[bukan waktu]\n[99:99.99]\n[00:xx]").is_empty());
        let l = parse_lrc("\u{feff}[00:01.00]dengan bom");
        assert_eq!(l.len(), 1);
    }

    #[test]
    fn empty_text_lines_are_kept_as_gaps() {
        // Baris kosong bertanda waktu dipakai penyanyi untuk "jeda"; UI perlu tahu.
        let l = parse_lrc("[00:05.00]\n[00:09.00]lanjut");
        assert_eq!(l[0].s, "");
        assert_eq!(l.len(), 2);
    }

    #[test]
    fn finds_file_case_insensitively_and_prefers_artist_title() {
        let dir = std::env::temp_dir().join(format!("tab-lrc-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("Judul.lrc"), "[00:01.00]a").unwrap();
        std::fs::write(dir.join("ARTIS - JUDUL.LRC"), "[00:01.00]b").unwrap();

        let found = find_lrc(&dir, "Artis", "Judul").unwrap();
        assert!(found
            .to_string_lossy()
            .to_lowercase()
            .contains("artis - judul"));
        let title_only = find_lrc(&dir, "Lain", "judul").unwrap();
        assert!(title_only.ends_with("Judul.lrc"));
        assert!(find_lrc(&dir, "Artis", "Tidak Ada").is_none());
        std::fs::remove_dir_all(&dir).ok();
    }
}
