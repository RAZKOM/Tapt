//! License lifecycle: the AuthForge client, the cryptified Stripe URL,
//! and the Tauri commands the License tab calls.
//!
//! ## Licensing model
//!
//! Every license is a perpetual license activated **offline, once**:
//! [`activate_license`] calls `activate_offline`, which exchanges the key
//! for a signed, lifetime `.authforge` file bound to this PC's HWID and
//! writes it to `<app data>/license.authforge`. Every later launch verifies
//! that file locally with `login_from_file`, with no network
//! ([`restore_license`]). There is no online login and no heartbeat.
//!
//! ## Hardening notes
//!
//! All four sensitive strings (the three AuthForge credentials plus
//! the Stripe checkout URL) are wrapped in `cryptify::encrypt_string!`
//! so they never appear contiguously in the binary's `.rdata` /
//! `.rodata` sections. Decryption happens at the call site and the
//! plaintext lives only on the stack for the duration of one operation.
//!
//! [`is_session_valid`], [`is_licensed`] and [`overlay_license_gate`] are
//! marked `#[inline(always)]` so the gate disappears into the listener
//! loop bodies — a binary patcher cannot NOP a single `call` instruction
//! to bypass it.
//!
//! ## HWID handling
//!
//! The HWID is `SHA-256("tapt|" + MachineGuid)`, passed to the SDK via
//! `AuthForgeConfig::hwid_override` so what we display is exactly what the
//! server binds. Offline files only verify on the HWID they were issued
//! for, and every new HWID uses one of the license's machine slots for
//! good, so this value must stay stable: do not change the scheme.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::OnceLock;

use authforge::{AuthForgeClient, AuthForgeConfig, AuthForgeError};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_store::StoreExt;

use crate::config::{Config, CONFIG_STORE_KEY, STORE_FILE};
use crate::{mask_hwid, AuthStatusPayload, LicenseInfo, SharedState, UpdateStatus};

include!(concat!(env!("OUT_DIR"), "/secrets.rs"));

/// Persisted store key for the active license string. Kept after offline
/// activation: it is what makes re-activation after a deleted file or a
/// reinstall silent (same PC re-activating is free).
pub const LICENSE_KEY_STORE_KEY: &str = "license_key";

/// File name of the signed offline license inside the app data directory.
pub const LICENSE_FILE_NAME: &str = "license.authforge";

/// `<app data dir>/license.authforge` (`%APPDATA%\com.razkom.tapt\`).
/// Never the install directory.
pub fn license_file_path(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map(|dir| dir.join(LICENSE_FILE_NAME))
        .map_err(|e| e.to_string())
}

/// Match the settings UI: uppercase, strip whitespace, keep `A-Z0-9-` only.
pub fn normalize_license_key(key: &str) -> String {
    key.chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
        .collect::<String>()
        .to_uppercase()
}

/// Remove the persisted license key from `config.json`.
///
/// Uses the plugin store's delete + save so the in-memory cache matches disk.
/// Manually editing the file then reloading raced the store's debounced autosave:
/// a pending save could write an older cache (still holding `license_key`) after
/// the key was removed from disk. On exit the plugin saves every store again,
/// which could resurrect the license.
pub fn clear_persisted_license_key(app: &AppHandle) -> Result<(), String> {
    let store = app.store(STORE_FILE).map_err(|e| e.to_string())?;
    store.delete(LICENSE_KEY_STORE_KEY);
    store
        .save()
        .map_err(|e| format!("Could not save store after clearing license key: {e}"))?;
    Ok(())
}

/// Returns a normalized license key when one is persisted and long enough.
pub fn saved_license_key(app: &AppHandle) -> Option<String> {
    let key = normalize_license_key(&saved_license_key_raw(app)?);
    if key.len() < 8 {
        return None;
    }
    Some(key)
}

fn saved_license_key_raw(app: &AppHandle) -> Option<String> {
    app.store(STORE_FILE)
        .ok()?
        .get(LICENSE_KEY_STORE_KEY)
        .and_then(|v| v.as_str().map(|s| s.to_string()))
}

