//! Helpers for navigating to / showing the settings window. The actual
//! transparent overlay is rendered natively in [`crate::overlay_window`]
//! via eframe — it is NOT a Tauri WebView window.

use tauri::{Emitter, Manager};

/// Show the settings window and focus the License tab via the
/// `navigate-license` event the frontend listens to. Called after a
/// rejected saved license at startup.
pub fn focus_license_tab(app: &tauri::AppHandle) {
    if let Some(win) = app.get_webview_window("settings") {
        let _ = win.show();
        let _ = win.set_focus();
        let _ = win.unminimize();
        let _ = win.emit("navigate-license", ());
    }
}
