//! State machine pairing.
//!
//! PIN itu state milik **host**, satu untuk semua koneksi — bukan milik task per-koneksi.
//! Jumlah percobaan salah dihitung global, jadi dua perangkat yang menebak bergantian tetap
//! kena batas 5 kali yang sama. Waktu disuntikkan sebagai argumen agar mudah dites.

use rand::{rngs::OsRng, Rng};
use std::time::{Duration, Instant};
use tab_protocol::{PIN_DIGITS, PIN_MAX_ATTEMPTS, PIN_TTL};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verify {
    Ok,
    /// PIN salah; `attempts_left == 0` berarti PIN dibatalkan dan tidak berlaku lagi.
    Invalid {
        attempts_left: u8,
    },
    /// Tidak ada PIN aktif: kedaluwarsa, sudah terpakai, habis percobaan, atau dibatalkan.
    Expired,
}

struct Active {
    pin: String,
    expires: Instant,
    attempts_left: u8,
}

#[derive(Default)]
pub struct Pairing {
    active: Option<Active>,
}

impl Pairing {
    pub fn new() -> Self {
        Self::default()
    }

    /// Buka sesi pairing baru; PIN lama (bila ada) langsung tidak berlaku.
    pub fn begin(&mut self, now: Instant) -> (String, Duration) {
        let n: u32 = OsRng.gen_range(0..10u32.pow(PIN_DIGITS as u32));
        let pin = format!("{n:0width$}", width = PIN_DIGITS);
        self.active = Some(Active {
            pin: pin.clone(),
            expires: now + PIN_TTL,
            attempts_left: PIN_MAX_ATTEMPTS,
        });
        (pin, PIN_TTL)
    }

    pub fn cancel(&mut self) {
        self.active = None;
    }

    /// Sisa waktu PIN aktif, atau `None` bila tidak ada / sudah lewat.
    pub fn remaining(&mut self, now: Instant) -> Option<Duration> {
        self.expire_if_due(now);
        self.active
            .as_ref()
            .map(|a| a.expires.saturating_duration_since(now))
    }

    pub fn is_open(&mut self, now: Instant) -> bool {
        self.remaining(now).is_some()
    }

    pub fn verify(&mut self, candidate: &str, now: Instant) -> Verify {
        self.expire_if_due(now);
        let Some(active) = self.active.as_mut() else {
            return Verify::Expired;
        };
        if ct_eq(candidate.as_bytes(), active.pin.as_bytes()) {
            // Sekali pakai: berhasil berarti PIN langsung hangus.
            self.active = None;
            return Verify::Ok;
        }
        active.attempts_left -= 1;
        let left = active.attempts_left;
        if left == 0 {
            self.active = None;
        }
        Verify::Invalid {
            attempts_left: left,
        }
    }

    fn expire_if_due(&mut self, now: Instant) {
        if self.active.as_ref().is_some_and(|a| now >= a.expires) {
            self.active = None;
        }
    }
}

/// Perbandingan waktu-konstan: lama eksekusi tidak bergantung pada posisi digit pertama yang
/// berbeda, sehingga PIN tidak bisa ditebak digit demi digit lewat pengukuran waktu.
pub fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    let mut diff = (a.len() ^ b.len()) as u8;
    for i in 0..a.len().max(b.len()) {
        let x = a.get(i).copied().unwrap_or(0);
        let y = b.get(i).copied().unwrap_or(0);
        diff |= x ^ y;
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pin_is_six_digits_and_varies() {
        let mut p = Pairing::new();
        let now = Instant::now();
        let pins: Vec<_> = (0..20).map(|_| p.begin(now).0).collect();
        assert!(pins
            .iter()
            .all(|s| s.len() == 6 && s.chars().all(|c| c.is_ascii_digit())));
        assert!(
            pins.iter().any(|s| *s != pins[0]),
            "PIN tidak boleh konstan"
        );
    }

    #[test]
    fn correct_pin_works_once() {
        let mut p = Pairing::new();
        let now = Instant::now();
        let (pin, _) = p.begin(now);
        assert_eq!(p.verify(&pin, now), Verify::Ok);
        assert_eq!(p.verify(&pin, now), Verify::Expired, "sekali pakai");
    }

    #[test]
    fn five_wrong_attempts_cancel_the_pin() {
        let mut p = Pairing::new();
        let now = Instant::now();
        let (pin, _) = p.begin(now);
        let wrong = if pin == "000000" { "111111" } else { "000000" };
        for left in (0..5u8).rev() {
            assert_eq!(
                p.verify(wrong, now),
                Verify::Invalid {
                    attempts_left: left
                }
            );
        }
        // PIN yang benar pun sudah tidak berguna setelah dibatalkan.
        assert_eq!(p.verify(&pin, now), Verify::Expired);
        assert!(!p.is_open(now));
    }

    #[test]
    fn pin_expires_after_ttl() {
        let mut p = Pairing::new();
        let t0 = Instant::now();
        let (pin, ttl) = p.begin(t0);
        assert_eq!(p.remaining(t0 + Duration::from_secs(60)), Some(ttl / 2));
        assert_eq!(p.verify(&pin, t0 + ttl), Verify::Expired);
    }

    #[test]
    fn new_session_replaces_old_pin_and_resets_attempts() {
        let mut p = Pairing::new();
        let now = Instant::now();
        let (old, _) = p.begin(now);
        let wrong = if old == "000000" { "111111" } else { "000000" };
        p.verify(wrong, now);
        let (new, _) = p.begin(now);
        assert_eq!(
            p.verify(wrong, now),
            if wrong == new {
                Verify::Ok
            } else {
                Verify::Invalid { attempts_left: 4 }
            }
        );
    }

    #[test]
    fn cancel_closes_pairing() {
        let mut p = Pairing::new();
        let now = Instant::now();
        p.begin(now);
        p.cancel();
        assert!(!p.is_open(now));
    }

    #[test]
    fn ct_eq_handles_length_mismatch() {
        assert!(ct_eq(b"048213", b"048213"));
        assert!(!ct_eq(b"048213", b"048214"));
        assert!(!ct_eq(b"04821", b"048213"));
        assert!(!ct_eq(b"", b"048213"));
        assert!(ct_eq(b"", b""));
    }
}
