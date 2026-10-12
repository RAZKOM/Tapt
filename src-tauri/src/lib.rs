//! Shared types and re-exports.
//!
//! This module is the single source of truth for serde payloads that
//! cross the Tauri IPC boundary. The frontend mirrors them in
//! `src/types.ts`.

pub mod auth;
pub mod config;
pub mod key_display;
pub mod keylistener;
pub mod listener;
pub mod monitors;
pub mod overlay;
pub mod overlay_window;
pub mod toggle_shortcut;
pub mod trial;
pub mod version;
#[cfg(windows)]
pub mod win_registry;

use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use authforge::AuthForgeClient;
use parking_lot::{Mutex, RwLock};
use serde::{Deserialize, Serialize};

use crate::auth::LicenseFailure;
use crate::config::Config;

/// Pushed on the `auth-status-changed` event whenever the session
/// transitions between licensed and unlicensed. `authenticated` means a
/// real license; the free trial is reported separately (`trial.rs`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthStatusPayload {
    pub authenticated: bool,
    pub reason: String,
    /// Why activating the saved key at startup failed, for the License
    /// tab. `None` when there is nothing to show.
    pub error: Option<LicenseFailure>,
}

pub use version::UpdateStatus;

/// Returned by `activate_license` / `get_license_info` while licensed.
/// Every license is a lifetime license activated offline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseInfo {
    /// Last 4 chars of the locally-computed HWID, masked as `••••XXXX`.
    pub hwid_masked: String,
}

/// Pushed on the `key-down` and `key-up` events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyEvent {
    /// Physical key code, e.g. "KeyS", "ShiftLeft", "Space".
    pub code: String,
    /// Display label, e.g. "S", "Shift", "Space".
    pub label: String,
    pub is_modifier: bool,
    /// Unix milliseconds.
    pub timestamp: u64,
}

/// Input events sent from the rdev listener thread to the eframe overlay
/// thread via a `crossbeam_channel`. Replaces the previous `app.emit()`
/// path now that the overlay no longer runs inside a webview.
#[derive(Debug, Clone)]
pub enum InputEvent {
    KeyDown(KeyEvent),
    KeyUp(KeyEvent),
}

/// Cross-thread shared state. Owned as `Arc<SharedState>` and read by
/// both the eframe overlay thread (each frame) and the rdev listeners.
///
/// We deliberately reuse the same `Arc<AuthForgeClient>` for the overlay
/// gate rather than caching `is_authenticated` into an atomic: the
/// `#[inline(always)]` `is_session_valid` checker is part of the binary-
/// hardening spec and inlines into every call site, so a binary patcher
/// cannot NOP a single instruction to bypass the gate.
pub struct SharedState {
    /// Offline client: `activate_offline` / `login_from_file`, no network
    /// after activation, no heartbeat.
    pub auth: Arc<AuthForgeClient>,
    /// Live user config, kept in sync with the persisted store.
    pub config: RwLock<Config>,
    /// User-controlled toggle (tray menu "Toggle Overlay"). When `false`
    /// the eframe overlay renders nothing even if authenticated. The
    /// auth gate still applies — both must be true to draw anything.
    pub overlay_enabled: AtomicBool,
    /// Serializes startup restore / activate / remove so they never race.
    pub auth_transition_lock: Mutex<()>,
    /// True while `main::startup_sequence` is verifying the license file or
    /// saved key.
    pub startup_auth_pending: AtomicBool,
    /// Last license failure to show on the License tab (cleared on success
    /// and on "Remove license from this PC").
    pub license_error: Mutex<Option<LicenseFailure>>,
}

impl SharedState {
    pub fn new(auth: Arc<AuthForgeClient>, config: Config) -> Self {
        Self {
            auth,
            config: RwLock::new(config),
            overlay_enabled: AtomicBool::new(true),
            auth_transition_lock: Mutex::new(()),
            startup_auth_pending: AtomicBool::new(false),
            license_error: Mutex::new(None),
        }
    }
}

/// Mask a HWID for UI display: `••••XXXX` (last 4 chars only).
pub fn mask_hwid(hwid: &str) -> String {
    let last4 = hwid
        .chars()
        .rev()
        .take(4)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<String>();
    format!("••••{last4}")
}
