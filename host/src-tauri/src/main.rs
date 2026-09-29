//! Shell desktop Tab: ikon tray, jendela pairing, daftar perangkat, setelan, dan preflight.
//!
//! Seluruh logika jaringan ada di crate `tab-host`; berkas ini hanya menempelkannya ke
//! Tauri (command + event) dan tray.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod firewall;
mod settings;

use serde::Serialize;
use serde_json::json;
use settings::Settings;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Mutex;
use tab_deck::{system_runner, JsonProfileStore};
use tab_host::{HostConfig, HostDeps, HostEvent, HostHandle};
use tab_protocol::message::InputSettings;
use tab_protocol::noise::hex;
use tab_protocol::Id16;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, State, WindowEvent};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};

struct AppState {
    host: tokio::sync::RwLock<Option<HostHandle>>,
    start_error: Mutex<Option<String>>,
    connected: Mutex<HashSet<Id16>>,
    settings: Mutex<Settings>,
    settings_path: PathBuf,
    config_dir: PathBuf,
    obs: std::sync::Arc<tab_deck::ObsHandle>,
}

// ------------------------------------------------------------------- DTO

#[derive(Serialize)]
struct StatusDto {
    running: bool,
    error: Option<String>,
    name: String,
    os: String,
    version: String,
    host_id: String,
    fingerprint: String,
    session_port: u16,
    discovery_port: u16,
    addresses: Vec<String>,
    modes: Vec<String>,
    input_permission: &'static str,
}

#[derive(Serialize)]
struct DeviceDto {
    id: String,
    name: String,
    platform: String,
    paired_at: u64,
    last_seen: u64,
    online: bool,
}

#[derive(Serialize)]
struct PinDto {
    pin: String,
    ttl_secs: u64,
}

// -------------------------------------------------------------- commands

