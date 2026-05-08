//! Persistent user config — appearance, display mode, behavior,
//! position. Persisted via `tauri-plugin-store` to a platform-
//! appropriate path (Windows):
//!
//!   `%APPDATA%\com.tapt.app\config.json`

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Theme {
    Light,
    Dark,
    System,
    Custom,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum DisplayMode {
    #[default]
    KeyFeed,
    KeyboardOverlay,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum FeedDirection {
    #[default]
    Vertical,
    Horizontal,
}

/// Horizontal key-feed only: growth direction of pills along the row.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum FeedRowDirection {
    /// Newest at the leading (left) edge; older pills extend rightward.
    LeftToRight,
    /// Newest at the trailing (right) edge; older pills extend leftward.
    #[default]
    RightToLeft,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum FeedFilter {
    #[default]
    All,
    ModifiersAndCombos,
    CombosOnly,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum KeyboardLayoutSize {
    Sixty,
    #[default]
    Tkl,
    Full,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum SnapCorner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    Free,
}

/// Where to pin the keyboard panel on the overlay (offsets refine from that anchor).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum KeyboardAnchor {
    TopLeft,
    TopCenter,
    TopRight,
    MiddleLeft,
    #[default]
    BottomCenter,
    MiddleRight,
    Center,
    BottomLeft,
    BottomRight,
    Free,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppearanceConfig {
    pub theme: Theme,
    pub primary_accent: String,
    pub modifier_accent: String,
    pub font_size: u32,
    pub pill_opacity: u32,
    pub corner_radius: u32,
    pub use_symbols: bool,
    pub backdrop_blur: bool,
}

/// Default used when migrating configs that only stored `keyboard_opacity`.
fn default_keyboard_inactive_opacity() -> u32 {
    75
}

fn default_keyboard_active_opacity() -> u32 {
    100
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DisplayConfig {
    pub mode: DisplayMode,
    pub feed_direction: FeedDirection,
    /// Only affects horizontal key feed (see [`FeedRowDirection`]).
    #[serde(default)]
    pub feed_row_direction: FeedRowDirection,
    #[serde(default = "default_feed_deduplicate_keys")]
    pub feed_deduplicate_keys: bool,
    pub feed_filter: FeedFilter,
    pub keyboard_layout: KeyboardLayoutSize,
    /// Idle / unpressed key caps and borders (labels included).
    #[serde(
        default = "default_keyboard_inactive_opacity",
        alias = "keyboard_opacity"
    )]
    pub keyboard_inactive_opacity: u32,
    /// Pressed / held keys (accent fills and labels).
    #[serde(default = "default_keyboard_active_opacity")]
    pub keyboard_active_opacity: u32,
    pub keyboard_scale: u32,
    pub keyboard_anchor: KeyboardAnchor,
    pub keyboard_offset_x: i32,
    pub keyboard_offset_y: i32,
}

fn default_open_settings_on_startup() -> bool {
    true
}

fn default_feed_deduplicate_keys() -> bool {
    false
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BehaviorConfig {
    pub fade_ms: u32,
    pub max_visible_keys: u32,
    pub launch_at_login: bool,
    /// When true, the settings window is shown each time the app starts.
    #[serde(default = "default_open_settings_on_startup")]
    pub open_settings_on_startup: bool,
    pub global_toggle_hotkey: Option<String>,
}

/// Virtual-screen rectangle of a display (`rcMonitor`), used to pin the overlay.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct OverlayMonitorRect {
    pub left: i32,
    pub top: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionConfig {
    pub snap: SnapCorner,
    pub offset_x: i32,
    pub offset_y: i32,
    pub remember_position: bool,
    /// When set, the native overlay fills this monitor rectangle instead of relying on platform default fullscreen.
    #[serde(default)]
    pub overlay_monitor: Option<OverlayMonitorRect>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    pub appearance: AppearanceConfig,
    pub display: DisplayConfig,
    pub behavior: BehaviorConfig,
    pub position: PositionConfig,
}

impl Default for AppearanceConfig {
    fn default() -> Self {
        Self {
            theme: Theme::System,
            primary_accent: "#04cfe5".into(),
            modifier_accent: "#ff7e5b".into(),
            font_size: 18,
            pill_opacity: 90,
            corner_radius: 10,
            use_symbols: true,
            backdrop_blur: true,
        }
    }
}

impl Default for BehaviorConfig {
    fn default() -> Self {
        Self {
            fade_ms: 2000,
            max_visible_keys: 6,
            launch_at_login: false,
            open_settings_on_startup: default_open_settings_on_startup(),
            global_toggle_hotkey: None,
        }
    }
}

impl Default for PositionConfig {
    fn default() -> Self {
        Self {
            snap: SnapCorner::BottomLeft,
            offset_x: 24,
            offset_y: 24,
            remember_position: true,
            overlay_monitor: None,
        }
    }
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            mode: DisplayMode::default(),
            feed_direction: FeedDirection::default(),
            feed_row_direction: FeedRowDirection::default(),
            feed_deduplicate_keys: default_feed_deduplicate_keys(),
            feed_filter: FeedFilter::default(),
            keyboard_layout: KeyboardLayoutSize::default(),
            keyboard_inactive_opacity: default_keyboard_inactive_opacity(),
            keyboard_active_opacity: default_keyboard_active_opacity(),
            keyboard_scale: 100,
            keyboard_anchor: KeyboardAnchor::default(),
            keyboard_offset_x: 0,
            keyboard_offset_y: 56,
        }
    }
}

/// Storage key inside the tauri-plugin-store JSON file.
pub const CONFIG_STORE_KEY: &str = "config";
/// File name used by the store plugin under app data dir.
pub const STORE_FILE: &str = "config.json";
