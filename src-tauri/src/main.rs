// Hide the console window on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::atomic::Ordering;
use std::sync::Arc;

use tapt_lib::auth::{
    self, build_client, AuthState, LicenseFailure, StartupLicense,
};
use tapt_lib::config::{Config, CONFIG_STORE_KEY, STORE_FILE};
use tapt_lib::keylistener::start_keylistener;
use tapt_lib::overlay::focus_license_tab;
use tapt_lib::overlay_window;
use tapt_lib::{trial, version};
use tapt_lib::{AuthStatusPayload, InputEvent, SharedState};

use tauri::image::Image;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton as TrayMouseButton, MouseButtonState, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, RunEvent};
use tauri_plugin_store::StoreExt;

/// `icons/32x32.png` embedded at compile time; reapplied in setup for tray and settings window icons.
const SETTINGS_BRAND_ICON_PNG: &[u8] = include_bytes!("../icons/32x32.png");

fn brand_png_icon() -> Result<Image<'static>, String> {
    Image::from_bytes(SETTINGS_BRAND_ICON_PNG).map_err(|e| e.to_string())
}

#[cfg(windows)]
fn init_windows_process_dpi() {
    use windows_sys::Win32::UI::HiDpi::{
        SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
    };
    // Match Win32 monitor rects (`rcMonitor`) and `SetWindowPos` to physical pixels on each display.
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
}

fn main() {
    #[cfg(windows)]
    init_windows_process_dpi();

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(win) = app.get_webview_window("settings") {
                let _ = win.show();
                let _ = win.set_focus();
            }
        }))
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(
            tauri_plugin_autostart::Builder::new()
                .args(["--autostart"])
                .build(),
        )
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            auth::activate_license,
            auth::deactivate_license,
            auth::get_auth_status,
            auth::get_license_info,
            auth::get_hwid_masked,
            auth::get_purchase_urls,
            auth::get_update_status,
            auth::get_config,
            auth::set_config,
            trial::get_trial_status,
            commands::show_settings,
            commands::toggle_overlay,
            commands::quit_app,
            tapt_lib::monitors::list_monitors,
        ])
        .setup(|app| {
            let app_handle: AppHandle = app.handle().clone();

            // 1. Load persisted config from disk (or default).
            let config = load_config(&app_handle);
            let open_settings_on_startup = config.behavior.open_settings_on_startup;
            let toggle_hotkey = config.behavior.global_toggle_hotkey.clone();

            // 2. Initialize the offline AuthForge client.
            let client = build_client();

            // 3. Build cross-thread SharedState owning both.
            let shared = Arc::new(SharedState::new(client, config));

            // 4. Tauri-managed AuthState wraps the same Arc — used by the
            //    license / config commands the settings UI invokes.
            app.manage(AuthState::new(shared.clone()));

            // Trial time must be loaded before the overlay gate first runs.
            trial::load(&app_handle);

            let has_license_file = auth::license_file_path(&app_handle)
                .is_ok_and(|p| p.is_file());
            if has_license_file || auth::saved_license_key(&app_handle).is_some() {
                shared
                    .startup_auth_pending
                    .store(true, Ordering::SeqCst);
                emit_auth_status(&app_handle, false, "checking", None);
            }

            // 5. Build tray icon menu (toggle overlay, settings, quit).
            let brand_icon = match brand_png_icon() {
                Ok(img) => Some(img),
                Err(e) => {
                    log::warn!("could not decode icons/32x32.png (run npm run icons:generate): {e}");
                    None
                }
            };
            if let Some(win) = app.get_webview_window("settings") {
                if let Err(e) = win.set_resizable(true) {
                    log::warn!("could not enable settings window resize: {e}");
                }
                if let Some(ref icon) = brand_icon {
                    if let Err(e) = win.set_icon(icon.clone()) {
                        log::warn!("could not set settings window icon: {e}");
                    }
                }
                if open_settings_on_startup {
                    let _ = win.show();
                    let _ = win.set_focus();
                }
            }
            build_tray(app, shared.clone(), brand_icon)?;

            // 6. Keyboard hook → overlay channel (`rdev::listen`; see `listener.rs`).
            let (event_tx, event_rx) = crossbeam_channel::unbounded::<InputEvent>();
            start_keylistener(shared.clone(), event_tx);

            tapt_lib::toggle_shortcut::sync(
                &app_handle,
                &shared,
                toggle_hotkey.as_deref(),
            );

            // Trial clock: counts while the overlay is enabled and the app is
            // not licensed (and not while the saved license is being checked).
            let shared_for_trial = shared.clone();
            trial::start_ticker(app_handle.clone(), move || {
                shared_for_trial.overlay_enabled.load(Ordering::Relaxed)
                    && !shared_for_trial.startup_auth_pending.load(Ordering::SeqCst)
                    && !auth::is_licensed(&shared_for_trial)
            });

            version::spawn_update_checker(app_handle.clone());

            // 7. Spawn the eframe overlay on a dedicated thread. winit's
            //    event loop runs there independently of Tauri's tao loop
            //    (which keeps the main thread). On Windows each event
            //    loop has its own Win32 message pump tied to its creator
            //    thread, so the two coexist cleanly.
            let overlay_state = shared.clone();
            std::thread::Builder::new()
                .name("kv-overlay".into())
                .spawn(move || {
                    if let Err(e) = overlay_window::run_overlay(overlay_state, event_rx) {
                        eprintln!("[overlay] {e}");
                    }
                })
                .expect("could not spawn overlay thread");

            // 8. Run the controlled startup auth sequence asynchronously
            //    so we don't block setup().
            let app_for_startup = app_handle.clone();
            let shared_for_startup = shared.clone();
            tauri::async_runtime::spawn(async move {
                startup_sequence(app_for_startup, shared_for_startup).await;
            });

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            if let RunEvent::Exit = event {
                trial::persist(app);
            }
        });
}

