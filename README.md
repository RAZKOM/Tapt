# Tapt.

Windows desktop overlay that displays keyboard input in real time. Built with **Tauri v2** (Rust backend) and **React 18 + TypeScript + Framer Motion** (frontend).

Designed for screencasters, streamers, tutorial makers, and live presenters who want their audience to see what keys they are pressing.

> Commercial product: **one-time** 1‑month license ($2.99) or lifetime license ($9.99). All functionality is gated behind license validation via the AuthForge SDK.

---

## Features

- **Two display modes** — animated Key Feed (stacking pills) and on-screen Keyboard Overlay (60% / TKL / Full ANSI layouts)
- **Transparent, click-through, always-on-top** overlay window
- **Visible to OBS / screen capture** on Windows
- **Tray-icon controlled** — no Taskbar entry for the overlay
- **License-gated** — Silent Gate drops keyboard events for the overlay when the session is not authenticated (`rdev`; keys never reach the native overlay renderer)
- **Hardened binary** — `cryptify::encrypt_string!` for secrets, full LTO, stripped, `panic = "abort"`, `#[inline(always)]` auth checks, self-ban tamper response

## Project layout

```
.
├── package.json              # Vite + React frontend deps; `icons:generate` → Tauri icon
├── vite.config.ts            # Dev server + React build (`src/` → `dist/`)
├── index.html                # Entry for the Tauri settings webview (single page)
├── src/                      # React + TypeScript (settings UI only)
│   ├── main.tsx              # Mounts SettingsApp
│   ├── settings/             # Tabbed settings + License tab
│   ├── hooks/                # useAuthGuard, useConfig, useThemeTokens, …
│   ├── store/                # Zustand (auth, config)
│   ├── branding.ts
│   └── styles/tokens.css
├── scripts/
│   └── icons-generate.cjs    # Thin wrapper around `npx tauri icon` (see Icons below)
├── src-tauri/
│   ├── Cargo.toml            # Hardened release profile; `authforge` version pinned in Cargo.toml
│   ├── build.rs              # Reads .env → rustc env / compile-time crypto
│   ├── tauri.conf.json       # Settings webview window, tray (`icons/icon.ico`), capabilities
│   ├── capabilities/
│   ├── icons/                # Tray / window / bundle assets (`tauri icon` output)
│   └── src/
│       ├── main.rs           # Tray, IPC commands, wires listener → overlay channel
│       ├── lib.rs            # Shared serde types crossing the IPC boundary
│       ├── auth.rs           # AuthForge + license Tauri commands
│       ├── config.rs         # Persistent config store
│       ├── listener.rs       # Single `rdev::listen` dispatch thread (process-wide)
│       ├── keylistener.rs    # Registers key handler + Silent Gate
│       ├── overlay.rs        # Helpers for the settings window (e.g. focus License tab)
│       ├── overlay_window.rs # Native transparent overlay (`eframe` / egui, not WebView)
│       └── toggle_shortcut.rs # Global shortcut for overlay toggle
├── .github/workflows/        # Windows CI
└── .env.example              # Template for `src-tauri/.env`
```

## Prerequisites

- **Node.js** 18+
- **Rust** 1.77+ (`rustup default stable`)
- **Tauri prerequisites** — see <https://v2.tauri.app/start/prerequisites/> (Windows: WebView2, MSVC build tools)

## First-time setup

```bash
# Install JS deps
npm install

# Copy the env template, fill in values when you have them
cp .env.example src-tauri/.env

# Compile-check the Rust side (works with empty .env)
cd src-tauri
cargo check
cd ..
```

## Running locally

```bash
# Hot-reloading dev mode (native overlay thread + settings webview + Rust backend)
npm run tauri dev
```

In debug builds an **empty `.env`** is acceptable for compilation. The real AuthForge SDK rejects every key with `InvalidApp` until you populate `AUTHFORGE_APP_ID`, `AUTHFORGE_APP_SECRET`, and `AUTHFORGE_PUBLIC_KEY` in `src-tauri/.env`, but the rest of the app (windows, settings, frontend) is fully runnable. Set `STRIPE_*` to test the License tab "Buy" buttons.

## Icons

Tray and bundled icons live under [`src-tauri/icons/`](src-tauri/icons/). To regenerate **all** platform sizes from a square master image (overwrites that folder):

```bash
npm run icons:generate -- "C:/path/to/your-square-icon.png"
```

