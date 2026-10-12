//! Installed app version (`CARGO_PKG_VERSION`) and the update check.
//!
//! The latest version is the `tag_name` of the latest published release of
//! this repo ([`UPDATE_MANIFEST_URL`], `RAZKOM/Tapt`). CI publishes one for
//! every `v*` tag; that (not a draft or pre-release) is all it takes for
//! installed copies to offer the update.
//!
//! The fetch is unauthenticated and failure-silent, and runs on its own
//! thread. It has nothing to do with licensing or the trial: offline
//! license files freeze AuthForge `app_variables` at activation time, and
//! trial users have no AuthForge session at all, so `app_variables` can't
//! carry a "latest version" anymore.

use std::cmp::Ordering;
use std::sync::Mutex;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Emitter};

/// Must stay in sync with `Cargo.toml` / `tauri.conf.json` `version`.
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

pub const RELEASES_URL: &str = "https://razkom.com/tapt/";

/// Latest release of the public releases repo (GitHub REST API). Drafts
/// and pre-releases are excluded by GitHub. Unauthenticated calls allow 60
/// requests per hour per IP; we make one every [`CHECK_INTERVAL`].
pub const UPDATE_MANIFEST_URL: &str = "https://api.github.com/repos/RAZKOM/Tapt/releases/latest";

const CHECK_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// Keys read from the response: GitHub's `tag_name` (`v1.0.1`), or a plain
/// `version` if the source is ever swapped for a static JSON file.
const VERSION_KEYS: &[&str] = &["tag_name", "version"];

/// Latest version from the last successful check.
static LATEST_VERSION: Mutex<Option<String>> = Mutex::new(None);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateStatus {
    pub current_version: String,
    pub latest_version: Option<String>,
    pub update_available: bool,
    pub releases_url: String,
}

pub fn current_status() -> UpdateStatus {
    let latest = LATEST_VERSION.lock().ok().and_then(|v| v.clone());
    status_for(latest)
}

fn status_for(latest_version: Option<String>) -> UpdateStatus {
    let current_version = APP_VERSION.to_string();
    let update_available = latest_version
        .as_deref()
        .is_some_and(|latest| version_cmp(latest, &current_version) == Ordering::Greater);

    UpdateStatus {
        current_version,
        latest_version,
        update_available,
        releases_url: RELEASES_URL.to_string(),
    }
}

/// Check now and then every [`CHECK_INTERVAL`]; emit `update-status-changed`
/// when the known latest version changes.
pub fn spawn_update_checker(app: AppHandle) {
    let spawned = std::thread::Builder::new()
        .name("tapt-update-check".into())
        .spawn(move || loop {
            if let Some(latest) = fetch_latest_version() {
                let changed = match LATEST_VERSION.lock() {
                    Ok(mut slot) if slot.as_deref() != Some(latest.as_str()) => {
                        *slot = Some(latest);
                        true
                    }
                    _ => false,
                };
                if changed {
                    let _ = app.emit("update-status-changed", current_status());
                }
            }
            std::thread::sleep(CHECK_INTERVAL);
        });
    if let Err(e) = spawned {
        eprintln!("[update] could not spawn update check: {e}");
    }
}

fn fetch_latest_version() -> Option<String> {
    let response = ureq::get(UPDATE_MANIFEST_URL)
        .timeout(REQUEST_TIMEOUT)
        // GitHub rejects API requests without a User-Agent.
        .set("User-Agent", concat!("Tapt/", env!("CARGO_PKG_VERSION")))
        .set("Accept", "application/vnd.github+json")
        .call()
        .ok()?;
    let body: Value = response.into_json().ok()?;
    latest_version_from_manifest(&body)
}

fn latest_version_from_manifest(manifest: &Value) -> Option<String> {
    let map = manifest.as_object()?;
    VERSION_KEYS
        .iter()
        .filter_map(|key| map.get(*key))
        .find_map(version_from_json_value)
}

fn version_from_json_value(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => normalize_version_string(s),
        Value::Number(n) => n
            .as_f64()
            .filter(|f| f.is_finite())
            .and_then(|f| normalize_version_string(&f.to_string())),
        _ => None,
    }
}

/// Strip a leading `v` and keep a numeric `major.minor.patch` prefix.
fn normalize_version_string(raw: &str) -> Option<String> {
    let t = raw.trim().trim_start_matches(['v', 'V']).trim();
    if t.is_empty() {
        return None;
    }
    let core: String = t
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    if core.is_empty() || !core.chars().any(|c| c.is_ascii_digit()) {
        return None;
    }
    Some(core)
}

fn parse_version_parts(s: &str) -> Option<(u32, u32, u32)> {
    let core = normalize_version_string(s)?;
    let mut nums = core.split('.').map(|p| p.parse::<u32>().ok());
    let major = nums.next().flatten()?;
    let minor = nums.next().flatten().unwrap_or(0);
    let patch = nums.next().flatten().unwrap_or(0);
    Some((major, minor, patch))
}

fn version_cmp(a: &str, b: &str) -> Ordering {
    match (parse_version_parts(a), parse_version_parts(b)) {
        (Some(pa), Some(pb)) => pa.cmp(&pb),
        _ => a.cmp(b),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_semver_triplets() {
        assert_eq!(version_cmp("1.0.1", "1.0.0"), Ordering::Greater);
        assert_eq!(version_cmp("1.0.0", "1.0.1"), Ordering::Less);
        assert_eq!(version_cmp("v2.0.0", "1.9.9"), Ordering::Greater);
        assert_eq!(version_cmp("1.0", "1.0.0"), Ordering::Equal);
    }

    #[test]
    fn reads_version_from_manifest() {
        let manifest = serde_json::json!({ "version": "1.0.2" });
        assert_eq!(latest_version_from_manifest(&manifest).as_deref(), Some("1.0.2"));
    }

    #[test]
    fn reads_github_release_tag() {
        let release = serde_json::json!({ "tag_name": "v2.1.0", "draft": false });
        assert_eq!(latest_version_from_manifest(&release).as_deref(), Some("2.1.0"));
    }

    /// Hits the real GitHub API: `cargo test -- --ignored live_latest_release`.
    #[test]
    #[ignore]
    fn live_latest_release() {
        let latest = fetch_latest_version().expect("latest release readable");
        assert!(parse_version_parts(&latest).is_some(), "{latest}");
    }

    #[test]
    fn missing_or_bad_manifest_is_silent() {
        assert_eq!(latest_version_from_manifest(&serde_json::json!([])), None);
        assert_eq!(latest_version_from_manifest(&serde_json::json!({ "version": "" })), None);
        assert!(!status_for(None).update_available);
    }
}
