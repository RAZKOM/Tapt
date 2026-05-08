// Mirrors src-tauri/src/lib.rs serde types so the IPC boundary is
// fully typed. Keep these in lockstep with the Rust definitions.

export interface AuthStatusPayload {
  authenticated: boolean;
  reason: string;
}

export type LicenseType = "monthly" | "lifetime" | "trial" | "unknown";

export interface LicenseInfo {
  license_type: LicenseType;
  /** UNIX seconds for expiry UI: `license_variables` / `app_variables` (if set), else signed payload `licenseExpiresAt`, else persisted ceiling. Session `expiresIn` is excluded. */
  subscription_period_end_unix: number | null;
  hwid_masked: string;
}

export interface KeyEvent {
  code: string;
  label: string;
  is_modifier: boolean;
  timestamp: number;
}

export type LicenseErrorCode =
  | "invalid_key"
  | "expired"
  | "revoked"
  | "hwid_mismatch"
  | "rate_limited"
  | "network"
  | "storage"
  | "other";

export interface LicenseFailure {
  code: LicenseErrorCode;
  message: string;
}

export interface PurchaseUrls {
  monthly: string;
  lifetime: string;
  authforge_app_id: string;
}

export type Theme = "light" | "dark" | "system" | "custom";
export type DisplayMode = "key-feed" | "keyboard-overlay";
export type FeedDirection = "vertical" | "horizontal";
export type FeedRowDirection = "left-to-right" | "right-to-left";
export type FeedFilter = "all" | "modifiers-and-combos" | "combos-only";
export type KeyboardLayoutSize = "sixty" | "tkl" | "full";
export type KeyboardAnchor =
  | "top-left"
  | "top-center"
  | "top-right"
  | "middle-left"
  | "middle-right"
  | "center"
  | "bottom-left"
  | "bottom-center"
  | "bottom-right"
  | "free";
export type SnapCorner =
  | "top-left"
  | "top-right"
  | "bottom-left"
  | "bottom-right"
  | "free";

export interface AppearanceConfig {
  theme: Theme;
  primary_accent: string;
  modifier_accent: string;
  font_size: number;
  pill_opacity: number;
  corner_radius: number;
  use_symbols: boolean;
  backdrop_blur: boolean;
}

export interface DisplayConfig {
  mode: DisplayMode;
  feed_direction: FeedDirection;
  /** Horizontal key feed only: which way older pills extend from the anchor. */
  feed_row_direction: FeedRowDirection;
  /** When true, at most one pill per physical key (latest press wins). */
  feed_deduplicate_keys: boolean;
  feed_filter: FeedFilter;
  keyboard_layout: KeyboardLayoutSize;
  /** Unpressed keys (and their labels). */
  keyboard_inactive_opacity: number;
  /** Pressed / held keys (accent highlight). */
  keyboard_active_opacity: number;
  keyboard_scale: number;
  keyboard_anchor: KeyboardAnchor;
  keyboard_offset_x: number;
  keyboard_offset_y: number;
}

export interface BehaviorConfig {
  fade_ms: number;
  max_visible_keys: number;
  launch_at_login: boolean;
  /** When true, open the settings window each time the app starts (tray still available). */
  open_settings_on_startup: boolean;
  global_toggle_hotkey: string | null;
}

export interface OverlayMonitorRect {
  left: number;
  top: number;
  width: number;
  height: number;
}

export interface MonitorInfo {
  left: number;
  top: number;
  width: number;
  height: number;
  is_primary: boolean;
  device_name: string;
}

export interface PositionConfig {
  snap: SnapCorner;
  offset_x: number;
  offset_y: number;
  remember_position: boolean;
  /** Pin the native overlay to this monitor; `null` uses the OS primary display. */
  overlay_monitor: OverlayMonitorRect | null;
}

export interface Config {
  appearance: AppearanceConfig;
  display: DisplayConfig;
  behavior: BehaviorConfig;
  position: PositionConfig;
}

export const DEFAULT_CONFIG: Config = {
  appearance: {
    theme: "system",
    primary_accent: "#04cfe5",
    modifier_accent: "#ff7e5b",
    font_size: 18,
    pill_opacity: 90,
    corner_radius: 10,
    use_symbols: true,
    backdrop_blur: true,
  },
  display: {
    mode: "key-feed",
    feed_direction: "vertical",
    feed_row_direction: "right-to-left",
    feed_deduplicate_keys: false,
    feed_filter: "all",
    keyboard_layout: "tkl",
    keyboard_inactive_opacity: 75,
    keyboard_active_opacity: 100,
    keyboard_scale: 100,
    keyboard_anchor: "bottom-center",
    keyboard_offset_x: 0,
    keyboard_offset_y: 56,
  },
  behavior: {
    fade_ms: 2000,
    max_visible_keys: 6,
    launch_at_login: false,
    open_settings_on_startup: true,
    global_toggle_hotkey: null,
  },
  position: {
    snap: "bottom-left",
    offset_x: 24,
    offset_y: 24,
    remember_position: true,
    overlay_monitor: null,
  },
};