fn save_license_key(app: &AppHandle, key: &str) -> Result<(), String> {
    let store = app.store(STORE_FILE).map_err(|e| e.to_string())?;
    store.set(LICENSE_KEY_STORE_KEY, Value::String(key.to_string()));
    store.save().map_err(|e| e.to_string())
}

/// The HWID we compute locally and feed into the SDK via
/// `hwid_override`. Cached in a `OnceLock` so every call site
/// (`build_client`, `get_hwid_masked`, `get_license_info`) sees the
/// exact same value.
static LOCAL_HWID: OnceLock<String> = OnceLock::new();

fn local_hwid() -> &'static str {
    LOCAL_HWID.get_or_init(compute_hwid)
}

/// Tauri-managed wrapper around the cross-thread `SharedState`. The
/// settings UI's commands read it through `State<'_, AuthState>`; the
/// overlay thread reads the same `Arc<SharedState>` directly.
pub struct AuthState(pub Arc<SharedState>);

impl AuthState {
    pub fn new(shared: Arc<SharedState>) -> Self {
        Self(shared)
    }

    pub fn shared(&self) -> &Arc<SharedState> {
        &self.0
    }
}

/// The client's licensing check. Inlined into every gate.
#[inline(always)]
pub fn is_session_valid(client: &AuthForgeClient) -> bool {
    client.is_authenticated()
}

/// Licensed by a verified offline license file.
#[inline(always)]
pub fn is_licensed(shared: &SharedState) -> bool {
    is_session_valid(&shared.auth)
}

/// Overlay + key listener gate: licensed, or free-trial time left.
#[inline(always)]
pub fn overlay_license_gate(shared: &SharedState) -> bool {
    is_licensed(shared) || crate::trial::is_trial_active()
}

/// Offline client: `activate_offline` + `login_from_file`. No heartbeat;
/// nothing touches the network after activation.
pub fn build_client() -> Arc<AuthForgeClient> {
    Arc::new(AuthForgeClient::new(AuthForgeConfig {
        app_id: authforge_app_id(),
        app_secret: authforge_app_secret(),
        public_key: authforge_public_key(),
        hwid_override: Some(local_hwid().to_string()),
        // `login_from_file` reports a missing or bad file here as
        // `offline_login_failed: <code>`. That only means "activate from
        // the saved key or use the trial", never an error or a logout.
        on_failure: Some(Box::new(|reason: &str| {
            eprintln!("[authforge] {reason}");
        })),
        ..Default::default()
    }))
}

/// Cryptified Stripe URL. Decrypted at the moment of use.
pub fn purchase_lifetime_url() -> String {
    stripe_lifetime_url()
}

const MSG_UNREACHABLE: &str = "Couldn't reach the activation server. Try again.";

/// Where buyers get help. The settings UI turns this address into a mailto
/// link wherever it appears in a message; keep it in sync with
/// `SUPPORT_EMAIL` in `src/branding.ts`.
pub const SUPPORT_EMAIL: &str = "hello@razkom.com";

/// User-facing message for an activation error. Keyed on `err.code()`
/// because the offline-activation codes arrive as
/// `AuthForgeError::Other(code)`.
pub fn error_to_message(err: &AuthForgeError) -> String {
    let code = err.code();
    let msg = match code {
        "invalid_key" => "License key not found. Check for typos.",
        "revoked" | "expired" => "This license is no longer valid.",
        "offline_activation_limit_reached" => {
            "This key has been activated on the maximum number of PCs. \
             Email hello@razkom.com for help."
        }
        "hwid_mismatch" => {
            "No free activation slot for this PC. Email hello@razkom.com for help."
        }
        "offline_activation_disabled"
        | "offline_activation_requires_seats"
        | "offline_activation_requires_perpetual" => {
            "Activation isn't available for this license. Email hello@razkom.com for help."
        }
        "no_credits" | "app_burn_cap_reached" | "rate_limited" | "system_error"
        | "network_error" | "timeout" | "invalid_json_response" | "unexpected_response" => {
            MSG_UNREACHABLE
        }
        c if c.starts_with("http_error_") => MSG_UNREACHABLE,
        "offline_file_rejected" => "Activation failed. Email hello@razkom.com for help.",
        "file_write_failed" => {
            let full = err.to_string();
            let reason = full.split_once(':').map_or(full.as_str(), |(_, r)| r.trim());
            return format!("Couldn't save the license file: {reason}");
        }
        "invalid_app" => "This copy of Tapt can't activate licenses. Reinstall it from razkom.com/tapt.",
        "blocked" => "This PC can't activate Tapt. Email hello@razkom.com for help.",
        "app_disabled" => {
            "Activation isn't available right now. Email hello@razkom.com for help."
        }
        "replay_detected" | "bad_request" | "signature_mismatch" => "Activation failed. Try again.",
        _ => "Activation failed. Try again, or email hello@razkom.com if it keeps happening.",
    };
    msg.to_string()
}

