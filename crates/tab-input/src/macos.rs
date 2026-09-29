//! Input macOS lewat Core Graphics (`CGEvent`).
//!
//! Butuh izin Accessibility; tanpanya event diam-diam dibuang OS, jadi [`permission`]
//! diperiksa dulu dan UI host menampilkan panduan izin (langkah 1.7).

use crate::keys::{self, Key};
use crate::{InputError, PermissionState, PlatformInput};
use core_graphics::event::{
    CGEvent, CGEventFlags, CGEventTapLocation, CGEventType, CGMouseButton, ScrollEventUnit,
};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use core_graphics::geometry::CGPoint;
use std::collections::HashSet;
use tab_protocol::message::{Button, ScrollPhase};

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXIsProcessTrusted() -> bool;
}

pub struct MacInput {
    held_keys: HashSet<Key>,
    held_buttons: HashSet<u8>,
    flags: CGEventFlags,
}

impl MacInput {
    pub fn new() -> Self {
        Self {
            held_keys: HashSet::new(),
            held_buttons: HashSet::new(),
            flags: CGEventFlags::empty(),
        }
    }
}

fn source() -> Result<CGEventSource, InputError> {
    CGEventSource::new(CGEventSourceStateID::HIDSystemState)
        .map_err(|_| InputError::Os("CGEventSource gagal dibuat".into()))
}

fn cursor(src: &CGEventSource) -> Result<CGPoint, InputError> {
    CGEvent::new(src.clone())
        .map(|e| e.location())
        .map_err(|_| InputError::Os("posisi kursor tidak terbaca".into()))
}

/// Kode tombol virtual macOS (ANSI). Huruf dan angka tidak berurutan, jadi pakai tabel.
fn keycode(key: Key) -> Option<u16> {
    const LETTERS: [u16; 26] = [
        0, 11, 8, 2, 14, 3, 5, 4, 34, 38, 40, 37, 46, 45, 31, 35, 12, 15, 1, 17, 32, 9, 13, 7, 16,
        6,
    ];
    const DIGITS: [u16; 10] = [29, 18, 19, 20, 21, 23, 22, 26, 28, 25];
    const FKEYS: [u16; 20] = [
        122, 120, 99, 118, 96, 97, 98, 100, 101, 109, 103, 111, 105, 107, 113, 106, 64, 79, 80, 90,
    ];
    Some(match key {
        Key::Letter(b) => LETTERS[(b - b'a') as usize],
        Key::Digit(d) => DIGITS[d as usize],
        Key::F(n) => *FKEYS.get(n as usize - 1)?,
        Key::Esc => 53,
        Key::Tab => 48,
        Key::CapsLock => 57,
        Key::Space => 49,
        Key::Enter => 36,
        Key::Backspace => 51,
        Key::Delete => 117,
        Key::Insert => return None,
        Key::Home => 115,
        Key::End => 119,
        Key::PageUp => 116,
        Key::PageDown => 121,
        Key::Up => 126,
        Key::Down => 125,
        Key::Left => 123,
        Key::Right => 124,
        Key::Shift => 56,
        Key::Ctrl => 59,
        Key::Alt => 58,
        Key::Meta => 55,
        Key::Minus => 27,
        Key::Equal => 24,
        Key::BracketLeft => 33,
        Key::BracketRight => 30,
        Key::Backslash => 42,
        Key::Semicolon => 41,
        Key::Quote => 39,
        Key::Comma => 43,
        Key::Period => 47,
        Key::Slash => 44,
        Key::Grave => 50,
        // Tombol media macOS adalah event sistem khusus, bukan CGKeyCode biasa.
        Key::MediaPlay | Key::MediaNext | Key::MediaPrev => return None,
        Key::VolumeUp => 72,
        Key::VolumeDown => 73,
        Key::Mute => 74,
    })
}

fn modifier_flag(key: Key) -> Option<CGEventFlags> {
    match key {
        Key::Shift => Some(CGEventFlags::CGEventFlagShift),
        Key::Ctrl => Some(CGEventFlags::CGEventFlagControl),
        Key::Alt => Some(CGEventFlags::CGEventFlagAlternate),
        Key::Meta => Some(CGEventFlags::CGEventFlagCommand),
        _ => None,
    }
}

impl MacInput {
    fn post_mouse(&self, ty: CGEventType, at: CGPoint, btn: CGMouseButton) -> Result<(), InputError> {
        let ev = CGEvent::new_mouse_event(source()?, ty, at, btn)
            .map_err(|_| InputError::Os("event mouse gagal dibuat".into()))?;
        ev.set_flags(self.flags);
        ev.post(CGEventTapLocation::HID);
        Ok(())
    }

    fn drag_state(&self) -> Option<(CGEventType, CGMouseButton)> {
        if self.held_buttons.contains(&0) {
            Some((CGEventType::LeftMouseDragged, CGMouseButton::Left))
        } else if self.held_buttons.contains(&1) {
            Some((CGEventType::RightMouseDragged, CGMouseButton::Right))
        } else {
            None
        }
    }
}

impl PlatformInput for MacInput {
    fn permission(&self) -> PermissionState {
        // SAFETY: fungsi C tanpa argumen, hanya membaca status proses.
        if unsafe { AXIsProcessTrusted() } {
            PermissionState::Granted
        } else {
            PermissionState::Denied
        }
    }

    fn open_permission_settings(&self) -> Result<(), InputError> {
        std::process::Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
            .spawn()
            .map(|_| ())
            .map_err(|e| InputError::Os(e.to_string()))
    }