#[tauri::command]
async fn status(state: State<'_, AppState>) -> Result<StatusDto, String> {
    let guard = state.host.read().await;
    let error = state.start_error.lock().unwrap().clone();
    let permission = match tab_input::platform_input().permission() {
        tab_input::PermissionState::Granted => "granted",
        tab_input::PermissionState::Denied => "denied",
        tab_input::PermissionState::NotRequired => "not_required",
    };
    let addresses = if_addrs::get_if_addrs()
        .map(|list| {
            list.into_iter()
                .filter(|i| !i.is_loopback())
                .filter_map(|i| match i.addr {
                    if_addrs::IfAddr::V4(v4) if !v4.ip.is_link_local() => Some(v4.ip.to_string()),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default();

    let cfg = HostConfig::detect();
    Ok(match guard.as_ref() {
        Some(h) => StatusDto {
            running: true,
            error: None,
            name: cfg.name,
            os: cfg.os,
            version: cfg.app_version,
            host_id: h.host_id().to_hex(),
            fingerprint: hex(&h.fingerprint())[..16].to_owned(),
            session_port: h.session_addr().port(),
            discovery_port: h.discovery_addr().port(),
            addresses,
            modes: h
                .capabilities()
                .modes
                .iter()
                .map(|m| format!("{m:?}"))
                .collect(),
            input_permission: permission,
        },
        None => StatusDto {
            running: false,
            error,
            name: cfg.name,
            os: cfg.os,
            version: cfg.app_version,
            host_id: String::new(),
            fingerprint: String::new(),
            session_port: 0,
            discovery_port: 0,
            addresses,
            modes: vec![],
            input_permission: permission,
        },
    })
}

#[tauri::command]
async fn devices(state: State<'_, AppState>) -> Result<Vec<DeviceDto>, String> {
    let guard = state.host.read().await;
    let host = guard.as_ref().ok_or("host belum berjalan")?;
    let online = state.connected.lock().unwrap().clone();
    let mut list: Vec<DeviceDto> = host
        .devices()
        .await
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|d| DeviceDto {
            id: d.device.to_hex(),
            online: online.contains(&d.device),
            name: d.name,
            platform: d.platform,
            paired_at: d.paired_at,
            last_seen: d.last_seen,
        })
        .collect();
    list.sort_by(|a, b| b.online.cmp(&a.online).then(b.last_seen.cmp(&a.last_seen)));
    Ok(list)
}

#[tauri::command]
async fn begin_pairing(state: State<'_, AppState>) -> Result<PinDto, String> {
    let guard = state.host.read().await;
    let host = guard.as_ref().ok_or("host belum berjalan")?;
    let t = host.begin_pairing().await.map_err(|e| e.to_string())?;
    Ok(PinDto {
        pin: t.pin,
        ttl_secs: t.ttl.as_secs(),
    })
}

#[tauri::command]
async fn cancel_pairing(state: State<'_, AppState>) -> Result<(), String> {
    let guard = state.host.read().await;
    let host = guard.as_ref().ok_or("host belum berjalan")?;
    host.cancel_pairing().await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn revoke(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let device = Id16::from_hex(&id).ok_or("id perangkat tidak sah")?;
    let guard = state.host.read().await;
    let host = guard.as_ref().ok_or("host belum berjalan")?;
    host.revoke(device).await.map_err(|e| e.to_string())
}

#[tauri::command]
fn get_settings(state: State<'_, AppState>) -> Settings {
    state.settings.lock().unwrap().clone()
}

#[tauri::command]
async fn save_settings(state: State<'_, AppState>, new: Settings) -> Result<Settings, String> {
    let new = new.sanitized();
    if let Some(dir) = new.lyrics_path() {
        if !dir.is_dir() {
            return Err(format!("Folder lirik tidak ditemukan: {}", dir.display()));
        }
    }
    new.save(&state.settings_path).map_err(|e| e.to_string())?;
    *state.settings.lock().unwrap() = new.clone();
    apply_obs(&state);
    if let Some(h) = state.host.read().await.as_ref() {
        h.set_input_settings(InputSettings {
            sens: new.sens,
            natural: new.natural,
        });
        h.set_lyrics_dir(new.lyrics_path());
    }
    Ok(new)
}

#[tauri::command]
fn deck_list(state: State<'_, AppState>) -> Result<Vec<tab_deck::Profile>, String> {
    use tab_deck::ProfileStore;
    JsonProfileStore::open(state.config_dir.join("profiles"))
        .and_then(|s| s.list())
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn deck_save(state: State<'_, AppState>, profile: tab_deck::Profile) -> Result<(), String> {
    use tab_deck::ProfileStore;
    // `save` memvalidasi id, ukuran grid, dan referensi aksi sebelum menulis berkas.
    JsonProfileStore::open(state.config_dir.join("profiles"))
        .and_then(|mut s| s.save(&profile))
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn deck_delete(state: State<'_, AppState>, id: String) -> Result<(), String> {
    use tab_deck::ProfileStore;
    let mut store =
        JsonProfileStore::open(state.config_dir.join("profiles")).map_err(|e| e.to_string())?;
    if store.list().map_err(|e| e.to_string())?.len() <= 1 {
        return Err("Harus ada minimal satu profil.".into());
    }
    store.delete(&id).map_err(|e| e.to_string())
}

/// Terapkan setelan OBS ke pegangan bersama; password dibaca dari keyring.
fn apply_obs(state: &AppState) {
    let s = state.settings.lock().unwrap().clone();
    state
        .obs
        .configure(s.obs_enabled.then(|| tab_deck::ObsConfig {
            host: s.obs_host,
            port: s.obs_port,
            password: tab_host::secret_get("obs-password").unwrap_or_default(),
        }));
}

/// Simpan (atau hapus bila kosong) password OBS di keyring, lalu terapkan.
#[tauri::command]
fn obs_set_password(state: State<'_, AppState>, password: String) -> Result<(), String> {
    let value = (!password.is_empty()).then_some(password.as_str());
    tab_host::secret_set("obs-password", value).map_err(|e| e.to_string())?;
    apply_obs(&state);
    Ok(())
}

/// Uji koneksi ke OBS dengan setelan tersimpan.
#[tauri::command]
async fn obs_test(state: State<'_, AppState>) -> Result<String, String> {
    if !state.obs.is_configured() {
        return Err("OBS belum diaktifkan di setelan.".into());
    }
    let obs = state.obs.clone();
    tauri::async_runtime::spawn_blocking(move || {
        if obs.available() {
            Ok("Tersambung ke OBS.".to_owned())
        } else {
            Err("OBS tidak menjawab. Pastikan OBS berjalan, WebSocket Server aktif (Tools → WebSocket Server Settings), dan password benar.".to_owned())
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
fn autostart_enabled(app: AppHandle) -> bool {
    app.autolaunch().is_enabled().unwrap_or(false)
}

#[tauri::command]
fn set_autostart(app: AppHandle, enabled: bool) -> Result<bool, String> {
    let m = app.autolaunch();
    if enabled {
        m.enable().map_err(|e| e.to_string())?;
    } else {
        m.disable().map_err(|e| e.to_string())?;
    }
    Ok(m.is_enabled().unwrap_or(enabled))
}

#[tauri::command]
fn firewall_status() -> firewall::FirewallStatus {
    firewall::status()
}

#[tauri::command]
async fn firewall_fix() -> Result<firewall::FirewallStatus, String> {
    tauri::async_runtime::spawn_blocking(firewall::fix)
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
fn open_permission_settings() -> Result<(), String> {
    tab_input::platform_input()
        .open_permission_settings()
        .map_err(|e| e.to_string())
}

// ------------------------------------------------------------ host + event

#[cfg(any(windows, target_os = "macos"))]
fn token_store() -> Box<dyn tab_host::TokenStore> {
    Box::new(tab_host::KeyringTokenStore::new())
}

#[cfg(not(any(windows, target_os = "macos")))]
fn token_store() -> Box<dyn tab_host::TokenStore> {
    Box::new(tab_host::MemoryTokenStore::new())
}

async fn start_host(app: AppHandle) {
    let state = app.state::<AppState>();
    let settings = state.settings.lock().unwrap().clone();

    let mut cfg = HostConfig::detect();
    cfg.os_version = sysinfo::System::os_version().unwrap_or_else(|| "unknown".into());
    cfg.lyrics_dir = settings.lyrics_path();

    let result = async {
        let profiles = JsonProfileStore::open(state.config_dir.join("profiles"))
            .map_err(|e| format!("folder profil deck: {e}"))?;
        let deps = HostDeps {
            input: tab_input::platform_input(),
            clipboard: Box::new(tab_input::SystemClipboard::new()),
            metrics: Box::new(tab_metrics::SysMetrics::new()),
            media: tab_media::platform_media(),
            deck: Box::new(profiles),
            runner: Box::new(
                system_runner(tab_input::platform_input()).with_obs(state.obs.clone()),
            ),
            store: token_store(),
            obs: state.obs.clone(),
        };
        HostHandle::start(cfg, deps)
            .await
            .map_err(|e| format!("{e} — apakah Tab lain sudah berjalan?"))
    }
    .await;

    match result {
        Ok(host) => {
            host.set_input_settings(InputSettings {
                sens: settings.sens,
                natural: settings.natural,
            });
            let mut rx = host.events();
            *state.host.write().await = Some(host);
            let app2 = app.clone();
            tauri::async_runtime::spawn(async move {
                while let Ok(ev) = rx.recv().await {
                    forward(&app2, ev);
                }
            });
            let _ = app.emit("host-event", json!({ "kind": "started" }));
        }
        Err(e) => {
            *state.start_error.lock().unwrap() = Some(e.clone());
            let _ = app.emit("host-event", json!({ "kind": "start_failed", "error": e }));
        }
    }
}

fn forward(app: &AppHandle, ev: HostEvent) {
    let state = app.state::<AppState>();
    let payload = match ev {
        HostEvent::PairingStarted { pin, ttl_ms } => {
            json!({ "kind": "pairing_started", "pin": pin, "ttl_ms": ttl_ms })
        }
        HostEvent::PairingSucceeded { device, name } => {
            json!({ "kind": "pairing_succeeded", "id": device.to_hex(), "name": name })
        }
        HostEvent::PairingFailed {
            reason,
            attempts_left,
        } => json!({ "kind": "pairing_failed", "reason": reason, "attempts_left": attempts_left }),
        HostEvent::PairingEnded => json!({ "kind": "pairing_ended" }),
        HostEvent::DeviceConnected { device, name } => {
            state.connected.lock().unwrap().insert(device);
            json!({ "kind": "device_connected", "id": device.to_hex(), "name": name })
        }
        HostEvent::DeviceDisconnected { device } => {
            state.connected.lock().unwrap().remove(&device);
            json!({ "kind": "device_disconnected", "id": device.to_hex() })
        }
        HostEvent::DeviceRevoked { device } => {
            json!({ "kind": "device_revoked", "id": device.to_hex() })
        }
        HostEvent::ModeChanged { device, mode } => {
            json!({ "kind": "mode_changed", "id": device.to_hex(), "mode": format!("{mode:?}") })
        }
    };
    let _ = app.emit("host-event", payload);
}

// ------------------------------------------------------------------- tray

fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

fn build_tray(app: &tauri::App) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Buka Tab", true, None::<&str>)?;
    let pair = MenuItem::with_id(app, "pair", "Pair perangkat baru", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Keluar", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &pair, &quit])?;

    TrayIconBuilder::new()
        .icon(app.default_window_icon().expect("ikon aplikasi").clone())
        .tooltip("Tab")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show_main(app),
            "pair" => {
                show_main(app);
                let _ = app.emit("host-event", json!({ "kind": "request_pairing" }));
            }
            "quit" => {
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    // Lepas semua sesi dengan sopan (tombol yang tertahan dilepas) sebelum keluar.
                    let host = app.state::<AppState>().host.write().await.take();
                    if let Some(h) = host {
                        let _ = h.shutdown().await;
                    }
                    app.exit(0);
                });
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

fn main() {
    tauri::Builder::default()
        // Satu instance saja: dua host akan berebut port 4179/4180.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_main(app)
        }))
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec!["--hidden"]),
        ))
        .setup(|app| {
            let config_dir = app.path().app_config_dir()?;
            let settings_path = config_dir.join("settings.json");
            let settings = Settings::load(&settings_path);
            app.manage(AppState {
                host: tokio::sync::RwLock::new(None),
                start_error: Mutex::new(None),
                connected: Mutex::new(HashSet::new()),
                settings: Mutex::new(settings),
                settings_path,
                config_dir,
                obs: std::sync::Arc::new(tab_deck::ObsHandle::new()),
            });
            apply_obs(&app.state::<AppState>());

            build_tray(app)?;
            if std::env::args().any(|a| a == "--hidden") {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.hide();
                }
            }
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(start_host(handle));
            Ok(())
        })
        // Menutup jendela = sembunyikan ke tray; host tetap melayani perangkat.
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .invoke_handler(tauri::generate_handler![
            status,
            devices,
            begin_pairing,
            cancel_pairing,
            revoke,
            get_settings,
            save_settings,
            deck_list,
            deck_save,
            deck_delete,
            obs_set_password,
            obs_test,
            autostart_enabled,
            set_autostart,
            firewall_status,
            firewall_fix,
            open_permission_settings,
        ])
        .run(tauri::generate_context!())
        .expect("gagal menjalankan Tab");
}