/// Stable JSON-serializable error code the frontend can branch on.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LicenseError {
    InvalidApp,
    InvalidKey,
    Expired,
    Revoked,
    /// No free activation slot for this PC.
    HwidMismatch,
    /// Every machine slot on the license is used (support only).
    OfflineActivationLimitReached,
    /// Disabled for the app, unlimited-seat or non-perpetual license.
    OfflineActivationUnavailable,
    OfflineFileRejected,
    FileWriteFailed,
    NoCredits,
    AppBurnCapReached,
    Blocked,
    RateLimited,
    ReplayDetected,
    AppDisabled,
    BadRequest,
    SystemError,
    SignatureMismatch,
    Network,
    Storage,
    Other,
}

impl From<&AuthForgeError> for LicenseError {
    fn from(err: &AuthForgeError) -> Self {
        match err.code() {
            "invalid_app" => Self::InvalidApp,
            "invalid_key" => Self::InvalidKey,
            "expired" => Self::Expired,
            "revoked" => Self::Revoked,
            "hwid_mismatch" => Self::HwidMismatch,
            "offline_activation_limit_reached" => Self::OfflineActivationLimitReached,
            "offline_activation_disabled"
            | "offline_activation_requires_seats"
            | "offline_activation_requires_perpetual" => Self::OfflineActivationUnavailable,
            "offline_file_rejected" => Self::OfflineFileRejected,
            "file_write_failed" => Self::FileWriteFailed,
            "no_credits" => Self::NoCredits,
            "app_burn_cap_reached" => Self::AppBurnCapReached,
            "blocked" => Self::Blocked,
            "rate_limited" => Self::RateLimited,
            "replay_detected" => Self::ReplayDetected,
            "app_disabled" => Self::AppDisabled,
            "bad_request" => Self::BadRequest,
            "system_error" => Self::SystemError,
            "signature_mismatch" => Self::SignatureMismatch,
            "network_error" | "timeout" | "invalid_json_response" | "unexpected_response" => {
                Self::Network
            }
            c if c.starts_with("http_error_") => Self::Network,
            _ => Self::Other,
        }
    }
}

/// Combined error envelope used as the `Err` payload for license
/// commands. Frontend reads `code` for branching, `message` for
/// display.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseFailure {
    pub code: LicenseError,
    pub message: String,
}

impl From<AuthForgeError> for LicenseFailure {
    fn from(err: AuthForgeError) -> Self {
        Self {
            code: (&err).into(),
            message: error_to_message(&err),
        }
    }
}

impl LicenseFailure {
    fn storage(message: String) -> Self {
        Self {
            code: LicenseError::Storage,
            message,
        }
    }
}

/// Log an activation error. The full `Display` form carries the detail
/// support needs (for example `offline_file_rejected: wrong_app`, which
/// usually means a wrong AUTHFORGE_PUBLIC_KEY or app id in the build).
fn log_activation_error(context: &str, err: &AuthForgeError) {
    eprintln!("[license] {context} failed (code {}): {err}", err.code());
}

/// The key itself is dead: forget it so we stop retrying it every launch.
/// Everything else (network, no free slot, ...) keeps it for a silent retry.
fn is_dead_key(err: &AuthForgeError) -> bool {
    matches!(err.code(), "invalid_key" | "revoked" | "expired")
}

