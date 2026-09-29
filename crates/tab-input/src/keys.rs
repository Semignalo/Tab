//! Nama tombol kanonik (PROTOCOL.md §8) → identitas tombol netral platform.
//!
//! Nama dikirim sebagai string supaya tidak bergantung pada layout fisik. Tabel ini satu-
//! satunya tempat nama itu diterjemahkan; sisi Windows dan macOS hanya memetakan `Key`.

use crate::InputError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    Letter(u8), // b'a'..=b'z'
    Digit(u8),  // 0..=9
    F(u8),      // 1..=24
    Esc,
    Tab,
    CapsLock,
    Space,
    Enter,
    Backspace,
    Delete,
    Insert,
    Home,
    End,
    PageUp,
    PageDown,
    Up,
    Down,
    Left,
    Right,
    Shift,
    Ctrl,
    Alt,
    Meta,
    Minus,
    Equal,
    BracketLeft,
    BracketRight,
    Backslash,
    Semicolon,
    Quote,
    Comma,
    Period,
    Slash,
    Grave,
    MediaPlay,
    MediaNext,
    MediaPrev,
    VolumeUp,
    VolumeDown,
    Mute,
}

pub fn parse(name: &str) -> Result<Key, InputError> {
    let unknown = || InputError::UnknownKey(name.to_owned());
    let bytes = name.as_bytes();
    if bytes.len() == 1 {
        return match bytes[0] {
            b @ b'a'..=b'z' => Ok(Key::Letter(b)),
            b @ b'0'..=b'9' => Ok(Key::Digit(b - b'0')),
            _ => Err(unknown()),
        };
    }
    if let Some(n) = name.strip_prefix('f').and_then(|n| n.parse::<u8>().ok()) {
        return if (1..=24).contains(&n) {
            Ok(Key::F(n))
        } else {
            Err(unknown())
        };
    }
    Ok(match name {
        "esc" => Key::Esc,
        "tab" => Key::Tab,
        "capslock" => Key::CapsLock,
        "space" => Key::Space,
        "enter" => Key::Enter,
        "backspace" => Key::Backspace,
        "delete" => Key::Delete,
        "insert" => Key::Insert,
        "home" => Key::Home,
        "end" => Key::End,
        "pageup" => Key::PageUp,
        "pagedown" => Key::PageDown,
        "up" => Key::Up,
        "down" => Key::Down,
        "left" => Key::Left,
        "right" => Key::Right,
        "shift" => Key::Shift,
        "ctrl" => Key::Ctrl,
        "alt" => Key::Alt,
        "meta" => Key::Meta,
        "minus" => Key::Minus,
        "equal" => Key::Equal,
        "bracketleft" => Key::BracketLeft,
        "bracketright" => Key::BracketRight,
        "backslash" => Key::Backslash,
        "semicolon" => Key::Semicolon,
        "quote" => Key::Quote,
        "comma" => Key::Comma,
        "period" => Key::Period,
        "slash" => Key::Slash,
        "grave" => Key::Grave,
        "media_play" => Key::MediaPlay,
        "media_next" => Key::MediaNext,
        "media_prev" => Key::MediaPrev,
        "volume_up" => Key::VolumeUp,
        "volume_down" => Key::VolumeDown,
        "mute" => Key::Mute,
        _ => return Err(unknown()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_canonical_names() {
        assert_eq!(parse("a").unwrap(), Key::Letter(b'a'));
        assert_eq!(parse("7").unwrap(), Key::Digit(7));
        assert_eq!(parse("f24").unwrap(), Key::F(24));
        assert_eq!(parse("meta").unwrap(), Key::Meta);
        assert_eq!(parse("volume_up").unwrap(), Key::VolumeUp);
    }

    #[test]
    fn rejects_unknown_and_out_of_range() {
        for bad in ["", "A", "f0", "f25", "ff", "hyper", "!", "10"] {
            assert!(parse(bad).is_err(), "{bad} seharusnya ditolak");
        }
    }
}
