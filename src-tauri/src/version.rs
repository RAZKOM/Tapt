//! Installed app version (`CARGO_PKG_VERSION`) and update checks against
//! AuthForge **`app_variables`** (e.g. `version` = latest build on razkom.com/tapt).

use std::cmp::Ordering;
use std::collections::HashMap;

use authforge::AuthForgeClient;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// Must stay in sync with `Cargo.toml` / `tauri.conf.json` `version`.
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

pub const RELEASES_URL: &str = "https://razkom.com/tapt/";

const VERSION_KEYS: &[&str] = &[
    "version",
    "minVersion",
    "min_version",
    "latest_version",
    "latestVersion",
    "app_version",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateStatus {
    pub current_version: String,
    pub latest_version: Option<String>,
    pub update_available: bool,
    pub releases_url: String,
}

pub fn update_status_for_client(client: Option<&AuthForgeClient>) -> UpdateStatus {
    let current_version = APP_VERSION.to_string();
    let latest_version = client.and_then(latest_version_from_client);
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

fn latest_version_from_client(client: &AuthForgeClient) -> Option<String> {
    if let Some(map) = client.get_app_variables() {
        if let Some(v) = latest_version_from_var_map(&map) {
            return Some(v);
        }
    }

    // SERVER heartbeats refresh `session_data` but the SDK does not always
    // mirror `appVariables` into `get_app_variables()` — read both sources.
    client
        .get_session_data()
        .as_ref()
        .and_then(latest_version_from_session_json)
}

fn latest_version_from_session_json(session: &Value) -> Option<String> {
    let app_vars = session
        .get("appVariables")
        .or_else(|| session.get("app_variables"));
    if let Some(vars) = app_vars {
        if let Value::Object(map) = vars {
            if let Some(v) = latest_version_from_json_map(map) {
                return Some(v);
            }
        }
    }

    if let Value::Object(map) = session {
        latest_version_from_json_map(map)
    } else {
        None
    }
}

fn latest_version_from_json_map(map: &Map<String, Value>) -> Option<String> {
    for key in VERSION_KEYS {
        if let Some(v) = map.get(*key) {
            if let Some(normalized) = version_from_json_value(v) {
                return Some(normalized);
            }
        }
    }
    None
}

fn latest_version_from_var_map(map: &HashMap<String, Value>) -> Option<String> {
    for key in VERSION_KEYS {
        if let Some(v) = map.get(*key) {
            if let Some(normalized) = version_from_json_value(v) {
                return Some(normalized);
            }
        }
    }
    None
}

fn version_from_json_value(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => normalize_version_string(s),
        Value::Number(n) => n
            .as_f64()
            .filter(|f| f.is_finite())
            .map(|f| normalize_version_string(&f.to_string()))
            .flatten(),
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
    fn reads_version_from_session_app_variables() {
        let session = serde_json::json!({
            "sessionToken": "x",
            "expiresIn": 86400,
            "appVariables": { "version": "1.0.1" }
        });
        assert_eq!(
            latest_version_from_session_json(&session).as_deref(),
            Some("1.0.1")
        );
    }

    #[test]
    fn reads_min_version_key() {
        let map = HashMap::from([(
            "minVersion".to_string(),
            Value::String("2.0.0".into()),
        )]);
        assert_eq!(
            latest_version_from_var_map(&map).as_deref(),
            Some("2.0.0")
        );
    }
}