/// Result of [`restore_license`] at startup.
pub enum StartupLicense {
    Licensed,
    /// No file and no saved key: the trial applies.
    Unlicensed,
    /// A saved key could not be activated; show this on the License tab.
    Failed(LicenseFailure),
}

/// Startup licensing, in order:
/// 1. A valid `license.authforge` file: licensed, no network.
/// 2. A saved key (the file was deleted, or the app reinstalled):
///    `activate_offline` once. The same PC gets its file back for free.
/// 3. Otherwise unlicensed; the trial applies.
pub fn restore_license(app: &AppHandle, shared: &SharedState) -> StartupLicense {
    let _auth_transition = shared.auth_transition_lock.lock();

    let path = match license_file_path(app) {
        Ok(path) => Some(path),
        Err(e) => {
            eprintln!("[license] no app data dir: {e}");
            None
        }
    };

    if let Some(path_str) = path
        .as_deref()
        .filter(|p| p.is_file())
        .and_then(|p| p.to_str())
    {
        if shared.auth.login_from_file(path_str).is_ok() {
            return StartupLicense::Licensed;
        }
    }

    let Some(key) = saved_license_key(app) else {
        if saved_license_key_raw(app).is_some() {
            let _ = clear_persisted_license_key(app);
        }
        return StartupLicense::Unlicensed;
    };
    if saved_license_key_raw(app).as_deref() != Some(key.as_str()) {
        if let Err(e) = save_license_key(app, &key) {
            eprintln!("[license] could not persist normalized license key: {e}");
        }
    }

    let Some(path) = path else {
        return StartupLicense::Failed(LicenseFailure::storage(
            "Couldn't save the license on this PC.".into(),
        ));
    };

    match shared.auth.activate_offline(&key, Some(&path)) {
        Ok(()) => StartupLicense::Licensed,
        Err(err) => {
            log_activation_error("activation with the saved key", &err);
            if is_dead_key(&err) {
                let _ = clear_persisted_license_key(app);
            }
            StartupLicense::Failed(err.into())
        }
    }
}

fn current_license_info(shared: &SharedState) -> Option<LicenseInfo> {
    is_licensed(shared).then(|| LicenseInfo {
        hwid_masked: mask_hwid(local_hwid()),
    })
}

#[tauri::command]
pub async fn activate_license(
    key: String,
    state: State<'_, AuthState>,
    app: AppHandle,
) -> Result<LicenseInfo, LicenseFailure> {
    let shared = state.shared();
    let _auth_transition = shared.auth_transition_lock.lock();

    let key = normalize_license_key(&key);
    if key.len() < 8 {
        return Err(LicenseFailure {
            code: LicenseError::InvalidKey,
            message: "Enter a complete license key.".into(),
        });
    }

    let path = license_file_path(&app).map_err(|e| {
        eprintln!("[license] no app data dir: {e}");
        LicenseFailure::storage("Couldn't save the license on this PC.".into())
    })?;

    // On error nothing is written and the session is untouched; the user
    // can fix the problem and press Activate again.
    if let Err(err) = shared.auth.activate_offline(&key, Some(&path)) {
        log_activation_error("activation", &err);
        return Err(err.into());
    }

    // The file is already written, so the license works regardless; the
    // saved key only makes a later re-activation silent.
    if let Err(e) = save_license_key(&app, &key) {
        eprintln!("[license] could not persist license key: {e}");
    }
    *shared.license_error.lock() = None;

    app.emit(
        "auth-status-changed",
        AuthStatusPayload {
            authenticated: true,
            reason: String::new(),
            error: None,
        },
    )
    .ok();

    current_license_info(shared).ok_or_else(|| LicenseFailure {
        code: LicenseError::Other,
        message: "Activation succeeded, but the license could not be read. Restart Tapt.".into(),
    })
}

