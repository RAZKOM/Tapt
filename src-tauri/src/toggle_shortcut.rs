//! OS-level shortcut to flip [`SharedState::overlay_enabled`], using the same
//! `global-hotkey` string grammar as keyboard capture in the settings UI.

use std::sync::atomic::Ordering;
use std::sync::Arc;

use tauri::AppHandle;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use crate::SharedState;

/// Unregister prior shortcuts owned by our manager, then register the configured
/// toggle (if any). Invalid specs are logged and skipped.
pub fn sync<R: tauri::Runtime>(app: &AppHandle<R>, shared: &Arc<SharedState>, spec: Option<&str>) {
    let gs = app.global_shortcut();

    if let Err(e) = gs.unregister_all() {
        eprintln!("[toggle-hotkey] unregister_all: {e}");
    }

    let Some(raw) = spec.map(str::trim) else {
        return;
    };
    if raw.is_empty() {
        return;
    }

    let sh = shared.clone();
    if let Err(e) = gs.on_shortcut(raw, move |_app, _shortcut, event| {
        if event.state == ShortcutState::Pressed {
            let v = sh.overlay_enabled.load(Ordering::Relaxed);
            sh.overlay_enabled.store(!v, Ordering::Relaxed);
        }
    }) {
        eprintln!("[toggle-hotkey] could not register {raw:?}: {e}");
    }
}
