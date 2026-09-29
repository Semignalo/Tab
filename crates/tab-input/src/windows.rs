//! Input Windows lewat `SendInput`.
//!
//! Sisa pecahan piksel disimpan antar panggilan: gerak jari di HP berupa float kecil, dan
//! membulatkan tiap event akan membuat gerakan lambat "macet" (setiap langkah < 0,5 piksel
//! dibulatkan jadi nol).

use crate::keys::{self, Key};
use crate::{InputError, PermissionState, PlatformInput};
use std::collections::HashSet;
use std::mem::size_of;
use tab_protocol::message::{Button, ScrollPhase};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;

/// Satu "notch" roda mouse. Aplikasi modern menerima kelipatan pecahan untuk scroll halus.
const WHEEL_DELTA_F: f32 = 120.0;
/// Piksel gerak jari yang setara dengan satu notch roda.
const PIXELS_PER_NOTCH: f32 = 40.0;

pub struct WindowsInput {
    frac_move: (f32, f32),
    frac_scroll: (f32, f32),
    held_keys: HashSet<Key>,
    held_buttons: HashSet<u8>,
}

impl WindowsInput {
    pub fn new() -> Self {
        Self {
            frac_move: (0.0, 0.0),
            frac_scroll: (0.0, 0.0),
            held_keys: HashSet::new(),
            held_buttons: HashSet::new(),
        }
    }
}

fn send(inputs: &[INPUT]) -> Result<(), InputError> {
    if inputs.is_empty() {
        return Ok(());
    }
    // SAFETY: `inputs` adalah slice INPUT valid dan ukuran struct diberikan apa adanya.
    let sent = unsafe { SendInput(inputs.len() as u32, inputs.as_ptr(), size_of::<INPUT>() as i32) };
    if sent as usize != inputs.len() {
        return Err(InputError::Os(format!(
            "SendInput hanya menerima {sent} dari {} event (jendela dengan hak lebih tinggi?)",
            inputs.len()
        )));
    }
    Ok(())
}

