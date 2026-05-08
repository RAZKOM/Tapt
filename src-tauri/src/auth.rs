//! License lifecycle: AuthForge client wrapper, cryptified Stripe URLs,
//! and the Tauri commands the License tab calls.
//!
//! ## Hardening notes
//!
//! All five sensitive strings (the three AuthForge credentials plus
//! the two Stripe checkout URLs) are wrapped in
//! `cryptify::encrypt_string!` so they never appear contiguously in
//! the binary's `.rdata` / `.rodata` sections. Decryption happens at
//! the call site and the plaintext lives only on the stack for the
//! duration of one operation.
//!
//! [`is_session_valid`] is marked `#[inline(always)]` so the gate
//! disappears into the listener loop bodies — a binary patcher cannot
//! NOP a single `call` instruction to bypass it.
//!
//! ## HWID handling
//!
//! The real `authforge` SDK does not expose its internally-generated
//! HWID. Instead we compute a stable per-host fingerprint locally and
//! pass it to the SDK via `AuthForgeConfig::hwid_override`, then keep
//! a copy for display. The SDK validates the override against the
//! same `/auth/validate` server, so what we display is exactly what
//! the server sees.

use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::sync::OnceLock;

use authforge::{
    AuthForgeClient, AuthForgeConfig, AuthForgeError, HeartbeatMode, LoginResult,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_store::StoreExt;

use crate::config::{Config, CONFIG_STORE_KEY, STORE_FILE};
use crate::{mask_hwid, AuthStatusPayload, LicenseInfo, LicenseType, SharedState};

include!(concat!(env!("OUT_DIR"), "/secrets.rs"));

/// Persisted store key for the active license string.
pub const LICENSE_KEY_STORE_KEY: &str = "license_key";

/// Best-known license end instant from `/auth/validate` (never rolling heartbeat TTL).
///
/// Live [`AuthForgeClient::get_session_data`] `expiresIn` resets to ~24h on `/auth/heartbeat`;
/// expiry UI uses this ceiling merged with explicit license variables instead.
pub const LICENSE_SUBSCRIPTION_CEILING_UNIX_KEY: &str = "license_subscription_period_ceiling_unix";

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
    store.delete(LICENSE_SUBSCRIPTION_CEILING_UNIX_KEY);
    store
        .save()
        .map_err(|e| format!("Could not save store after clearing license key: {e}"))?;
    Ok(())
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

    pub fn client(&self) -> &Arc<AuthForgeClient> {
        &self.0.auth
    }
}

/// The licensing gate. Inlined into `keylistener`
/// so it cannot be NOP'd as a single call site.
#[inline(always)]
pub fn is_session_valid(client: &AuthForgeClient) -> bool {
    client.is_authenticated()
}

/// Overlay + IPC gate: respects [`SharedState::auth_ui_suppressed`] so the UI can
/// flip to logged-out before `AuthForgeClient::logout` finishes joining the
/// heartbeat thread.
#[inline(always)]
pub fn overlay_license_gate(shared: &SharedState) -> bool {
    if shared.auth_ui_suppressed.load(Ordering::Relaxed) {
        return false;
    }
    is_session_valid(&shared.auth)
}

/// Build the AuthForge client with cryptified secrets. Called once
/// from `main.rs` at startup. The closure passed as `on_failure`
/// emits `auth-status-changed` so the frontend can react.
pub fn build_client(app_handle: AppHandle) -> Arc<AuthForgeClient> {
    let app_id = authforge_app_id();
    let app_secret = authforge_app_secret();
    let public_key = authforge_public_key();

    let handle_for_cb = app_handle.clone();
    // `..Default::default()` is preserved here on purpose so the call
    // site remains resilient against future fields added to
    // `AuthForgeConfig` by the upstream SDK.
    #[allow(clippy::needless_update)]
    let client = AuthForgeClient::new(AuthForgeConfig {
        app_id,
        app_secret,
        public_key,
        heartbeat_mode: HeartbeatMode::Server,
        heartbeat_interval: 900,
        hwid_override: Some(local_hwid().to_string()),
        on_failure: Some(Box::new(move |reason: &str| {
            eprintln!("[authforge] heartbeat failure: {reason}");
            handle_for_cb
                .emit(
                    "auth-status-changed",
                    AuthStatusPayload {
                        authenticated: false,
                        reason: reason.to_string(),
                    },
                )
                .ok();
        })),
        ..Default::default()
    });
    Arc::new(client)
}

