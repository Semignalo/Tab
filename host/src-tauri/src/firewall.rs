//! Preflight jaringan (langkah 5.2): apakah HP bisa menjangkau host?
//!
//! Di Windows, koneksi masuk diblokir Firewall sampai ada aturan; tanpa itu HP tidak
//! menemukan host dan user tidak tahu kenapa. Pemeriksaan ini bisa diulang dari UI.

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct FirewallStatus {
    /// `false` di OS yang tidak perlu langkah ini (macOS menanyakan sendiri saat pertama kali).
    pub applicable: bool,
    pub ok: bool,
    pub detail: String,
}

pub const TCP_RULE: &str = "Tab TCP 4180";
pub const UDP_RULE: &str = "Tab UDP 4179";

#[cfg(windows)]
pub fn status() -> FirewallStatus {
    let has = |name: &str| {
        std::process::Command::new("netsh")
            .args([
                "advfirewall",
                "firewall",
                "show",
                "rule",
                &format!("name={name}"),
            ])
            .output()
            .map(|o| {
                o.status.success() && !String::from_utf8_lossy(&o.stdout).contains("No rules match")
            })
            .unwrap_or(false)
    };
    let (tcp, udp) = (has(TCP_RULE), has(UDP_RULE));
    FirewallStatus {
        applicable: true,
        ok: tcp && udp,
        detail: match (tcp, udp) {
            (true, true) => "Aturan Firewall untuk jaringan Private sudah ada.".into(),
            (false, false) => {
                "Belum ada aturan Firewall. HP mungkin tidak bisa menemukan komputer ini.".into()
            }
            (false, true) => "Aturan TCP 4180 belum ada.".into(),
            (true, false) => {
                "Aturan UDP 4179 belum ada (pencarian otomatis tidak akan jalan).".into()
            }
        },
    }
}

/// Buat aturan lewat PowerShell yang dinaikkan (UAC). Hanya profil Private dan Domain — jaringan
/// Publik (kafe, bandara) sengaja tidak dibuka.
#[cfg(windows)]
pub fn fix() -> Result<FirewallStatus, String> {
    let add = |name: &str, proto: &str, port: u16| {
        format!(
            "netsh advfirewall firewall add rule name=\"{name}\" dir=in action=allow protocol={proto} localport={port} profile=private,domain"
        )
    };
    let script = format!(
        "{} & {}",
        add(TCP_RULE, "TCP", 4180),
        add(UDP_RULE, "UDP", 4179)
    );
    let arg = format!(
        "Start-Process -FilePath cmd.exe -ArgumentList '/c {}' -Verb RunAs -Wait -WindowStyle Hidden",
        script.replace('\'', "''")
    );
    let out = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", &arg])
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err("Izin administrator dibatalkan.".into());
    }
    Ok(status())
}

#[cfg(not(windows))]
pub fn status() -> FirewallStatus {
    FirewallStatus {
        applicable: false,
        ok: true,
        detail: "Tidak diperlukan di sistem ini.".into(),
    }
}

#[cfg(not(windows))]
pub fn fix() -> Result<FirewallStatus, String> {
    Ok(status())
}