/// "Remove license from this PC": delete the license file and the saved key
/// and log out. This does NOT free an activation slot; re-activating this
/// same PC later is free.
#[tauri::command]
pub async fn deactivate_license(
    state: State<'_, AuthState>,
    app: AppHandle,
) -> Result<(), String> {
    let shared = state.shared();
    let _auth_transition = shared.auth_transition_lock.lock();

    let path = license_file_path(&app).map_err(|e| {
        eprintln!("[license] no app data dir: {e}");
        "Couldn't remove the license from this PC. Try again.".to_string()
    })?;
    match std::fs::remove_file(&path) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => {
            eprintln!("[license] could not delete license file: {e}");
            return Err("Couldn't remove the license from this PC. Try again.".into());
        }
    }
    clear_persisted_license_key(&app).map_err(|e| {
        eprintln!("[license] could not clear saved key: {e}");
        "Couldn't remove the license from this PC. Try again.".to_string()
    })?;
    *shared.license_error.lock() = None;

    shared.auth.logout();

    app.emit(
        "auth-status-changed",
        AuthStatusPayload {
            authenticated: false,
            reason: "deactivated".into(),
            error: None,
        },
    )
    .ok();

    Ok(())
}

#[tauri::command]
pub fn get_auth_status(state: State<'_, AuthState>) -> AuthStatusPayload {
    let shared = state.shared();
    if shared
        .startup_auth_pending
        .load(std::sync::atomic::Ordering::SeqCst)
    {
        return AuthStatusPayload {
            authenticated: false,
            reason: "checking".into(),
            error: None,
        };
    }
    AuthStatusPayload {
        authenticated: is_licensed(shared.as_ref()),
        reason: String::new(),
        error: shared.license_error.lock().clone(),
    }
}

#[tauri::command]
pub fn get_license_info(state: State<'_, AuthState>) -> Option<LicenseInfo> {
    current_license_info(state.shared())
}

#[tauri::command]
pub fn get_hwid_masked(_state: State<'_, AuthState>) -> String {
    mask_hwid(local_hwid())
}

/// Independent of licensing and the trial (see `version.rs`).
#[tauri::command]
pub fn get_update_status() -> UpdateStatus {
    crate::version::current_status()
}