fn mouse(dx: i32, dy: i32, data: i32, flags: u32) -> INPUT {
    INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx,
                dy,
                mouseData: data as _,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn keybd(vk: u16, scan: u16, flags: u32) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: scan,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

/// Ambil bagian bulat dari `value + carry` dan simpan sisanya di `carry`.
fn take_whole(carry: &mut f32, value: f32) -> i32 {
    let total = *carry + value;
    let whole = total.trunc();
    *carry = total - whole;
    whole as i32
}

/// (virtual key, extended?) — tombol extended wajib diberi flag agar tidak terbaca sebagai
/// tombol numpad.
fn vk(key: Key) -> (u16, bool) {
    let plain = |v: u16| (v, false);
    let ext = |v: u16| (v, true);
    match key {
        Key::Letter(b) => plain(b.to_ascii_uppercase() as u16),
        Key::Digit(d) => plain(b'0' as u16 + d as u16),
        Key::F(n) => plain(VK_F1 + (n as u16 - 1)),
        Key::Esc => plain(VK_ESCAPE),
        Key::Tab => plain(VK_TAB),
        Key::CapsLock => plain(VK_CAPITAL),
        Key::Space => plain(VK_SPACE),
        Key::Enter => plain(VK_RETURN),
        Key::Backspace => plain(VK_BACK),
        Key::Delete => ext(VK_DELETE),
        Key::Insert => ext(VK_INSERT),
        Key::Home => ext(VK_HOME),
        Key::End => ext(VK_END),
        Key::PageUp => ext(VK_PRIOR),
        Key::PageDown => ext(VK_NEXT),
        Key::Up => ext(VK_UP),
        Key::Down => ext(VK_DOWN),
        Key::Left => ext(VK_LEFT),
        Key::Right => ext(VK_RIGHT),
        Key::Shift => plain(VK_SHIFT),
        Key::Ctrl => plain(VK_CONTROL),
        Key::Alt => plain(VK_MENU),
        Key::Meta => ext(VK_LWIN),
        Key::Minus => plain(VK_OEM_MINUS),
        Key::Equal => plain(VK_OEM_PLUS),
        Key::BracketLeft => plain(VK_OEM_4),
        Key::BracketRight => plain(VK_OEM_6),
        Key::Backslash => plain(VK_OEM_5),
        Key::Semicolon => plain(VK_OEM_1),
        Key::Quote => plain(VK_OEM_7),
        Key::Comma => plain(VK_OEM_COMMA),
        Key::Period => plain(VK_OEM_PERIOD),
        Key::Slash => plain(VK_OEM_2),
        Key::Grave => plain(VK_OEM_3),
        Key::MediaPlay => ext(VK_MEDIA_PLAY_PAUSE),
        Key::MediaNext => ext(VK_MEDIA_NEXT_TRACK),
        Key::MediaPrev => ext(VK_MEDIA_PREV_TRACK),
        Key::VolumeUp => ext(VK_VOLUME_UP),
        Key::VolumeDown => ext(VK_VOLUME_DOWN),
        Key::Mute => ext(VK_VOLUME_MUTE),
    }
}

fn button_flags(b: Button, down: bool) -> u32 {
    match (b, down) {
        (Button::L, true) => MOUSEEVENTF_LEFTDOWN,
        (Button::L, false) => MOUSEEVENTF_LEFTUP,
        (Button::R, true) => MOUSEEVENTF_RIGHTDOWN,
        (Button::R, false) => MOUSEEVENTF_RIGHTUP,
        (Button::M, true) => MOUSEEVENTF_MIDDLEDOWN,
        (Button::M, false) => MOUSEEVENTF_MIDDLEUP,
    }
}

fn button_id(b: Button) -> u8 {
    match b {
        Button::L => 0,
        Button::R => 1,
        Button::M => 2,
    }
}

impl PlatformInput for WindowsInput {
    fn permission(&self) -> PermissionState {
        PermissionState::NotRequired
    }

    fn open_permission_settings(&self) -> Result<(), InputError> {
        Ok(())
    }

    fn move_pointer(&mut self, dx: f32, dy: f32) -> Result<(), InputError> {
        let ix = take_whole(&mut self.frac_move.0, dx);
        let iy = take_whole(&mut self.frac_move.1, dy);
        if ix == 0 && iy == 0 {
            return Ok(());
        }
        send(&[mouse(ix, iy, 0, MOUSEEVENTF_MOVE)])
    }

    fn move_pointer_abs(&mut self, x: f32, y: f32) -> Result<(), InputError> {
        let scale = |v: f32| (v.clamp(0.0, 1.0) * 65535.0).round() as i32;
        send(&[mouse(
            scale(x),
            scale(y),
            0,
            MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK,
        )])
    }

    fn button(&mut self, b: Button, down: bool) -> Result<(), InputError> {
        send(&[mouse(0, 0, 0, button_flags(b, down))])?;
        if down {
            self.held_buttons.insert(button_id(b));
        } else {
            self.held_buttons.remove(&button_id(b));
        }
        Ok(())
    }

    fn scroll(
        &mut self,
        dx: f32,
        dy: f32,
        _phase: ScrollPhase,
        _momentum: bool,
    ) -> Result<(), InputError> {
        // Windows tidak membedakan fase/momentum; inersia sudah dihitung client.
        let to_wheel = |v: f32| v / PIXELS_PER_NOTCH * WHEEL_DELTA_F;
        // `dy > 0` = konten turun = roda maju = WHEEL positif. HWHEEL positif menggulung ke kanan
        // (konten bergeser kiri), jadi `dx` dibalik.
        let wy = take_whole(&mut self.frac_scroll.1, to_wheel(dy));
        let wx = take_whole(&mut self.frac_scroll.0, to_wheel(-dx));
        let mut events = Vec::with_capacity(2);
        if wy != 0 {
            events.push(mouse(0, 0, wy, MOUSEEVENTF_WHEEL));
        }
        if wx != 0 {
            events.push(mouse(0, 0, wx, MOUSEEVENTF_HWHEEL));
        }
        send(&events)
    }

    fn key(&mut self, key: &str, down: bool) -> Result<(), InputError> {
        let parsed = keys::parse(key)?;
        let (code, extended) = vk(parsed);
        let mut flags = if down { 0 } else { KEYEVENTF_KEYUP };
        if extended {
            flags |= KEYEVENTF_EXTENDEDKEY;
        }
        send(&[keybd(code, 0, flags)])?;
        if down {
            self.held_keys.insert(parsed);
        } else {
            self.held_keys.remove(&parsed);
        }
        Ok(())
    }

    fn text(&mut self, s: &str) -> Result<(), InputError> {
        // Unicode langsung per unit UTF-16: hasil IME (termasuk emoji/CJK) tidak bergantung
        // pada layout keyboard aktif.
        let mut events = Vec::with_capacity(s.len() * 2);
        for unit in s.encode_utf16() {
            events.push(keybd(0, unit, KEYEVENTF_UNICODE));
            events.push(keybd(0, unit, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP));
        }
        send(&events)
    }

    fn gesture(&mut self, g: &str, _fingers: u8) -> Result<(), InputError> {
        Err(InputError::Unsupported(format!("gesture {g} di Windows")))
    }

    fn supported_gestures(&self) -> &'static [&'static str] {
        &[]
    }

    fn release_all(&mut self) -> Result<(), InputError> {
        let mut events = Vec::new();
        for &key in &self.held_keys {
            let (code, extended) = vk(key);
            let flags = KEYEVENTF_KEYUP | if extended { KEYEVENTF_EXTENDEDKEY } else { 0 };
            events.push(keybd(code, 0, flags));
        }
        for &b in &self.held_buttons {
            let btn = match b {
                0 => Button::L,
                1 => Button::R,
                _ => Button::M,
            };
            events.push(mouse(0, 0, 0, button_flags(btn, false)));
        }
        self.held_keys.clear();
        self.held_buttons.clear();
        self.frac_move = (0.0, 0.0);
        self.frac_scroll = (0.0, 0.0);
        send(&events)
    }
}

impl Drop for WindowsInput {
    fn drop(&mut self) {
        let _ = self.release_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fractions_accumulate_instead_of_vanishing() {
        let mut carry = 0.0;
        let total: i32 = (0..10).map(|_| take_whole(&mut carry, 0.3)).sum();
        assert_eq!(total, 3, "10 x 0.3 harus menjadi 3 piksel, bukan 0");
    }

    #[test]
    fn negative_fractions_accumulate_toward_zero() {
        let mut carry = 0.0;
        let total: i32 = (0..10).map(|_| take_whole(&mut carry, -0.3)).sum();
        assert_eq!(total, -3);
    }

    #[test]
    fn extended_keys_are_flagged() {
        assert!(vk(Key::Left).1);
        assert!(vk(Key::Meta).1);
        assert!(!vk(Key::Letter(b'a')).1);
        assert_eq!(vk(Key::Letter(b'a')).0, 0x41);
        assert_eq!(vk(Key::F(12)).0, VK_F12);
    }
}
