//! Built-in free trial: one hour of cumulative use.
//!
//! "Use" is time the app runs with the overlay enabled while unlicensed.
//! The ticker measures it with [`Instant`] (monotonic), so moving the
//! system clock neither adds nor removes trial time.
//!
//! Used seconds are persisted in two places, the plugin store and an
//! `HKCU` registry value, every [`PERSIST_EVERY_SECS`] and on exit. On load
//! the larger of the two wins, and the in-memory value only ever grows.
//!
//! Everything trial-related lives in this module. The rest of the app only
//! uses [`load`], [`start_ticker`], [`persist`], [`is_trial_active`],
//! [`trial_remaining_secs`] and the `get_trial_status` command.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use tauri_plugin_store::StoreExt;

use crate::config::STORE_FILE;

/// Total trial length in seconds.
pub const TRIAL_SECS: u64 = 60 * 60;

/// How often used seconds are written to disk while the trial is counting.
const PERSIST_EVERY_SECS: u64 = 30;

const TICK: Duration = Duration::from_secs(1);

/// A gap this long between ticks means the PC slept or the thread stalled;
/// count it as a single tick instead of the whole gap.
const MAX_TICK: Duration = Duration::from_secs(5);

const STORE_KEY: &str = "trial_used_secs";

#[cfg(windows)]
const REG_SUBKEY: &str = r"Software\Razkom\Tapt";
#[cfg(windows)]
const REG_VALUE: &str = "TrialUsedSeconds";

/// Seconds of trial used. Starts exhausted so nothing runs on the trial
/// before [`load`] has read the persisted value.
static USED_SECS: AtomicU64 = AtomicU64::new(TRIAL_SECS);
static LOADED: AtomicBool = AtomicBool::new(false);

/// Pushed on `trial-status-changed` and returned by `get_trial_status`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrialStatus {
    pub remaining_secs: u64,
    pub total_secs: u64,
    pub active: bool,
}

#[inline(always)]
pub fn trial_remaining_secs() -> u64 {
    TRIAL_SECS.saturating_sub(USED_SECS.load(Ordering::Relaxed))
}

#[inline(always)]
pub fn is_trial_active() -> bool {
    trial_remaining_secs() > 0
}

pub fn status() -> TrialStatus {
    let remaining_secs = trial_remaining_secs();
    TrialStatus {
        remaining_secs,
        total_secs: TRIAL_SECS,
        active: remaining_secs > 0,
    }
}

#[tauri::command]
pub fn get_trial_status() -> TrialStatus {
    status()
}

/// Read used seconds from both copies, keep the larger, and write it back
/// so both copies agree. Call once in `setup`, before the overlay starts.
pub fn load(app: &AppHandle) {
    let from_store = read_store(app).unwrap_or(0);
    let from_registry = read_registry().unwrap_or(0);
    let used = from_store.max(from_registry).min(TRIAL_SECS);
    USED_SECS.store(used, Ordering::SeqCst);
    LOADED.store(true, Ordering::SeqCst);
    persist(app);
}

/// Write used seconds to the store and the registry. Never lowers a value
/// already on disk. No-op before [`load`].
pub fn persist(app: &AppHandle) {
    if !LOADED.load(Ordering::SeqCst) {
        return;
    }
    let used = USED_SECS.load(Ordering::SeqCst);

    match app.store(STORE_FILE) {
        Ok(store) => {
            let on_disk = store.get(STORE_KEY).and_then(|v| v.as_u64()).unwrap_or(0);
            store.set(STORE_KEY, serde_json::Value::from(used.max(on_disk)));
            if let Err(e) = store.save() {
                eprintln!("[trial] could not save store: {e}");
            }
        }
        Err(e) => eprintln!("[trial] could not open store: {e}"),
    }

    write_registry(used.max(read_registry().unwrap_or(0)));
}

/// Spawn the thread that adds used seconds while `should_count()` is true.
///
/// `should_count` is supplied by the caller (overlay enabled and not
/// licensed). When the trial reaches zero the thread persists, notifies
/// the settings UI, opens the License tab and exits; the overlay gate
/// stops the overlay on its own because [`is_trial_active`] is now false.
pub fn start_ticker<F>(app: AppHandle, should_count: F)
where
    F: Fn() -> bool + Send + 'static,
{
    std::thread::Builder::new()
        .name("tapt-trial".into())
        .spawn(move || run_ticker(app, should_count))
        .expect("could not spawn trial thread");
}

fn run_ticker<F: Fn() -> bool>(app: AppHandle, should_count: F) {
    let mut last = Instant::now();
    let mut carry = Duration::ZERO;
    let mut unsaved_secs = 0u64;

    // An already-used trial has nothing to count.
    if !is_trial_active() {
        return;
    }

    loop {
        std::thread::sleep(TICK);
        let now = Instant::now();
        let mut elapsed = now.duration_since(last);
        last = now;

        if !should_count() {
            carry = Duration::ZERO;
            continue;
        }
        if elapsed > MAX_TICK {
            elapsed = TICK;
        }
        carry += elapsed;
        let whole = carry.as_secs();
        if whole == 0 {
            continue;
        }
        carry -= Duration::from_secs(whole);
        add_used(whole);
        unsaved_secs += whole;

        let _ = app.emit("trial-status-changed", status());

        if !is_trial_active() {
            persist(&app);
            crate::overlay::focus_license_tab(&app);
            return;
        }
        if unsaved_secs >= PERSIST_EVERY_SECS {
            persist(&app);
            unsaved_secs = 0;
        }
    }
}

fn add_used(secs: u64) {
    let _ = USED_SECS.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |used| {
        Some(used.saturating_add(secs).min(TRIAL_SECS))
    });
}

fn read_store(app: &AppHandle) -> Option<u64> {
    app.store(STORE_FILE).ok()?.get(STORE_KEY)?.as_u64()
}

#[cfg(windows)]
fn read_registry() -> Option<u64> {
    use crate::win_registry::{read_u64, Hive};
    read_u64(Hive::CurrentUser, REG_SUBKEY, REG_VALUE)
}

#[cfg(windows)]
fn write_registry(used: u64) {
    use crate::win_registry::{write_u64, Hive};
    if !write_u64(Hive::CurrentUser, REG_SUBKEY, REG_VALUE, used) {
        eprintln!("[trial] could not write registry value");
    }
}

#[cfg(not(windows))]
fn read_registry() -> Option<u64> {
    None
}

#[cfg(not(windows))]
fn write_registry(_used: u64) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn used_time_caps_at_trial_length() {
        USED_SECS.store(TRIAL_SECS - 2, Ordering::SeqCst);
        assert_eq!(trial_remaining_secs(), 2);
        assert!(is_trial_active());
        add_used(5);
        assert_eq!(trial_remaining_secs(), 0);
        assert!(!is_trial_active());
        assert!(!status().active);
    }
}