#[tauri::command]
pub fn get_purchase_urls() -> PurchaseUrls {
    PurchaseUrls {
        lifetime: purchase_lifetime_url(),
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PurchaseUrls {
    pub lifetime: String,
}

// ============================================================================
// Config commands
//
// Single source of truth = `SharedState::config` (a parking_lot::RwLock).
// `tauri-plugin-store` is just the on-disk serialization. The eframe overlay
// thread reads `SharedState::config` on every frame, so any `set_config`
// call is visible to the overlay within ~16ms.
// ============================================================================

/// Read the live config from the in-memory shared state. Falls back to
/// `Config::default()` if the lock is poisoned (which can't happen with
/// parking_lot, but the pattern keeps the command infallible).
#[tauri::command]
pub fn get_config(state: State<'_, AuthState>) -> Config {
    state.shared().config.read().clone()
}

/// Replace the live config and persist to the tauri-plugin-store on disk.
/// The eframe overlay picks up the change on its next frame.
#[tauri::command]
pub fn set_config(
    config: Config,
    state: State<'_, AuthState>,
    app: AppHandle,
) -> Result<(), String> {
    *state.shared().config.write() = config.clone();

    let store = app.store(STORE_FILE).map_err(|e| e.to_string())?;
    store.set(
        CONFIG_STORE_KEY,
        serde_json::to_value(&config).map_err(|e| e.to_string())?,
    );
    store.save().map_err(|e| e.to_string())?;

    let hotkey = config.behavior.global_toggle_hotkey.clone();
    crate::toggle_shortcut::sync(&app, state.shared(), hotkey.as_deref());

    Ok(())
}

/// `SHA-256("tapt|" + MachineGuid)`. Set once by Windows setup, the same
/// for every user on the PC, and not spoofable through env vars. Falls
/// back to the old env-var scheme only if the registry can't be read.
/// We pass this to the SDK via `hwid_override` and display its last 4
/// chars (masked) in the License tab.
fn compute_hwid() -> String {
    match machine_guid() {
        Some(guid) => sha256_hex(&format!("tapt|{guid}")),
        None => {
            eprintln!("[hwid] could not read MachineGuid; using the legacy env-var HWID");
            legacy_env_hwid()
        }
    }
}

#[cfg(windows)]
fn machine_guid() -> Option<String> {
    use crate::win_registry::{read_string, Hive};
    let guid = read_string(
        Hive::LocalMachine,
        r"SOFTWARE\Microsoft\Cryptography",
        "MachineGuid",
    )?;
    let guid = guid.trim().to_ascii_lowercase();
    (!guid.is_empty()).then_some(guid)
}

#[cfg(not(windows))]
fn machine_guid() -> Option<String> {
    None
}

/// Pre-offline-activation HWID: `SHA-256(host|user|os)` from env vars.
fn legacy_env_hwid() -> String {
    let host = std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "unknown-host".into());
    let user = std::env::var("USERNAME")
        .or_else(|_| std::env::var("USER"))
        .unwrap_or_else(|_| "unknown-user".into());
    let os = std::env::consts::OS;
    sha256_hex(&format!("{host}|{user}|{os}"))
}

fn sha256_hex(material: &str) -> String {
    let digest = Sha256::digest(material.as_bytes());
    let mut s = String::with_capacity(digest.len() * 2);
    for b in digest.iter() {
        use std::fmt::Write;
        let _ = write!(&mut s, "{b:02x}");
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message(err: AuthForgeError) -> String {
        error_to_message(&err)
    }

    #[test]
    fn offline_activation_codes_map_to_messages() {
        assert_eq!(
            message(AuthForgeError::InvalidKey),
            "License key not found. Check for typos."
        );
        assert_eq!(message(AuthForgeError::Revoked), "This license is no longer valid.");
        assert!(message(AuthForgeError::Other("offline_activation_limit_reached".into()))
            .contains("maximum number of PCs"));
        assert_eq!(
            message(AuthForgeError::HwidMismatch),
            "No free activation slot for this PC. Email hello@razkom.com for help."
        );
        assert!(message(AuthForgeError::Other("offline_activation_requires_seats".into()))
            .starts_with("Activation isn't available"));
        assert_eq!(message(AuthForgeError::NoCredits), MSG_UNREACHABLE);
        assert_eq!(message(AuthForgeError::NetworkError("timeout: x".into())), MSG_UNREACHABLE);
        assert_eq!(
            message(AuthForgeError::Other("offline_file_rejected: wrong_app".into())),
            "Activation failed. Email hello@razkom.com for help."
        );
        assert_eq!(
            message(AuthForgeError::Other("file_write_failed: access denied".into())),
            "Couldn't save the license file: access denied"
        );
    }

    /// Every "get help" message names the support address.
    #[test]
    fn help_messages_name_support_email() {
        for code in [
            "offline_activation_limit_reached",
            "hwid_mismatch",
            "offline_activation_disabled",
            "offline_activation_requires_seats",
            "offline_activation_requires_perpetual",
            "offline_file_rejected",
            "blocked",
            "app_disabled",
            "some_unknown_code",
        ] {
            let msg = message(AuthForgeError::Other(code.into()));
            assert!(msg.contains(SUPPORT_EMAIL), "{code}: {msg}");
            assert!(!msg.to_lowercase().contains("contact support"), "{code}: {msg}");
        }
    }

    #[test]
    fn license_error_codes_from_other() {
        assert!(matches!(
            LicenseError::from(&AuthForgeError::Other("offline_activation_limit_reached".into())),
            LicenseError::OfflineActivationLimitReached
        ));
        assert!(matches!(
            LicenseError::from(&AuthForgeError::Other("http_error_502".into())),
            LicenseError::Network
        ));
    }

    #[cfg(windows)]
    #[test]
    fn hwid_uses_machine_guid() {
        let guid = machine_guid().expect("MachineGuid readable on Windows");
        assert_eq!(compute_hwid(), sha256_hex(&format!("tapt|{guid}")));
        assert_eq!(compute_hwid().len(), 64);
    }
}
