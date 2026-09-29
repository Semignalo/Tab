//! Lapisan input platform.
//!
//! Host tidak pernah menyentuh API OS langsung; semuanya lewat [`PlatformInput`]. Dengan
//! begitu logika sesi bisa dites dengan [`RecordingInput`] tanpa menggerakkan kursor asli.
//!
//! Konvensi arah yang berlaku di seluruh crate ini: `scroll(dx, dy)` menerima nilai
//! *roda*, bukan gerak jari — `dy > 0` berarti konten bergeser ke bawah (roda maju).
//! Pemetaan Natural/Inverted dilakukan host sebelum memanggil sini.

use std::sync::{Arc, Mutex};
use tab_protocol::message::{Button, ScrollPhase};
use thiserror::Error;

pub mod keys;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

#[derive(Debug, Error)]
pub enum InputError {
    #[error("tidak didukung di platform ini: {0}")]
    Unsupported(String),
    #[error("izin belum diberikan: {0}")]
    PermissionDenied(String),
    #[error("nama tombol tidak dikenal: {0}")]
    UnknownKey(String),
    #[error("gagal mengirim input: {0}")]
    Os(String),
    #[error("clipboard: {0}")]
    Clipboard(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionState {
    Granted,
    Denied,
    /// Platform ini tidak meminta izin khusus (Windows).
    NotRequired,
}

pub trait PlatformInput: Send {
    fn permission(&self) -> PermissionState;
    fn open_permission_settings(&self) -> Result<(), InputError>;
    fn move_pointer(&mut self, dx: f32, dy: f32) -> Result<(), InputError>;
    fn move_pointer_abs(&mut self, x: f32, y: f32) -> Result<(), InputError>;
    fn button(&mut self, b: Button, down: bool) -> Result<(), InputError>;
    fn scroll(
        &mut self,
        dx: f32,
        dy: f32,
        phase: ScrollPhase,
        momentum: bool,
    ) -> Result<(), InputError>;
    fn key(&mut self, key: &str, down: bool) -> Result<(), InputError>;
    fn text(&mut self, s: &str) -> Result<(), InputError>;
    fn gesture(&mut self, g: &str, fingers: u8) -> Result<(), InputError>;
    fn supported_gestures(&self) -> &'static [&'static str];
    /// Dipanggil saat sesi berakhir. Modifier yang "nyangkut" setelah koneksi putus adalah
    /// bug yang paling terasa, jadi ini wajib melepas semua tombol & tombol mouse.
    fn release_all(&mut self) -> Result<(), InputError>;
}

pub trait Clipboard: Send {
    fn get(&mut self) -> Result<String, InputError>;
    fn set(&mut self, s: &str) -> Result<(), InputError>;
}

/// Implementasi asli untuk OS yang sedang berjalan.
pub fn platform_input() -> Box<dyn PlatformInput> {
    #[cfg(windows)]
    {
        Box::new(windows::WindowsInput::new())
    }
    #[cfg(target_os = "macos")]
    {
        Box::new(macos::MacInput::new())
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        Box::new(NoopInput)
    }
}

/// Clipboard sistem via `arboard`. Isinya tidak pernah ditulis ke log.
pub struct SystemClipboard(Option<arboard::Clipboard>);

impl SystemClipboard {
    pub fn new() -> Self {
        Self(arboard::Clipboard::new().ok())
    }

    fn inner(&mut self) -> Result<&mut arboard::Clipboard, InputError> {
        if self.0.is_none() {
            self.0 = arboard::Clipboard::new().ok();
        }
        self.0
            .as_mut()
            .ok_or_else(|| InputError::Clipboard("clipboard tidak tersedia".into()))
    }
}

impl Default for SystemClipboard {
    fn default() -> Self {
        Self::new()
    }
}

impl Clipboard for SystemClipboard {
    fn get(&mut self) -> Result<String, InputError> {
        self.inner()?
            .get_text()
            .map_err(|e| InputError::Clipboard(e.to_string()))
    }