/// Cryptified Stripe URLs. Call sites decrypt at the moment of use.
pub fn purchase_monthly_url() -> String {
    stripe_monthly_url()
}

pub fn purchase_lifetime_url() -> String {
    stripe_lifetime_url()
}

/// User-facing mapping from `AuthForgeError` to the strings the
/// License tab displays.
pub fn error_to_message(err: &AuthForgeError) -> &'static str {
    match err {
        AuthForgeError::InvalidApp => {
            "App credentials are invalid. Please reinstall."
        }
        AuthForgeError::InvalidKey => "License key not found. Check for typos.",
        AuthForgeError::Expired => "This license has expired. Please renew.",
        AuthForgeError::Revoked => "This license has been revoked.",
        AuthForgeError::HwidMismatch => {
            "This key is already activated on another device."
        }
        AuthForgeError::NoCredits => {
            "License has no remaining activations. Please contact support."
        }
        AuthForgeError::AppBurnCapReached => {
            "Activation limit reached. Please try again later."
        }
        AuthForgeError::Blocked => "This account or device is blocked.",
        AuthForgeError::RateLimited => {
            "Too many attempts. Please wait a moment and try again."
        }
        AuthForgeError::ReplayDetected => {
            "Security check failed (replay detected). Please retry."
        }
        AuthForgeError::AppDisabled => "This application has been disabled.",
        AuthForgeError::SessionExpired => {
            "Your session has expired. Please activate again."
        }
        AuthForgeError::RevokeRequiresSession => {
            "An active session is required to revoke this license."
        }
        AuthForgeError::BadRequest => {
            "License server rejected the request. Please retry."
        }
        AuthForgeError::SystemError => {
            "License server is temporarily unavailable. Please retry."
        }
        AuthForgeError::SignatureMismatch => {
            "Server response failed signature verification."
        }
        AuthForgeError::NetworkError(_) => {
            "Could not reach the license server. Check your internet connection."
        }
        AuthForgeError::Other(_) => "An unexpected licensing error occurred.",
    }
}

/// Stable JSON-serializable error code the frontend can branch on.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LicenseError {
    InvalidApp,
    InvalidKey,
    Expired,
    Revoked,
    HwidMismatch,
    NoCredits,
    AppBurnCapReached,
    Blocked,
    RateLimited,
    ReplayDetected,
    AppDisabled,
    SessionExpired,
    RevokeRequiresSession,
    BadRequest,
    SystemError,
    SignatureMismatch,
    Network,
    Storage,
    Other,
}

impl From<&AuthForgeError> for LicenseError {
    fn from(err: &AuthForgeError) -> Self {
        match err {
            AuthForgeError::InvalidApp => Self::InvalidApp,
            AuthForgeError::InvalidKey => Self::InvalidKey,
            AuthForgeError::Expired => Self::Expired,
            AuthForgeError::Revoked => Self::Revoked,
            AuthForgeError::HwidMismatch => Self::HwidMismatch,
            AuthForgeError::NoCredits => Self::NoCredits,
            AuthForgeError::AppBurnCapReached => Self::AppBurnCapReached,
            AuthForgeError::Blocked => Self::Blocked,
            AuthForgeError::RateLimited => Self::RateLimited,
            AuthForgeError::ReplayDetected => Self::ReplayDetected,
            AuthForgeError::AppDisabled => Self::AppDisabled,
            AuthForgeError::SessionExpired => Self::SessionExpired,
            AuthForgeError::RevokeRequiresSession => Self::RevokeRequiresSession,
            AuthForgeError::BadRequest => Self::BadRequest,
            AuthForgeError::SystemError => Self::SystemError,
            AuthForgeError::SignatureMismatch => Self::SignatureMismatch,
            AuthForgeError::NetworkError(_) => Self::Network,
            AuthForgeError::Other(_) => Self::Other,
        }
    }
}

/// Combined error envelope used as the `Err` payload for license
/// commands. Frontend reads `code` for branching, `message` for
/// display.
#[derive(Debug, Serialize, Deserialize)]
pub struct LicenseFailure {
    pub code: LicenseError,
    pub message: String,
}