(`npm` needs the `--` before the path; this runs `tauri icon` via [`scripts/icons-generate.cjs`](scripts/icons-generate.cjs).) The executable also embeds `icons/32x32.png` for the settings window title bar where ICO conversion can fail on Windows.

## Producing release builds

```bash
npm run tauri build
```

A release build **requires** all `AUTHFORGE_*` and `STRIPE_*` values in `src-tauri/.env` (or in the environment). `build.rs` will fail loudly if any are missing.

### Verifying secret encryption

After a release build, confirm no plaintext secrets escaped:

```powershell
Select-String -Path "src-tauri\target\release\tapt.exe" -Pattern "AUTHFORGE" -SimpleMatch -Quiet
# Should return False; also run Sysinternals sigcheck when auditing binaries.
```

There must be no recognizable AuthForge URL, app id, or key fragments. Everything sensitive is wrapped in `cryptify::encrypt_string!` and decrypted on demand at the call site.

## AuthForge integration

We use the real `authforge` crate from crates.io (version pinned in [`src-tauri/Cargo.toml`](src-tauri/Cargo.toml)). The integration is in [`src-tauri/src/auth.rs`](src-tauri/src/auth.rs):

- Construct an `AuthForgeClient` once at startup with `HeartbeatMode::Server` and a 900s interval (~3-4 credits/day per active user).
- Pass a locally-computed HWID via `AuthForgeConfig::hwid_override` so what we display in the License tab is exactly what the SDK sends to `/auth/validate`. The HWID is `SHA-256(hostname | username | os)` and the License tab shows the masked form `••••XXXX`.
- The `on_failure` callback emits `auth-status-changed { authenticated: false, reason }` so the native overlay stops rendering and the settings window can navigate to the License tab.

### License classification

The SDK doesn't categorize licenses itself — it returns a generic `LoginResult { session_token, expires_in, app_variables, license_variables, request_id }`. We classify each license by reading `license_variables` and matching the value of one of these keys (preferred order):

1. `type`
2. `plan`
3. `tier`

Recognized values (case-insensitive):

| Server value                         | UI shows               |
| ------------------------------------ | ---------------------- |
| `monthly` / `subscription` / `subscription_monthly` | "1-month license" (UI label; AuthForge may still use `subscription*` values — pricing here is a **one-time purchase**, not recurring billing) |
| `lifetime` / `perpetual`             | "Lifetime"             |
| `trial`                              | "Trial"                |
| anything else / missing              | "Active"               |

Configure these on the licensing server when issuing keys.

### License expiry countdown

The signed payload (base64 inside the HTTP `payload` field) includes **`expiresIn`** for the **session JWT** and, when your validate endpoint provides it, **`licenseExpiresAt`** as an ISO‑8601 string (or JSON `null` for lifetime). The app reads **`licenseExpiresAt`** from the SDK’s stored session JSON for the License tab countdown; it does **not** treat rolling session `expiresIn` as license end time.

You can also place an end time in **`license_variables`** (e.g. `expiresAt` per [AuthForge variables](https://docs.authforge.cc/llms-full.txt)); map keys take precedence over `licenseExpiresAt` when both are set.

If **`licenseExpiresAt` is absent** from the JSON the SDK stores between heartbeats, expiry copy falls back to the persisted ceiling until the next successful login. Prefer including the same top-level license fields on **`/auth/heartbeat`** success payloads whenever possible.

### App version and updates

The installed build version comes from **`src-tauri/Cargo.toml`** / **`tauri.conf.json`** (`CARGO_PKG_VERSION` at compile time). Bump both when you cut a release.

Set AuthForge **`app_variables.version`** to the latest shipped build (e.g. `1.0.1` or `v1.0.1`). After login, the settings UI compares that value to the running build; when the server version is newer, a banner offers **Download** (opens [razkom.com/tapt](https://razkom.com/tapt/)). The License tab footer always shows the installed version.

### Error mapping

All 18 `AuthForgeError` variants are mapped to user-facing strings in `error_to_message`. The frontend receives both a stable `code` (snake_case) and the localized `message` so the License tab can branch on the code while displaying the message.

## Required permissions

`rdev` uses `SetWindowsHookEx(WH_KEYBOARD_LL)` which needs no special permission at user level. The overlay is set to `WS_EX_TRANSPARENT | WS_EX_LAYERED | WS_EX_TOOLWINDOW` so it is click-through and excluded from the taskbar.

## License

Proprietary — © Tapt.