    fn set(&mut self, s: &str) -> Result<(), InputError> {
        self.inner()?
            .set_text(s.to_owned())
            .map_err(|e| InputError::Clipboard(e.to_string()))
    }
}

// ------------------------------------------------------------------ stub

/// Tidak melakukan apa pun. Dipakai di platform tanpa implementasi dan di test.
#[derive(Debug, Default)]
pub struct NoopInput;

impl PlatformInput for NoopInput {
    fn permission(&self) -> PermissionState {
        PermissionState::NotRequired
    }
    fn open_permission_settings(&self) -> Result<(), InputError> {
        Ok(())
    }
    fn move_pointer(&mut self, _: f32, _: f32) -> Result<(), InputError> {
        Ok(())
    }
    fn move_pointer_abs(&mut self, _: f32, _: f32) -> Result<(), InputError> {
        Ok(())
    }
    fn button(&mut self, _: Button, _: bool) -> Result<(), InputError> {
        Ok(())
    }
    fn scroll(&mut self, _: f32, _: f32, _: ScrollPhase, _: bool) -> Result<(), InputError> {
        Ok(())
    }
    fn key(&mut self, _: &str, _: bool) -> Result<(), InputError> {
        Ok(())
    }
    fn text(&mut self, _: &str) -> Result<(), InputError> {
        Ok(())
    }
    fn gesture(&mut self, _: &str, _: u8) -> Result<(), InputError> {
        Err(InputError::Unsupported("gesture".into()))
    }
    fn supported_gestures(&self) -> &'static [&'static str] {
        &[]
    }
    fn release_all(&mut self) -> Result<(), InputError> {
        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct NoopClipboard(String);

impl Clipboard for NoopClipboard {
    fn get(&mut self) -> Result<String, InputError> {
        Ok(self.0.clone())
    }
    fn set(&mut self, s: &str) -> Result<(), InputError> {
        self.0 = s.to_owned();
        Ok(())
    }
}

/// Satu panggilan yang tercatat oleh [`RecordingInput`].
#[derive(Debug, Clone, PartialEq)]
pub enum Call {
    Move(f32, f32),
    MoveAbs(f32, f32),
    Button(Button, bool),
    Scroll(f32, f32, ScrollPhase, bool),
    Key(String, bool),
    Text(String),
    Gesture(String, u8),
    ReleaseAll,
}

/// Mata-mata untuk test: mencatat panggilan ke log bersama yang bisa dibaca dari luar
/// setelah instansinya dipindahkan ke dalam host.
#[derive(Debug, Clone, Default)]
pub struct RecordingInput {
    log: Arc<Mutex<Vec<Call>>>,
}

impl RecordingInput {
    pub fn new() -> Self {
        Self::default()
    }

    /// Pegangan ke log yang sama dengan instans ini.
    pub fn log(&self) -> Arc<Mutex<Vec<Call>>> {
        Arc::clone(&self.log)
    }

    fn push(&self, c: Call) -> Result<(), InputError> {
        self.log.lock().unwrap().push(c);
        Ok(())
    }
}

impl PlatformInput for RecordingInput {
    fn permission(&self) -> PermissionState {
        PermissionState::NotRequired
    }
    fn open_permission_settings(&self) -> Result<(), InputError> {
        Ok(())
    }
    fn move_pointer(&mut self, dx: f32, dy: f32) -> Result<(), InputError> {
        self.push(Call::Move(dx, dy))
    }
    fn move_pointer_abs(&mut self, x: f32, y: f32) -> Result<(), InputError> {
        self.push(Call::MoveAbs(x, y))
    }
    fn button(&mut self, b: Button, down: bool) -> Result<(), InputError> {
        self.push(Call::Button(b, down))
    }
    fn scroll(
        &mut self,
        dx: f32,
        dy: f32,
        phase: ScrollPhase,
        momentum: bool,
    ) -> Result<(), InputError> {
        self.push(Call::Scroll(dx, dy, phase, momentum))
    }
    fn key(&mut self, key: &str, down: bool) -> Result<(), InputError> {
        self.push(Call::Key(key.to_owned(), down))
    }
    fn text(&mut self, s: &str) -> Result<(), InputError> {
        self.push(Call::Text(s.to_owned()))
    }
    fn gesture(&mut self, g: &str, fingers: u8) -> Result<(), InputError> {
        self.push(Call::Gesture(g.to_owned(), fingers))
    }
    fn supported_gestures(&self) -> &'static [&'static str] {
        &[]
    }
    fn release_all(&mut self) -> Result<(), InputError> {
        self.push(Call::ReleaseAll)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recording_input_shares_log_with_handle() {
        let mut input = RecordingInput::new();
        let log = input.log();
        input.move_pointer(1.0, 2.0).unwrap();
        input.release_all().unwrap();
        assert_eq!(
            *log.lock().unwrap(),
            vec![Call::Move(1.0, 2.0), Call::ReleaseAll]
        );
    }
}
