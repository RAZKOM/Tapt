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
pub mod version;

use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use authforge::AuthForgeClient;
use parking_lot::{Mutex, RwLock};
use serde::{Deserialize, Serialize};

use crate::config::Config;

/// Pushed on the `auth-status-changed` event whenever the session
/// transitions between authenticated and unauthenticated.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthStatusPayload {
    pub authenticated: bool,
    pub reason: String,
}

pub use version::UpdateStatus;

/// License classification used by the License tab UI. The real
/// AuthForge SDK doesn't categorize licenses itself, so we derive
/// this from `LoginResult::license_variables` (server-controlled key
/// `"type"` or `"plan"` taking values like `"monthly"`, `"lifetime"`,
/// `"trial"`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LicenseType {
    Monthly,
    Lifetime,
    Trial,
    #[default]
    Unknown,
}

/// Returned by the `activate_license` Tauri command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseInfo {
    pub license_type: LicenseType,
    /// Unix instant (seconds) for license-expiry UI: explicit period fields in
    /// `license_variables` / `app_variables`, else top-level signed payload
    /// `licenseExpiresAt`, else a persisted ceiling merged at login. **`expiresIn` is
    /// session-only** and is not used here.
    pub subscription_period_end_unix: Option<u64>,
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
    pub auth: Arc<AuthForgeClient>,
    /// Live user config, kept in sync with the persisted store.
    pub config: RwLock<Config>,
    /// User-controlled toggle (tray menu "Toggle Overlay"). When `false`
    /// the eframe overlay renders nothing even if authenticated. The
    /// auth gate still applies — both must be true to draw anything.
    pub overlay_enabled: AtomicBool,
    /// Serialized activate/deactivate so `logout()` never races `login()`.
    pub auth_transition_lock: Mutex<()>,
    /// While true, UI and overlay treat the session as logged out even if
    /// [`AuthForgeClient`] has not finished `logout()` yet — lets us emit
    /// `auth-status-changed` before the blocking heartbeat join.
    pub auth_ui_suppressed: AtomicBool,
    /// True while `main::startup_sequence` is verifying a saved license key.
    pub startup_auth_pending: AtomicBool,
}

impl SharedState {
    pub fn new(auth: Arc<AuthForgeClient>, config: Config) -> Self {
        Self {
            auth,
            config: RwLock::new(config),
            overlay_enabled: AtomicBool::new(true),
            auth_transition_lock: Mutex::new(()),
            auth_ui_suppressed: AtomicBool::new(false),
            startup_auth_pending: AtomicBool::new(false),
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