/// Read the persisted config from `tauri-plugin-store`, falling back to
/// `Config::default()` on first run or any read / parse error.
fn load_config(app: &AppHandle) -> Config {
    let Ok(store) = app.store(STORE_FILE) else {
        return Config::default();
    };
    let Some(raw) = store.get(CONFIG_STORE_KEY) else {
        return Config::default();
    };
    serde_json::from_value::<Config>(raw).unwrap_or_default()
}

/// Controlled startup licensing (see [`auth::restore_license`]): verify the
/// license file, else activate from the saved key, else leave the trial in
/// charge. Emits `auth-status-changed` and opens the License tab when there
/// is an error to show or nothing (license or trial) left to run on. The
/// eframe overlay polls `overlay_license_gate` every frame, so we don't
/// need to "start" listeners here.
async fn startup_sequence(app: AppHandle, shared: Arc<SharedState>) {
    let (licensed, reason, error) = match auth::restore_license(&app, &shared) {
        StartupLicense::Licensed => (true, String::new(), None),
        StartupLicense::Unlicensed => (false, "no-license".to_string(), None),
        StartupLicense::Failed(failure) => {
            let reason = serde_json::to_value(failure.code)
                .ok()
                .and_then(|v| v.as_str().map(str::to_string))
                .unwrap_or_default();
            (false, reason, Some(failure))
        }
    };

    *shared.license_error.lock() = error.clone();
    shared.startup_auth_pending.store(false, Ordering::SeqCst);
    let show_license_tab = error.is_some() || (!licensed && !trial::is_trial_active());
    emit_auth_status(&app, licensed, &reason, error);
    if show_license_tab {
        focus_license_tab(&app);
    }
}

fn emit_auth_status(
    app: &AppHandle,
    authenticated: bool,
    reason: &str,
    error: Option<LicenseFailure>,
) {
    let _ = app.emit(
        "auth-status-changed",
        AuthStatusPayload {
            authenticated,
            reason: reason.to_string(),
            error,
        },
    );
}

fn build_tray(
    app: &mut tauri::App,
    shared: Arc<SharedState>,
    tray_icon: Option<Image<'static>>,
) -> tauri::Result<()> {
    let toggle_overlay =
        MenuItem::with_id(app, "toggle-overlay", "Show or hide overlay", true, None::<&str>)?;
    let open_settings =
        MenuItem::with_id(app, "open-settings", "Settings", true, None::<&str>)?;
    let separator = tauri::menu::PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Tapt", true, None::<&str>)?;

    let menu = Menu::with_items(app, &[&toggle_overlay, &open_settings, &separator, &quit])?;

    if let Some(tray) = app.tray_by_id("main-tray") {
        if let Some(icon) = tray_icon {
            if let Err(e) = tray.set_icon(Some(icon)) {
                log::warn!("could not set tray icon from icons/32x32.png: {e}");
            }
        }
        tray.set_menu(Some(menu))?;
        let shared_for_menu = shared.clone();
        tray.on_menu_event(move |app, event| match event.id.as_ref() {
            "toggle-overlay" => {
                let prev = shared_for_menu.overlay_enabled.load(Ordering::Relaxed);
                shared_for_menu
                    .overlay_enabled
                    .store(!prev, Ordering::Relaxed);
            }
            "open-settings" => {
                if let Some(win) = app.get_webview_window("settings") {
                    let _ = win.show();
                    let _ = win.set_focus();
                }
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        });
        tray.on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: TrayMouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(win) = app.get_webview_window("settings") {
                    let _ = win.show();
                    let _ = win.set_focus();
                }
            }
        });
    }

    Ok(())
}

/// Tauri commands that don't belong to license management.
mod commands {
    use std::sync::atomic::Ordering;

    use tauri::{AppHandle, Manager, State};

    use tapt_lib::auth::AuthState;

    #[tauri::command]
    pub fn show_settings(app: AppHandle) -> Result<(), String> {
        if let Some(win) = app.get_webview_window("settings") {
            win.show().map_err(|e| e.to_string())?;
            win.set_focus().map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    /// Flip `SharedState.overlay_enabled`. The eframe overlay reads it on
    /// the next frame and renders empty if `false`.
    #[tauri::command]
    pub fn toggle_overlay(state: State<'_, AuthState>) -> bool {
        let prev = state.shared().overlay_enabled.load(Ordering::Relaxed);
        let next = !prev;
        state.shared().overlay_enabled.store(next, Ordering::Relaxed);
        next
    }

    #[tauri::command]
    pub fn quit_app(app: AppHandle) {
        app.exit(0);
    }
}