impl From<AuthForgeError> for LicenseFailure {
    fn from(err: AuthForgeError) -> Self {
        Self {
            code: (&err).into(),
            message: error_to_message(&err).to_string(),
        }
    }
}

/// Derive the `LicenseType` from server-supplied `license_variables`.
/// Convention: the licensing server sets a `"type"` (preferred) or
/// `"plan"` field on each license to one of `monthly` / `lifetime` /
/// `trial`. If neither is present we fall back to `Unknown`.
fn classify(license_variables: Option<&HashMap<String, Value>>) -> LicenseType {
    let Some(map) = license_variables else {
        return LicenseType::Unknown;
    };
    let raw = map
        .get("type")
        .or_else(|| map.get("plan"))
        .or_else(|| map.get("tier"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    match raw.to_ascii_lowercase().as_str() {
        "monthly" | "subscription" | "subscription_monthly" => LicenseType::Monthly,
        "lifetime" | "perpetual" => LicenseType::Lifetime,
        "trial" => LicenseType::Trial,
        _ => LicenseType::Unknown,
    }
}

/// Canonical keys when reading license end times from AuthForge maps.
///
/// Per [AuthForge variables](https://docs.authforge.cc), prefer **`expiresAt`** on the
/// license (ISO 8601). Session payload `expiresIn` is a **rolling session token** TTL
/// updated on each heartbeat in SERVER mode—not when the licensed term ends.
///
/// Values: Unix seconds, milliseconds, decimal numbers, digit strings, or RFC 3339 /
/// ISO‑8601 datetime strings.
fn subscription_period_end_from_vars(
    vars: Option<&HashMap<String, Value>>,
) -> Option<u64> {
    let map = vars?;

    #[rustfmt::skip]
    let keys: &[&str] = &[
        "expiresAt", "expires_at",
        "subscription_expires_at", "subscriptionExpiresAt",
        "subscription_period_end", "subscriptionPeriodEnd",
        "period_end", "periodEnd",
        "stripe_current_period_end", "stripeCurrentPeriodEnd",
        "current_period_end", "currentPeriodEnd",
        "trial_ends_at", "trialEndsAt",
        "trial_end", "trialEnd",
        "billing_period_end", "billingPeriodEnd",
    ];

    for &k in keys {
        if let Some(v) = map.get(k) {
            if let Some(ts) = json_value_to_epoch_unix(v) {
                return Some(ts);
            }
        }
    }

    None
}

fn json_value_to_epoch_unix(v: &Value) -> Option<u64> {
    match v {
        Value::Number(n) => {
            if let Some(u) = n.as_u64() {
                return Some(normalize_epoch_units(u));
            }
            let f = n.as_f64()?;
            (f.is_finite() && f >= 0.0).then(|| normalize_epoch_units(f as u64))
        }
        Value::String(s) => string_to_epoch_unix(s),
        _ => None,
    }
}

fn string_to_epoch_unix(raw: &str) -> Option<u64> {
    let t = raw.trim();
    if t.is_empty() {
        return None;
    }
    if let Ok(u) = t.parse::<u64>() {
        return Some(normalize_epoch_units(u));
    }

    use time::format_description::well_known::Rfc3339;
    use time::OffsetDateTime;

    if let Ok(dt) = OffsetDateTime::parse(t, &Rfc3339) {
        return Some(dt.unix_timestamp().max(0) as u64);
    }

    // Minimal offset forms used by some gateways (e.g. `...T00:00:00+00:00`)
    const ISO8601_Z: &[time::format_description::FormatItem<'_>] = time::macros::format_description!(
        "[year]-[month]-[day]T[hour]:[minute]:[second]Z"
    );
    if let Ok(dt) = time::PrimitiveDateTime::parse(t, &ISO8601_Z)
        .map(|p| p.assume_utc())
    {
        return Some(dt.unix_timestamp().max(0) as u64);
    }

    None
}

/// Interpret values above ~year 2286 in seconds as milliseconds.
#[inline]
fn normalize_epoch_units(raw: u64) -> u64 {
    const SECOND_CEILING_MS: u64 = 10_000_000_000;
    if raw > SECOND_CEILING_MS {
        raw / 1000
    } else {
        raw
    }
}

/// Top-level **`licenseExpiresAt`** on signed validate/heartbeat JSON (camelCase or snake_case).
/// JSON **`null`** means lifetime (returns `None` here). Missing key ⇒ `None`.
fn license_expires_at_from_signed_session(client: &AuthForgeClient) -> Option<u64> {
    let session = client.get_session_data()?;
    let v = session
        .get("licenseExpiresAt")
        .or_else(|| session.get("license_expires_at"))?;
    if v.is_null() {
        return None;
    }
    json_value_to_epoch_unix(v)
}

fn persist_subscription_ceiling_merge(app: &AppHandle, candidates: &[u64]) -> Result<(), String> {
    if candidates.is_empty() {
        return Ok(());
    }
    let mut best = *candidates.iter().max().expect("non-empty slice");
    let store = app.store(STORE_FILE).map_err(|e| e.to_string())?;
    if let Some(prev) = store
        .get(LICENSE_SUBSCRIPTION_CEILING_UNIX_KEY)
        .and_then(|v| v.as_u64())
    {
        best = best.max(prev);
    }
    store.set(
        LICENSE_SUBSCRIPTION_CEILING_UNIX_KEY,
        serde_json::Value::Number(best.into()),
    );
    store.save().map_err(|e| e.to_string())
}

/// Persist the farthest license end instant we know for expiry UI (`max(previous, candidates…)`).
///
/// Uses **`licenseExpiresAt`** on the signed payload (decoded session JSON — same JSON the SDK stores
/// in [`AuthForgeClient::get_session_data`]), plus hints from `license_variables` / `app_variables`
/// on the [`LoginResult`]. **`expiresIn` session TTL is deliberately excluded**: it rolls with SERVER
/// heartbeats and is not the license term end.
pub fn ingest_subscription_horizon_after_login_success(
    app: &AppHandle,
    login: &LoginResult,
    client: &AuthForgeClient,
) -> Result<(), String> {
    let mut candidates: Vec<u64> = Vec::new();
    if let Some(v) = subscription_period_end_from_vars(login.license_variables.as_ref()) {
        candidates.push(v);
    }
    if let Some(v) = subscription_period_end_from_vars(login.app_variables.as_ref()) {
        candidates.push(v);
    }
    if let Some(v) = license_expires_at_from_signed_session(client) {
        candidates.push(v);
    }
    if candidates.is_empty() {
        return Ok(());
    }
    persist_subscription_ceiling_merge(app, &candidates)
}

fn load_subscription_ceiling(app: &AppHandle) -> Option<u64> {
    let store = app.store(STORE_FILE).ok()?;
    store
        .get(LICENSE_SUBSCRIPTION_CEILING_UNIX_KEY)
        .and_then(|v| v.as_u64())
}

fn subscription_period_display_unix(
    app: &AppHandle,
    license_type: LicenseType,
    client: &AuthForgeClient,
) -> Option<u64> {
    if matches!(license_type, LicenseType::Lifetime) {
        return None;
    }
    let explicit = subscription_period_end_from_vars(client.get_license_variables().as_ref())
        .or_else(|| subscription_period_end_from_vars(client.get_app_variables().as_ref()));
    let from_signed = license_expires_at_from_signed_session(client);
    let ceiling = load_subscription_ceiling(app);

    if let Some(v) = explicit {
        let _ = persist_subscription_ceiling_merge(app, &[v]);
    }
    if let Some(v) = from_signed {
        let _ = persist_subscription_ceiling_merge(app, &[v]);
    }

    explicit.or(from_signed).or(ceiling)
}

#[tauri::command]
pub async fn activate_license(
    key: String,
    state: State<'_, AuthState>,
    app: AppHandle,
) -> Result<LicenseInfo, LicenseFailure> {
    let _auth_transition = state.shared().auth_transition_lock.lock();
    state
        .shared()
        .auth_ui_suppressed
        .store(false, Ordering::SeqCst);

    let client = state.client().clone();
    let key = normalize_license_key(&key);
    if key.len() < 8 {
        return Err(LicenseFailure {
            code: LicenseError::InvalidKey,
            message: "Enter a complete license key.".into(),
        });
    }

    // Clear any prior session/heartbeat so a new key cannot inherit stale
    // `license_variables` from the previous activation in the same process.
    client.logout();

    match client.login(&key) {
        Ok(result) => {
            let store = app.store(STORE_FILE).map_err(|e| LicenseFailure {
                code: LicenseError::Storage,
                message: format!("Could not persist license: {e}"),
            })?;
            store.set(LICENSE_KEY_STORE_KEY, serde_json::Value::String(key));
            store.save().map_err(|e| LicenseFailure {
                code: LicenseError::Storage,
                message: format!("Could not save store: {e}"),
            })?;

            app.emit(
                "auth-status-changed",
                AuthStatusPayload {
                    authenticated: true,
                    reason: String::new(),
                },
            )
            .ok();

            ingest_subscription_horizon_after_login_success(&app, &result, state.client()).map_err(
                |e| LicenseFailure {
                    code: LicenseError::Storage,
                    message: format!("Could not save license expiry: {e}"),
                },
            )?;

            let license_type = classify(result.license_variables.as_ref());
            let subscription_period_end_unix = subscription_period_display_unix(
                &app,
                license_type,
                state.client(),
            );
            Ok(LicenseInfo {
                license_type,
                subscription_period_end_unix,
                hwid_masked: mask_hwid(local_hwid()),
            })
        }
        Err(err) => Err(err.into()),
    }
}

#[tauri::command]
pub async fn deactivate_license(
    state: State<'_, AuthState>,
    app: AppHandle,
) -> Result<(), String> {
    let _auth_transition = state.shared().auth_transition_lock.lock();

    clear_persisted_license_key(&app)?;

    state
        .shared()
        .auth_ui_suppressed
        .store(true, Ordering::SeqCst);

    app.emit(
        "auth-status-changed",
        AuthStatusPayload {
            authenticated: false,
            reason: "deactivated".into(),
        },
    )
    .ok();

    state.client().logout();

    state
        .shared()
        .auth_ui_suppressed
        .store(false, Ordering::SeqCst);

    Ok(())
}

#[tauri::command]
pub fn get_auth_status(state: State<'_, AuthState>) -> AuthStatusPayload {
    AuthStatusPayload {
        authenticated: overlay_license_gate(state.shared().as_ref()),
        reason: String::new(),
    }
}

#[tauri::command]
pub fn get_license_info(app: AppHandle, state: State<'_, AuthState>) -> Option<LicenseInfo> {
    let shared = state.shared();
    if shared.auth_ui_suppressed.load(Ordering::SeqCst) {
        return None;
    }
    let client = state.client();
    if !client.is_authenticated() {
        return None;
    }

    let license_vars = client.get_license_variables();
    let license_type = classify(license_vars.as_ref());

    let subscription_period_end_unix =
        subscription_period_display_unix(&app, license_type, client);

    Some(LicenseInfo {
        license_type,
        subscription_period_end_unix,
        hwid_masked: mask_hwid(local_hwid()),
    })
}

#[tauri::command]
pub fn get_hwid_masked(_state: State<'_, AuthState>) -> String {
    mask_hwid(local_hwid())
}

#[tauri::command]
pub fn get_purchase_urls() -> PurchaseUrls {
    PurchaseUrls {
        monthly: purchase_monthly_url(),
        lifetime: purchase_lifetime_url(),
        authforge_app_id: authforge_app_id(),
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PurchaseUrls {
    pub monthly: String,
    pub lifetime: String,
    /// Public AuthForge application id (safe to show in support UI).
    pub authforge_app_id: String,
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

/// Compute a stable per-(host, user, OS) HWID. Hashed with SHA-256 so
/// it never leaks identifying material directly. We pass this to the
/// SDK via `hwid_override` and display its first 8 chars (masked) in
/// the License tab.
fn compute_hwid() -> String {
    let host = std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "unknown-host".into());
    let user = std::env::var("USERNAME")
        .or_else(|_| std::env::var("USER"))
        .unwrap_or_else(|_| "unknown-user".into());
    let os = std::env::consts::OS;

    let material = format!("{host}|{user}|{os}");
    let mut hasher = Sha256::new();
    hasher.update(material.as_bytes());
    let digest = hasher.finalize();

    let mut s = String::with_capacity(digest.len() * 2);
    for b in digest.iter() {
        use std::fmt::Write;
        let _ = write!(&mut s, "{b:02x}");
    }
    s
}