    fn move_pointer(&mut self, dx: f32, dy: f32) -> Result<(), InputError> {
        let src = source()?;
        let mut p = cursor(&src)?;
        p.x += dx as f64;
        p.y += dy as f64;
        let (ty, btn) = self
            .drag_state()
            .unwrap_or((CGEventType::MouseMoved, CGMouseButton::Left));
        self.post_mouse(ty, p, btn)
    }

    fn move_pointer_abs(&mut self, x: f32, y: f32) -> Result<(), InputError> {
        let display = core_graphics::display::CGDisplay::main().bounds();
        let p = CGPoint::new(
            display.origin.x + display.size.width * x.clamp(0.0, 1.0) as f64,
            display.origin.y + display.size.height * y.clamp(0.0, 1.0) as f64,
        );
        let (ty, btn) = self
            .drag_state()
            .unwrap_or((CGEventType::MouseMoved, CGMouseButton::Left));
        self.post_mouse(ty, p, btn)
    }

    fn button(&mut self, b: Button, down: bool) -> Result<(), InputError> {
        let at = cursor(&source()?)?;
        let (ty, btn, id) = match (b, down) {
            (Button::L, true) => (CGEventType::LeftMouseDown, CGMouseButton::Left, 0),
            (Button::L, false) => (CGEventType::LeftMouseUp, CGMouseButton::Left, 0),
            (Button::R, true) => (CGEventType::RightMouseDown, CGMouseButton::Right, 1),
            (Button::R, false) => (CGEventType::RightMouseUp, CGMouseButton::Right, 1),
            (Button::M, true) => (CGEventType::OtherMouseDown, CGMouseButton::Center, 2),
            (Button::M, false) => (CGEventType::OtherMouseUp, CGMouseButton::Center, 2),
        };
        self.post_mouse(ty, at, btn)?;
        if down {
            self.held_buttons.insert(id);
        } else {
            self.held_buttons.remove(&id);
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
        // Satuan piksel supaya scroll halus di trackpad-style app; `dy` sudah berupa nilai
        // roda (positif = konten turun) dan CGEvent memakai konvensi yang sama.
        let ev = CGEvent::new_scroll_event(source()?, ScrollEventUnit::PIXEL, 2, dy as i32, dx as i32, 0)
            .map_err(|_| InputError::Os("event scroll gagal dibuat".into()))?;
        ev.post(CGEventTapLocation::HID);
        Ok(())
    }

    fn key(&mut self, key: &str, down: bool) -> Result<(), InputError> {
        let parsed = keys::parse(key)?;
        let code = keycode(parsed)
            .ok_or_else(|| InputError::Unsupported(format!("tombol {key} di macOS")))?;
        if let Some(flag) = modifier_flag(parsed) {
            self.flags.set(flag, down);
        }
        let ev = CGEvent::new_keyboard_event(source()?, code, down)
            .map_err(|_| InputError::Os("event keyboard gagal dibuat".into()))?;
        ev.set_flags(self.flags);
        ev.post(CGEventTapLocation::HID);
        if down {
            self.held_keys.insert(parsed);
        } else {
            self.held_keys.remove(&parsed);
        }
        Ok(())
    }

    fn text(&mut self, s: &str) -> Result<(), InputError> {
        // Kirim string Unicode utuh; tidak bergantung pada layout keyboard.
        for down in [true, false] {
            let ev = CGEvent::new_keyboard_event(source()?, 0, down)
                .map_err(|_| InputError::Os("event keyboard gagal dibuat".into()))?;
            ev.set_string(s);
            ev.post(CGEventTapLocation::HID);
        }
        Ok(())
    }

    fn gesture(&mut self, g: &str, fingers: u8) -> Result<(), InputError> {
        // Spaces/Mission Control dipetakan ke pintasan keyboard bawaan macOS.
        let combo: &[&str] = match (g, fingers) {
            ("spaces_left", _) => &["ctrl", "left"],
            ("spaces_right", _) => &["ctrl", "right"],
            ("mission_control", _) => &["ctrl", "up"],
            _ => return Err(InputError::Unsupported(format!("gesture {g}"))),
        };
        for k in combo {
            self.key(k, true)?;
        }
        for k in combo.iter().rev() {
            self.key(k, false)?;
        }
        Ok(())
    }

    fn supported_gestures(&self) -> &'static [&'static str] {
        &["spaces_left", "spaces_right", "mission_control"]
    }

    fn release_all(&mut self) -> Result<(), InputError> {
        let at = cursor(&source()?)?;
        for id in self.held_buttons.drain().collect::<Vec<_>>() {
            let (ty, btn) = match id {
                0 => (CGEventType::LeftMouseUp, CGMouseButton::Left),
                1 => (CGEventType::RightMouseUp, CGMouseButton::Right),
                _ => (CGEventType::OtherMouseUp, CGMouseButton::Center),
            };
            self.post_mouse(ty, at, btn)?;
        }
        self.flags = CGEventFlags::empty();
        for key in self.held_keys.drain().collect::<Vec<_>>() {
            if let Some(code) = keycode(key) {
                if let Ok(ev) = CGEvent::new_keyboard_event(source()?, code, false) {
                    ev.set_flags(CGEventFlags::empty());
                    ev.post(CGEventTapLocation::HID);
                }
            }
        }
        Ok(())
    }
}

impl Drop for MacInput {
    fn drop(&mut self) {
        let _ = self.release_all();
    }
}
