# Developing Tapt

Tapt is a Windows desktop overlay that shows keyboard input in real time. It's built with **Tauri v2** (Rust backend, native `eframe`/egui overlay) and **React 18 + TypeScript** (settings window).

## Project layout

```
.
├── package.json              # Vite + React frontend; `icons:generate` wraps `tauri icon`
├── vite.config.ts            # Dev server + React build (`src/` → `dist/`)
├── index.html                # Entry for the settings webview
├── src/                      # React + TypeScript (settings UI only)
│   ├── main.tsx              # Mounts SettingsApp
│   ├── settings/             # Tabs: License, Behavior, Appearance, Position
│   ├── hooks/                # useAuthGuard, useConfig, useUpdateCheck, …
│   ├── store/                # Zustand (auth, config)
│   └── branding.ts           # Product name, support email, download page
├── scripts/
│   └── icons-generate.cjs    # Thin wrapper around `npx tauri icon`
├── src-tauri/
│   ├── Cargo.toml            # Release profile; `authforge` version pinned here
│   ├── build.rs              # Reads .env → compile-time, obfuscated credentials
│   ├── tauri.conf.json       # Settings window, tray, bundle config
│   ├── icons/                # Tray / window / installer icons (`tauri icon` output)
│   ├── assets/fonts/         # Noto fonts for the overlay (SIL OFL, see OFL.txt)
│   └── src/
│       ├── main.rs           # Tray, IPC commands, startup, wires listener → overlay
│       ├── lib.rs            # Shared types crossing the IPC boundary
│       ├── auth.rs           # AuthForge offline activation + license commands
│       ├── trial.rs          # Built-in one-hour free trial (all trial logic)
│       ├── version.rs        # Update check (latest GitHub release of this repo)
│       ├── win_registry.rs   # MachineGuid (HWID) + HKCU trial value
│       ├── config.rs         # Persistent settings
│       ├── listener.rs       # Single `rdev::listen` dispatch thread
│       ├── keylistener.rs    # Key handler + license gate
│       ├── overlay.rs        # Settings-window helpers (e.g. focus the License tab)
│       ├── overlay_window.rs # Native transparent overlay (`eframe`/egui, not a WebView)
│       └── toggle_shortcut.rs # Global show/hide shortcut
├── docs/                     # This guide + README assets
├── .github/workflows/        # Windows CI and releases
└── .env.example              # Template for `src-tauri/.env`
```

## Prerequisites

- **Node.js** 18+
- **Rust** 1.77+ (`rustup default stable`)
- **Tauri prerequisites**: see <https://v2.tauri.app/start/prerequisites/> (Windows: WebView2, MSVC build tools)

## Build and run

```bash
npm install
cp .env.example src-tauri/.env   # optional for debug builds

# Hot-reloading dev mode (native overlay + settings window + Rust backend)
npm run tauri dev
```

Debug builds compile with an empty `.env`. Without the AuthForge credentials, activation fails with "This copy of Tapt can't activate licenses", and the app runs as the 1-hour free trial. Everything else works normally.

Checks CI runs on every push and pull request:

```bash
npm run build                                   # tsc + vite
cd src-tauri && cargo clippy --all-targets && cargo test
```

## Release builds

```bash
npm run tauri build
```

A release build **requires** `AUTHFORGE_APP_ID`, `AUTHFORGE_APP_SECRET`, `AUTHFORGE_PUBLIC_KEY` and `STRIPE_LIFETIME_URL` in `src-tauri/.env` or the environment; `build.rs` fails loudly if any is missing. They're compiled in wrapped in `cryptify::encrypt_string!`, so they don't appear as plain text in the binary. CI checks that after every release build.

The release profile strips symbols and uses full LTO, `codegen-units = 1` and `panic = "abort"`.

## Releasing

Releases are automated by [`.github/workflows/build.yml`](../.github/workflows/build.yml):

1. Bump the version in `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json` and `package.json`.
2. Commit, then create an annotated tag. Its message becomes the release notes:
   ```bash
   git tag -a v1.2.0 -m "Changes:" -m "- …"
   git push origin main v1.2.0
   ```
