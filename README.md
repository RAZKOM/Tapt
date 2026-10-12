# Tapt.

Windows desktop overlay that displays keyboard input in real time. Built with **Tauri v2** (Rust backend) and **React 18 + TypeScript + Framer Motion** (frontend).

Designed for screencasters, streamers, tutorial makers, and live presenters who want their audience to see what keys they are pressing.

> Commercial product: **one hour of free trial** built in, then a **one-time lifetime license ($2.99)**. Licenses are activated offline once per PC through the AuthForge SDK; after that Tapt never needs the network to stay licensed.

---

## Features

- **Two display modes** — animated Key Feed (stacking pills) and on-screen Keyboard Overlay (60% / TKL / Full ANSI layouts)
- **Transparent, click-through, always-on-top** overlay window
- **Visible to OBS / screen capture** on Windows
- **Tray-icon controlled** — no Taskbar entry for the overlay
- **License-gated** — Silent Gate drops keyboard events for the overlay unless the PC is licensed or free-trial time remains (`rdev`; keys never reach the native overlay renderer)
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
│       ├── auth.rs           # AuthForge offline activation + license Tauri commands
│       ├── trial.rs          # Built-in one-hour free trial (all trial logic)
│       ├── version.rs        # Update check (latest RAZKOM/Tapt. GitHub release)
│       ├── win_registry.rs   # MachineGuid (HWID) + HKCU trial value
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

In debug builds an **empty `.env`** is acceptable for compilation. The real AuthForge SDK rejects every key with `InvalidApp` until you populate `AUTHFORGE_APP_ID`, `AUTHFORGE_APP_SECRET`, and `AUTHFORGE_PUBLIC_KEY` in `src-tauri/.env`, but the rest of the app (windows, settings, frontend) is fully runnable. Set `STRIPE_LIFETIME_URL` to test the License tab "Buy lifetime" button.

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

A release build **requires** all `AUTHFORGE_*` values and `STRIPE_LIFETIME_URL` in `src-tauri/.env` (or in the environment). `build.rs` will fail loudly if any are missing.

### Verifying secret encryption

After a release build, confirm no plaintext secrets escaped:

```powershell
Select-String -Path "src-tauri\target\release\tapt.exe" -Pattern "AUTHFORGE" -SimpleMatch -Quiet
# Should return False; also run Sysinternals sigcheck when auditing binaries.
```

There must be no recognizable AuthForge URL, app id, or key fragments. Everything sensitive is wrapped in `cryptify::encrypt_string!` and decrypted on demand at the call site.

## AuthForge integration

We use the real `authforge` crate from crates.io (version pinned in [`src-tauri/Cargo.toml`](src-tauri/Cargo.toml)). The integration is in [`src-tauri/src/auth.rs`](src-tauri/src/auth.rs) and uses AuthForge **self-serve offline activation**:

- **Activate** calls `activate_offline(key, path)` once. AuthForge returns a signed, lifetime `.authforge` file bound to this PC's HWID; the SDK verifies it and writes it to `%APPDATA%\com.razkom.tapt\license.authforge` (the app data dir, never the install dir). The key is also kept in the plugin store so a deleted file or a reinstall re-activates silently. The same PC re-activating is free and gets the identical file back.
- **Every launch** verifies that file locally with `login_from_file`: no network, no heartbeat. If the file is missing or invalid and a key is saved, Tapt calls `activate_offline` once with the saved key.
- **Only perpetual licenses** can be activated. A key with an expiry gets `offline_activation_requires_perpetual` and the License tab shows "Activation isn't available for this license." There is no online login and no heartbeat anywhere in the app.
- **HWID** is `SHA-256("tapt|" + MachineGuid)` from `HKLM\SOFTWARE\Microsoft\Cryptography\MachineGuid`, passed via `AuthForgeConfig::hwid_override`, so what the License tab shows (masked `••••XXXX`) is exactly what the server binds. Every new HWID uses one of the license's machine slots for good, so do not change this scheme. If the registry can't be read, Tapt falls back to the old `SHA-256(hostname | username | os)` and logs it.
- **Remove license from this PC** deletes the file and the saved key. It does not free an activation slot; re-activating the same PC later is free.
- **No customer portal.** Portal HWID resets free seats but never machine slots, and an issued file keeps working regardless, so the portal can't help offline-licensed buyers. Support is by email at **hello@razkom.com** (`SUPPORT_EMAIL` in `auth.rs` and `branding.ts`; activation errors that need help name it, and the License tab links it). Moving a license to new hardware is a support task: release the old machine (or raise the license's HWID slots) in the AuthForge dashboard.

### Free trial

[`src-tauri/src/trial.rs`](src-tauri/src/trial.rs) gives every PC **one hour of cumulative use**: time Tapt runs with the overlay enabled while unlicensed, measured with a monotonic clock so changing the system time has no effect. Used seconds are saved every ~30 s and on exit, both in the plugin store and in `HKCU\Software\Razkom\Tapt\TrialUsedSeconds`; on load the larger value wins. The overlay works while licensed **or** while trial time remains; at zero it stops and the License tab opens.

### App version and updates

The installed build version comes from **`src-tauri/Cargo.toml`** (`CARGO_PKG_VERSION` at compile time). Bump it together with `src-tauri/tauri.conf.json` and `package.json` when you cut a release.

The latest version is the tag of the latest published release in the public releases repo, [RAZKOM/Tapt.](https://github.com/RAZKOM/Tapt./releases) (tags like `v1.0.2`). Tapt reads it from the GitHub API (`/repos/RAZKOM/Tapt./releases/latest`) unauthenticated, at startup and every 6 hours, silently ignoring any failure, independent of the license and the trial. Drafts and pre-releases don't count, so a release only reaches users once it is published. When it is newer than the running build, a banner offers **Download** (opens [razkom.com/tapt](https://razkom.com/tapt/)). The License tab footer always shows the installed version. AuthForge `app_variables` are no longer used for this: offline license files freeze them at activation time.

### Error mapping

`error_to_message` maps every `AuthForgeError` code, including the offline-activation codes that arrive as `AuthForgeError::Other(code)`, to a user-facing string. The frontend receives both a stable `code` (snake_case) and the `message`. Activation errors never exit or log the user out: the message is shown and the user can retry.

## Required permissions

`rdev` uses `SetWindowsHookEx(WH_KEYBOARD_LL)` which needs no special permission at user level. The overlay is set to `WS_EX_TRANSPARENT | WS_EX_LAYERED | WS_EX_TOOLWINDOW` so it is click-through and excluded from the taskbar.

## License

Proprietary — © Tapt.
