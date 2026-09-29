//! Eksekusi aksi di mesin host.

use crate::{Action, ActionError, ActionRunner};
use std::process::{Command, Stdio};
use std::time::Duration;
use tab_input::PlatformInput;

/// Skema URL yang boleh dibuka. `file:`/`javascript:`/skema kustom sengaja tidak masuk:
/// aksi deck tidak boleh jadi jalan pintas menjalankan sesuatu yang tidak dimaksud user.
const URL_SCHEMES: [&str; 3] = ["http://", "https://", "mailto:"];
const MAX_DEPTH: usize = 4;

pub struct SystemRunner {
    input: Box<dyn PlatformInput>,
}

impl SystemRunner {
    pub fn new(input: Box<dyn PlatformInput>) -> Self {
        Self { input }
    }

    fn run_depth(&mut self, action: &Action, depth: usize) -> Result<(), ActionError> {
        if depth > MAX_DEPTH {
            return Err(ActionError::Invalid("Multi terlalu dalam".into()));
        }
        match action {
            Action::Hotkey { keys } => self.hotkey(keys),
            Action::MediaKey { key } => self.tap(key),
            Action::LaunchApp { path } => spawn_detached(path),
            Action::OpenPath { path } => open_target(path),
            Action::OpenUrl { url } => {
                let lower = url.to_ascii_lowercase();
                if !URL_SCHEMES.iter().any(|s| lower.starts_with(s)) {
                    return Err(ActionError::Invalid(
                        "hanya http, https, dan mailto yang boleh dibuka".into(),
                    ));
                }
                open_target(url)
            }
            Action::ObsScene { .. } => Err(ActionError::Unsupported(
                "OBS tidak tersambung".into(),
            )),
            Action::Multi { steps } => {
                for (i, step) in steps.iter().enumerate() {
                    if i > 0 {
                        std::thread::sleep(Duration::from_millis(60));
                    }
                    // Berhenti di langkah pertama yang gagal: melanjutkan urutan yang sudah
                    // pincang lebih berbahaya daripada berhenti.
                    self.run_depth(step, depth + 1)?;
                }
                Ok(())
            }
        }
    }

    fn tap(&mut self, key: &str) -> Result<(), ActionError> {
        self.input.key(key, true).map_err(fail)?;
        self.input.key(key, false).map_err(fail)
    }

    fn hotkey(&mut self, keys: &[String]) -> Result<(), ActionError> {
        if keys.is_empty() {
            return Err(ActionError::Invalid("hotkey kosong".into()));
        }
        let mut pressed: Vec<&str> = Vec::new();
        let mut result = Ok(());
        for k in keys {
            if let Err(e) = self.input.key(k, true) {
                result = Err(fail(e));
                break;
            }
            pressed.push(k);
        }
        // Lepas semua yang sempat ditekan, urutan terbalik, walau ada yang gagal.
        for k in pressed.iter().rev() {
            let _ = self.input.key(k, false);
        }
        result
    }
}

fn fail(e: tab_input::InputError) -> ActionError {
    ActionError::Failed(e.to_string())
}

impl ActionRunner for SystemRunner {
    fn run(&mut self, action: &Action) -> Result<(), ActionError> {
        self.run_depth(action, 0)
    }
}

fn spawn_detached(path: &str) -> Result<(), ActionError> {
    Command::new(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|e| ActionError::Failed(format!("{path}: {e}")))
}

/// Buka file/folder/URL dengan aplikasi bawaan OS, tanpa shell — argumen tidak pernah
/// ditafsirkan sebagai perintah.
fn open_target(target: &str) -> Result<(), ActionError> {
    let mut cmd;
    #[cfg(windows)]
    {
        cmd = Command::new("rundll32.exe");
        cmd.args(["url.dll,FileProtocolHandler", target]);
    }
    #[cfg(target_os = "macos")]
    {
        cmd = Command::new("open");
        cmd.arg(target);
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        cmd = Command::new("xdg-open");
        cmd.arg(target);
    }
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|e| ActionError::Failed(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tab_input::{Call, RecordingInput};

    fn runner() -> (SystemRunner, std::sync::Arc<std::sync::Mutex<Vec<Call>>>) {
        let input = RecordingInput::new();
        let log = input.log();
        (SystemRunner::new(Box::new(input)), log)
    }

    #[test]
    fn hotkey_presses_in_order_and_releases_in_reverse() {
        let (mut r, log) = runner();
        r.run(&Action::Hotkey {
            keys: vec!["ctrl".into(), "shift".into(), "p".into()],
        })
        .unwrap();
        let got: Vec<_> = log
            .lock()
            .unwrap()
            .iter()
            .map(|c| match c {
                Call::Key(k, d) => format!("{k}{}", if *d { "↓" } else { "↑" }),
                other => format!("{other:?}"),
            })
            .collect();
        assert_eq!(got, ["ctrl↓", "shift↓", "p↓", "p↑", "shift↑", "ctrl↑"]);
    }

    #[test]
    fn non_web_urls_are_refused() {
        let (mut r, _) = runner();
        for bad in ["file:///C:/Windows/System32/calc.exe", "javascript:alert(1)", "calc.exe"] {
            assert!(
                matches!(
                    r.run(&Action::OpenUrl { url: bad.into() }),
                    Err(ActionError::Invalid(_))
                ),
                "{bad} harus ditolak"
            );
        }
    }

    #[test]
    fn multi_stops_at_first_failure() {
        let (mut r, log) = runner();
        let res = r.run(&Action::Multi {
            steps: vec![
                Action::ObsScene { scene: "x".into() },
                Action::MediaKey { key: "mute".into() },
            ],
        });
        assert!(res.is_err());
        assert!(log.lock().unwrap().is_empty(), "langkah kedua tidak boleh jalan");
    }

    #[test]
    fn multi_depth_is_bounded() {
        let (mut r, _) = runner();
        let mut a = Action::MediaKey { key: "mute".into() };
        for _ in 0..(MAX_DEPTH + 2) {
            a = Action::Multi { steps: vec![a] };
        }
        assert!(matches!(r.run(&a), Err(ActionError::Invalid(_))));
    }
}