3. CI checks the tag matches all three versions, builds with the secrets from the repo's Actions secrets, checks the binary for leaked credentials, and publishes a GitHub release with the installer as **`TaptSetup.exe`**.

That release is what [razkom.com/tapt](https://razkom.com/tapt/) links to (`releases/latest/download/TaptSetup.exe`), and what installed copies compare against to show "Update available". Pull requests from forks build without secrets and never publish.

## Licensing (AuthForge)

[`src-tauri/src/auth.rs`](../src-tauri/src/auth.rs) uses AuthForge **self-serve offline activation** through the `authforge` crate:

- **Activate** calls `activate_offline(key, path)` once. AuthForge returns a signed, lifetime `.authforge` file bound to this PC's HWID, which the SDK verifies and writes to `%APPDATA%\com.razkom.tapt\license.authforge`. The key is kept in the settings store, so a deleted file or a reinstall re-activates silently. The same PC re-activating is free and gets the identical file back.
- **Every launch** verifies that file locally with `login_from_file`, with no network and no heartbeat. If the file is missing or invalid and a key is saved, Tapt calls `activate_offline` once with the saved key.
- **Only perpetual licenses** can be activated. A key with an expiry gets `offline_activation_requires_perpetual`.
- **HWID** is `SHA-256("tapt|" + MachineGuid)` from `HKLM\SOFTWARE\Microsoft\Cryptography\MachineGuid`, passed through `AuthForgeConfig::hwid_override`. Every new HWID uses one of the license's machine slots for good, so **don't change this scheme**. If the registry can't be read, Tapt falls back to `SHA-256(hostname | username | os)` and logs it.
- **Remove license from this PC** deletes the file and the saved key. It doesn't free a machine slot; re-activating the same PC later is free.
- **Support** is by email at hello@razkom.com (`SUPPORT_EMAIL` in `auth.rs` and `branding.ts`). Error messages that need help name it, and the License tab turns it into a link. There's no customer portal: portal HWID resets free seats but never machine slots, and an issued file keeps working anyway. To move a buyer to new hardware, release their old machine or raise the license's HWID slots in the AuthForge dashboard.

`error_to_message` maps every `AuthForgeError` code, including the offline-activation codes that arrive as `AuthForgeError::Other(code)`, to a user-facing message. Activation errors never exit or log the user out; the message is shown and the user can retry.

### Free trial

[`src-tauri/src/trial.rs`](../src-tauri/src/trial.rs) gives every PC **one hour of cumulative use**: time Tapt runs with the overlay on while unlicensed, measured with a monotonic clock so changing the system time has no effect. Used seconds are saved about every 30 seconds and on exit, both in the settings store (`trial_used_secs`) and in `HKCU\Software\Razkom\Tapt\TrialUsedSeconds`; on load the larger value wins. The overlay works while licensed **or** while trial time remains. At zero it stops and the License tab opens.

To reset the trial while testing, close Tapt, delete that registry value and remove `trial_used_secs` from `%APPDATA%\com.razkom.tapt\config.json`.

### Updates

The installed version is `CARGO_PKG_VERSION`. [`version.rs`](../src-tauri/src/version.rs) reads the latest published release of this repo from the GitHub API (`/repos/RAZKOM/Tapt/releases/latest`), unauthenticated, at startup and every 6 hours, and silently ignores any failure. When it's newer, the settings window shows a banner with **Download** (opens razkom.com/tapt). Drafts and pre-releases don't count.

## Icons

Icons live in [`src-tauri/icons/`](../src-tauri/icons/). To regenerate all sizes from a square master image (overwrites that folder):

```bash
npm run icons:generate -- "C:/path/to/your-square-icon.png"
```

The executable also embeds `icons/32x32.png` for the settings window title bar, where ICO conversion can fail on Windows.

## Permissions

`rdev` uses `SetWindowsHookEx(WH_KEYBOARD_LL)`, which needs no special permission. The overlay window uses `WS_EX_TRANSPARENT | WS_EX_LAYERED | WS_EX_TOOLWINDOW`, so it's click-through and stays off the taskbar.
