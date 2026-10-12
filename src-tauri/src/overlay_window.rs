//! Native eframe-based transparent overlay.
//!
//! Runs on a dedicated thread (separate from Tauri / Tao's main event loop)
//! and receives input events through a `crossbeam_channel` from the rdev
//! listener. Renders the Key Feed pills and the on-screen Keyboard Overlay.
//!
//! # Why this isn't a Tauri/WebView2 window
//!
//! WebView2's compositing pipeline does not reliably honor per-pixel alpha
//! on Windows 11. Even with `transparent: true`, `WS_CLIPCHILDREN` stripped
//! on every host HWND, and explicit `setBackgroundColor([0,0,0,0])` calls,
//! the overlay surface can stay opaque-white (Tauri #8632, #10318). Native eframe+winit
//! uses a standard top-level window; we render with **WGPU** so the framebuffer has a
//! well-defined alpha path to the compositor — Glow/Glutin's first GL choice on Windows
//! can lack alpha and produce an all-invisible buffer on some GPUs.
//!
//! # Threading
//!
//! - **Main thread** runs Tauri's tao event loop (settings window, tray).
//! - **Overlay thread** runs `eframe::run_native` (winit event loop). On
//!   Windows, winit's event loop can run on any thread. It MUST NOT share
//!   a thread with Tauri's tao loop because each loop drives its own Win32
//!   message pump tied to its creator thread.
//! - **rdev dispatch thread** (see `listener.rs`) reads the global hook and
//!   pushes `InputEvent`s into the channel.
//!
//! # State sharing
//!
//! `Arc<SharedState>` is read by the overlay thread on every frame for the
//! authentication gate and live config. Writes happen in `auth.rs` (auth
//! flips) and when the settings UI calls `set_config` (config persistence).

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crossbeam_channel::Receiver;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use eframe::egui::{
    self, Align2, Color32, FontData, FontDefinitions, FontFamily, FontId, Id, LayerId, Order,
    Painter, Pos2, Rect, Rounding, Stroke, Vec2,
};

use crate::auth::overlay_license_gate;
use crate::key_display::{
    caps_lock_engaged, combo_main_key_display, shift_collapses_to_symbol_only,
};
use crate::config::{
    Config, DisplayMode, FeedDirection, FeedFilter, FeedRowDirection, KeyboardAnchor,
    KeyboardLayoutSize, OverlayMonitorRect, SnapCorner, Theme as ConfigTheme,
};
use crate::{InputEvent, KeyEvent, SharedState};

/// Spawn the eframe app. Blocks until the overlay window closes.
/// Called from a dedicated `std::thread::spawn` in `main.rs`.
pub fn run_overlay(
    state: Arc<SharedState>,
    events: Receiver<InputEvent>,
) -> Result<(), String> {
    // Borderless fullscreen covers the full display (including the taskbar region).
    // `maximized` on Windows uses the work area and stops above the taskbar.
    // Windows: we place the HWND with `SetWindowPos` per chosen monitor so the
    // borderless surface matches `rcMonitor`. Generic winit fullscreen targets the
    // platform default display only.
    let viewport = egui::ViewportBuilder::default()
        .with_title("Tapt overlay")
        .with_transparent(true)
        .with_decorations(false)
        .with_resizable(false)
        .with_always_on_top()
        .with_taskbar(false)
        .with_mouse_passthrough(true)
        .with_fullscreen({
            #[cfg(windows)]
            {
                false
            }
            #[cfg(not(windows))]
            {
                true
            }
        });

    let options = eframe::NativeOptions {
        viewport,
        renderer: eframe::Renderer::Wgpu,
        // winit refuses to build an event loop off the main thread on Windows
        // by default. Tauri's tao loop already owns the main thread, so we opt
        // in to multi-threaded event loops via winit's Windows extension.
        // This is documented + supported — the only caveats (hidden taskbar
        // entries, alt-tab quirks) don't apply to a borderless click-through
        // overlay.
        event_loop_builder: Some(Box::new(|builder| {
            use winit::platform::windows::EventLoopBuilderExtWindows;
            builder.with_any_thread(true);
        })),
        persist_window: false,
        ..Default::default()
    };

    eframe::run_native(
        "Tapt overlay",
        options,
        Box::new(move |cc| {
            let initial = state.config.read().appearance.theme;
            cc.egui_ctx
                .set_theme(egui_theme_preference_from_config(initial));
            install_overlay_fonts(&cc.egui_ctx);
            cc.egui_ctx.request_repaint();
            Ok(Box::new(OverlayApp::new(state, events)))
        }),
    )
    .map_err(|e| format!("eframe::run_native failed: {e:?}"))
}

/// **Noto Sans Symbols 2** is tried before **Noto Sans** so modifier / nav glyphs
/// (⇧, ⌃, ⌥, …) resolve: `NotoSans-Regular` often lacks those codepoints while still
/// “claiming” a glyph (tofu), which blocks fallback. Latin text still comes from Noto Sans
/// when Symbols 2 omits basic letters. SIL OFL for both bundles.
fn install_overlay_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        "noto_symbols2".into(),
        FontData::from_static(include_bytes!("../assets/fonts/NotoSansSymbols2-Regular.ttf")),
    );
    fonts.font_data.insert(
        "noto_sans".into(),
        FontData::from_static(include_bytes!("../assets/fonts/NotoSans-Regular.ttf")),
    );
    for family in [FontFamily::Proportional, FontFamily::Monospace] {
        let entry = fonts.families.entry(family).or_default();
        entry.insert(0, "noto_symbols2".into());
        entry.insert(1, "noto_sans".into());
    }
    ctx.set_fonts(fonts);
}

// ============================================================================
// App state
// ============================================================================

struct OverlayApp {
    state: Arc<SharedState>,
    events: Receiver<InputEvent>,
    /// Last `appearance.theme` applied to egui (`set_theme`). Synced when config changes.
    last_config_theme: ConfigTheme,
    /// Active feed entries; oldest at the front, newest at the back.
    feed: VecDeque<FeedEntry>,
    /// `code` -> id of the FeedEntry created for the held modifier, so we
    /// can replace it with a combo entry when a non-modifier follows.
    held_modifiers: HashMap<String, u64>,
    /// Currently pressed key codes (for the on-screen keyboard highlight).
    pressed: HashSet<String>,
    next_id: u64,
}

struct FeedEntry {
    id: u64,
    timestamp: Instant,
    label: String,
    is_modifier: bool,
    is_combo: bool,
    /// Physical key code for simple/modifier rows; `None` for combo strings (no dedupe by key).
    dedupe_key: Option<String>,
}

impl OverlayApp {
    fn new(state: Arc<SharedState>, events: Receiver<InputEvent>) -> Self {
        let last_config_theme = state.config.read().appearance.theme;
        Self {
            state,
            events,
            last_config_theme,
            feed: VecDeque::new(),
            held_modifiers: HashMap::new(),
            pressed: HashSet::new(),
            next_id: 0,
        }
    }

    fn next_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn drain_events(&mut self) {
        while let Ok(event) = self.events.try_recv() {
            match event {
                InputEvent::KeyDown(ke) => self.on_key_down(ke),
                InputEvent::KeyUp(ke) => self.on_key_up(ke),
            }
        }
    }

    fn on_key_down(&mut self, ke: KeyEvent) {
        self.pressed.insert(ke.code.clone());
        let now = Instant::now();
        let id = self.next_id();
        let cfg = self.state.config.read();
        let dedupe_keys = cfg.display.feed_deduplicate_keys;
        let use_symbols = cfg.appearance.use_symbols;
        drop(cfg);

        if ke.is_modifier {
            if dedupe_keys {
                self.feed.retain(|e| e.dedupe_key.as_deref() != Some(ke.code.as_str()));
            }
            self.held_modifiers.insert(ke.code.clone(), id);
            self.feed.push_back(FeedEntry {
                id,
                timestamp: now,
                label: modifier_display(&ke.code, use_symbols).into(),
                is_modifier: true,
                is_combo: false,
                dedupe_key: Some(ke.code.clone()),
            });
        } else if !self.held_modifiers.is_empty() {
            // Combo: drop the individual modifier feed entries and push one
            // combined entry like "Ctrl + Shift + S". Shift-only on the number /
            // punctuation row shows the shifted symbol only (`?`, `!`, …). Shift
            // + letter / F-key / etc. still shows `Shift + F` as a combo pill.
            let modifier_ids: HashSet<u64> =
                self.held_modifiers.values().copied().collect();
            self.feed.retain(|e| !modifier_ids.contains(&e.id));

            let shift_held = self.held_modifiers.contains_key("ShiftLeft")
                || self.held_modifiers.contains_key("ShiftRight");
            let main_label = combo_main_key_display(
                &ke.code,
                shift_held,
                caps_lock_engaged(),
                &ke.label,
            );

            let shift_only = self
                .held_modifiers
                .keys()
                .all(|c| c == "ShiftLeft" || c == "ShiftRight");

            if shift_only && shift_collapses_to_symbol_only(&ke.code) {
                self.feed.push_back(FeedEntry {
                    id,
                    timestamp: now,
                    label: main_label,
                    is_modifier: false,
                    is_combo: false,
                    dedupe_key: Some(ke.code.clone()),
                });
            } else {
                let mut parts: Vec<String> = self
                    .held_modifiers
                    .keys()
                    .map(|c| modifier_display(c, use_symbols).to_string())
                    .collect();
                parts.sort();
                parts.dedup();
                parts.push(main_label);

                self.feed.push_back(FeedEntry {
                    id,
                    timestamp: now,
                    label: parts.join(" + "),
                    is_modifier: false,
                    is_combo: true,
                    dedupe_key: None,
                });
            }
        } else {
            if dedupe_keys {
                self.feed.retain(|e| e.dedupe_key.as_deref() != Some(ke.code.as_str()));
            }
            self.feed.push_back(FeedEntry {
                id,
                timestamp: now,
                label: display_label(&ke.label, use_symbols),
                is_modifier: false,
                is_combo: false,
                dedupe_key: Some(ke.code.clone()),
            });
        }
    }

    fn on_key_up(&mut self, ke: KeyEvent) {
        self.pressed.remove(&ke.code);
        if ke.is_modifier {
            self.held_modifiers.remove(&ke.code);
        }
    }

    fn prune(&mut self, fade_ms: u64) {
        let now = Instant::now();
        let max_age = Duration::from_millis(fade_ms.max(200) + FADE_OUT_MS);
        self.feed
            .retain(|e| now.duration_since(e.timestamp) < max_age);
    }

    fn clear_visual_state(&mut self) {
        while self.events.try_recv().is_ok() {}
        self.feed.clear();
        self.held_modifiers.clear();
        self.pressed.clear();
    }

    #[cfg(windows)]
    fn sync_overlay_monitor(&mut self, frame: &mut eframe::Frame, config: &Config) {
        let Some(hwnd) = win32_hwnd(frame) else {
            return;
        };
        // `SetWindowPos` with `HWND_TOPMOST` only runs in `apply_overlay_monitor_rect`, which
        // is skipped once the outer rect matches the chosen monitor. Winit / the compositor /
        // other topmost windows can still drop `WS_EX_TOPMOST` without changing our size or
        // position — refresh z-order every frame (no move, no size, no activation steal).
        ensure_overlay_topmost(hwnd);

        let target = config
            .position
            .overlay_monitor
            .or_else(|| crate::monitors::primary_monitor_rect());
        let Some(target) = target else {
            return;
        };
        if !overlay_outer_rect_matches_monitor(hwnd, &target) {
            apply_overlay_monitor_rect(hwnd, &target);
        }
    }
}

#[cfg(windows)]
fn overlay_outer_rect_matches_monitor(
    hwnd: windows_sys::Win32::Foundation::HWND,
    target: &OverlayMonitorRect,
) -> bool {
    use windows_sys::Win32::Foundation::RECT;
    use windows_sys::Win32::UI::WindowsAndMessaging::GetWindowRect;

    let mut wr: RECT = unsafe { std::mem::zeroed() };
    unsafe {
        if GetWindowRect(hwnd, &mut wr) == 0 {
            return false;
        }
    }
    let w = (wr.right - wr.left).max(0) as i32;
    let h = (wr.bottom - wr.top).max(0) as i32;
    const EPS: i32 = 4;
    (wr.left - target.left).abs() <= EPS
        && (wr.top - target.top).abs() <= EPS
        && (w - target.width as i32).abs() <= EPS
        && (h - target.height as i32).abs() <= EPS
}

#[cfg(windows)]
fn win32_hwnd(frame: &eframe::Frame) -> Option<windows_sys::Win32::Foundation::HWND> {
    let handle = frame.window_handle().ok()?;
    match handle.as_raw() {
        RawWindowHandle::Win32(w) => Some(w.hwnd.get() as windows_sys::Win32::Foundation::HWND),
        _ => None,
    }
}

#[cfg(windows)]
fn overlay_logical_client_rect(ctx: &egui::Context, hwnd: Option<windows_sys::Win32::Foundation::HWND>) -> egui::Rect {
    use windows_sys::Win32::Foundation::RECT;
    use windows_sys::Win32::UI::HiDpi::GetDpiForWindow;
    use windows_sys::Win32::UI::WindowsAndMessaging::GetClientRect;

    let Some(hwnd) = hwnd else {
        return ctx.screen_rect();
    };
    let mut cr: RECT = unsafe { std::mem::zeroed() };
    let dpi = unsafe {
        if GetClientRect(hwnd, &mut cr) == 0 {
            return ctx.screen_rect();
        }
        GetDpiForWindow(hwnd)
    };
    if dpi == 0 {
        return ctx.screen_rect();
    }
    let cw = (cr.right - cr.left) as f32;
    let ch = (cr.bottom - cr.top) as f32;
    let pixels_per_point = dpi as f32 / 96.0;
    let w = cw / pixels_per_point;
    let h = ch / pixels_per_point;
    if !(w > 0.0 && h > 0.0 && w.is_finite() && h.is_finite()) {
        return ctx.screen_rect();
    }
    egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(w, h))
}

#[cfg(windows)]
fn ensure_overlay_topmost(hwnd: windows_sys::Win32::Foundation::HWND) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SetWindowPos, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW,
    };

    unsafe {
        SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
    }
}

#[cfg(windows)]
fn apply_overlay_monitor_rect(hwnd: windows_sys::Win32::Foundation::HWND, rect: &OverlayMonitorRect) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SetWindowPos, HWND_TOPMOST, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_SHOWWINDOW,
    };

    unsafe {
        SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            rect.left,
            rect.top,
            rect.width as i32,
            rect.height as i32,
            SWP_NOACTIVATE | SWP_SHOWWINDOW | SWP_FRAMECHANGED,
        );
    }
}

impl eframe::App for OverlayApp {
    fn clear_color(&self, _: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }

    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // Force continuous redraw at vsync — the overlay must repaint on every
        // input event without delay, and `request_repaint_after` only schedules
        // a single deferred frame.
        ctx.request_repaint();

        // We need a transparent CentralPanel to absorb hit-testing / focus, but
        // we draw the actual content on the foreground layer with its own
        // painter — that bypasses the panel's clip rect and any layout that
        // egui would apply to widget-tree drawing.
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(Color32::TRANSPARENT))
            .show(ctx, |_| {});

        // Tray toggle: hide all overlay content until the user turns it back on.
        if !self.state.overlay_enabled.load(Ordering::Relaxed) {
            self.clear_visual_state();
            return;
        }

        // Silent gate: key capture and main overlay content only when licensed
        // or the free trial has time left. Running out of trial lands here
        // exactly like an unlicensed session.
        if !overlay_license_gate(&self.state) {
            self.clear_visual_state();
            return;
        }

        let config = self.state.config.read().clone();

        #[cfg(windows)]
        self.sync_overlay_monitor(frame, &config);

        let screen = {
            #[cfg(windows)]
            {
                overlay_logical_client_rect(ctx, win32_hwnd(frame))
            }
            #[cfg(not(windows))]
            {
                ctx.screen_rect()
            }
        };
        let painter = ctx.layer_painter(LayerId::new(Order::Foreground, Id::new("kv-overlay")));

        self.drain_events();

        if config.appearance.theme != self.last_config_theme {
            self.last_config_theme = config.appearance.theme;
            ctx.set_theme(egui_theme_preference_from_config(config.appearance.theme));
        }

        self.prune(config.behavior.fade_ms as u64);

        let ui_theme = ctx.theme();
        let palette = overlay_draw_palette(ui_theme);

        match config.display.mode {
            DisplayMode::KeyFeed => self.draw_feed(&painter, screen, &config, &palette),
            DisplayMode::KeyboardOverlay => {
                self.draw_keyboard(&painter, screen, &config, &palette);
            }
        }
    }
}

// ============================================================================
// Drawing — Key Feed
// ============================================================================

const FEED_PADDING: f32 = 24.0;
const PILL_GAP: f32 = 8.0;
const PILL_PADDING_X: f32 = 14.0;
const FADE_IN_MS: f32 = 80.0;
const FADE_OUT_MS: u64 = 320;

/// Resolved egui light/dark (after `ThemePreference::System` → OS).
#[derive(Clone, Copy)]
struct OverlayDrawPalette {
    feed_pill_bg: Color32,
    feed_label_fg: Color32,
    /// Subtle border / non-modifier single-key stroke.
    feed_muted_stroke: Color32,
    kb_panel: Color32,
    kb_key_bg: Color32,
    kb_key_border: Color32,
    kb_key_label: Color32,
}

fn overlay_draw_palette(ui_theme: egui::Theme) -> OverlayDrawPalette {
    match ui_theme {
        egui::Theme::Dark => OverlayDrawPalette {
            // Alpha 255 so pill_opacity 0–100% maps transparent → fully opaque (same idea as keyboard palette).
            feed_pill_bg: Color32::from_rgba_unmultiplied(20, 22, 32, 255),
            feed_label_fg: Color32::WHITE,
            feed_muted_stroke: Color32::from_rgba_unmultiplied(255, 255, 255, 25),
            kb_panel: Color32::from_rgba_unmultiplied(18, 20, 28, 248),
            kb_key_bg: Color32::from_rgba_unmultiplied(58, 60, 76, 255),
            kb_key_border: Color32::from_rgba_unmultiplied(255, 255, 255, 72),
            kb_key_label: Color32::from_rgba_unmultiplied(255, 255, 255, 255),
        },
        egui::Theme::Light => OverlayDrawPalette {
            feed_pill_bg: Color32::from_rgba_unmultiplied(255, 255, 255, 255),
            feed_label_fg: Color32::from_rgb(28, 32, 48),
            feed_muted_stroke: Color32::from_rgba_unmultiplied(20, 24, 40, 45),
            kb_panel: Color32::from_rgba_unmultiplied(252, 253, 255, 252),
            kb_key_bg: Color32::from_rgba_unmultiplied(234, 236, 244, 255),
            kb_key_border: Color32::from_rgba_unmultiplied(30, 35, 55, 65),
            kb_key_label: Color32::from_rgb(35, 38, 55),
        },
    }
}

fn egui_theme_preference_from_config(theme: ConfigTheme) -> egui::ThemePreference {
    match theme {
        ConfigTheme::Light => egui::ThemePreference::Light,
        ConfigTheme::Dark => egui::ThemePreference::Dark,
        ConfigTheme::System | ConfigTheme::Custom => egui::ThemePreference::System,
    }
}

impl OverlayApp {
    fn draw_feed(
        &self,
        painter: &Painter,
        screen: Rect,
        config: &Config,
        palette: &OverlayDrawPalette,
    ) {
        let now = Instant::now();
        let fade_ms = config.behavior.fade_ms as u64;
        let max_visible = (config.behavior.max_visible_keys as usize).max(1);

        let primary_accent = parse_hex_color(&config.appearance.primary_accent)
            .unwrap_or(Color32::from_rgb(91, 139, 255));
        let modifier_accent = parse_hex_color(&config.appearance.modifier_accent)
            .unwrap_or(Color32::from_rgb(255, 126, 91));
        let pill_bg = palette.feed_pill_bg;
        let label_fg = palette.feed_label_fg;

        let font_size = config.appearance.font_size as f32;
        let pill_height = (font_size * 2.4).max(28.0);
        let radius = config.appearance.corner_radius as f32;
        let opacity = ((config.appearance.pill_opacity.min(100)) as f32 / 100.0).clamp(0.0, 1.0);

        // Newest first, filtered, capped.
        let visible: Vec<&FeedEntry> = self
            .feed
            .iter()
            .rev()
            .filter(|e| passes_filter(e, config.display.feed_filter))
            .take(max_visible)
            .collect();

        let font = FontId::proportional(font_size);

        // Pre-measure pill widths so we can lay them out from the corner.
        let pill_widths: Vec<f32> = visible
            .iter()
            .map(|entry| {
                let label = display_label(&entry.label, config.appearance.use_symbols);
                let galley = painter.layout_no_wrap(label, font.clone(), label_fg);
                galley.rect.width() + PILL_PADDING_X * 2.0
            })
            .collect();

        // Vertical feeds: snap corner picks screen edge and column alignment.
        // Horizontal feeds: snap picks top vs bottom row and whether the cluster
        // hugs the left or right screen edge; `feed_row_direction` only changes
        // whether newest sits on the inner or outer side along that row (cursor math).
        let (
            anchor,
            right_aligned,
            bottom_aligned,
            vertical,
            horizontal_ltr,
            horizontal_right_snap,
        ) = feed_layout(
            screen,
            config.position.snap,
            config.display.feed_direction,
            config.display.feed_row_direction,
            config.position.offset_x as f32,
            config.position.offset_y as f32,
        );

        let n_visible = visible.len();
        let total_row_width: f32 = pill_widths.iter().sum::<f32>()
            + if n_visible > 0 {
                (n_visible.saturating_sub(1)) as f32 * PILL_GAP
            } else {
                0.0
            };

        // Newest pill sits flush against the snap corner; older pills step
        // further into the screen along the flow direction.
        let mut cursor_x = if vertical {
            anchor.x
        } else if horizontal_right_snap {
            if horizontal_ltr {
                anchor.x - total_row_width
            } else {
                anchor.x
            }
        } else if horizontal_ltr {
            anchor.x
        } else {
            anchor.x + total_row_width
        };
        let mut cursor_y = anchor.y;
        for (i, entry) in visible.iter().enumerate() {
            let age = now.duration_since(entry.timestamp);
            let alpha = fade_alpha(age, fade_ms) * opacity;
            if alpha <= 0.0 {
                if vertical {
                    cursor_y += if bottom_aligned {
                        -(pill_height + PILL_GAP)
                    } else {
                        pill_height + PILL_GAP
                    };
                } else {
                    let w = pill_widths[i];
                    cursor_x += if horizontal_ltr {
                        w + PILL_GAP
                    } else {
                        -(w + PILL_GAP)
                    };
                }
                continue;
            }
            let width = pill_widths[i];

            let pill_x = if vertical {
                if right_aligned { anchor.x - width } else { anchor.x }
            } else if horizontal_ltr {
                cursor_x
            } else {
                cursor_x - width
            };
            let pill_y = if !vertical {
                if bottom_aligned { anchor.y - pill_height } else { anchor.y }
            } else if bottom_aligned {
                cursor_y - pill_height
            } else {
                cursor_y
            };

            let rect = Rect::from_min_size(
                Pos2::new(pill_x, pill_y),
                Vec2::new(width, pill_height),
            );

            // Pill border / label: **modifier accent** (often orange) = modifier
            // key alone; **primary accent** (your blue/teal) = shortcut chord
            // (`Ctrl + …`, `Shift + F`); muted stroke = plain single keys / shifted
            // punctuation shown as one symbol (`?`, `!`).
            let (border_color, label_color) = if entry.is_combo {
                (primary_accent, label_fg)
            } else if entry.is_modifier {
                (modifier_accent, modifier_accent)
            } else {
                (palette.feed_muted_stroke, label_fg)
            };

            painter.rect_filled(rect, Rounding::same(radius), multiply_alpha(pill_bg, alpha));
            painter.rect_stroke(
                rect,
                Rounding::same(radius),
                Stroke::new(1.0, multiply_alpha(border_color, alpha)),
            );

            let label = display_label(&entry.label, config.appearance.use_symbols);
            painter.text(
                rect.center(),
                Align2::CENTER_CENTER,
                label,
                font.clone(),
                multiply_alpha(label_color, alpha),
            );

            // Advance the cursor for the NEXT pill in the flow direction.
            if vertical {
                cursor_y += if bottom_aligned {
                    -(pill_height + PILL_GAP)
                } else {
                    pill_height + PILL_GAP
                };
            } else {
                cursor_x += if horizontal_ltr {
                    width + PILL_GAP
                } else {
                    -(width + PILL_GAP)
                };
            }
        }
    }
}

/// Returns `(anchor, right_aligned, bottom_aligned, vertical_flow, horizontal_ltr,
/// horizontal_right_snap)`.
///
/// For horizontal feeds, `horizontal_right_snap` is true when the cluster hugs the
/// screen's **right** edge (`TopRight` / `BottomRight`). Row direction does not
/// move that anchor — it only affects whether newest sits toward the left or
/// right along the row (the horizontal cursor initializer uses row width).
/// For vertical feeds, `horizontal_ltr` and `horizontal_right_snap` are unused.
fn feed_layout(
    screen: Rect,
    snap: SnapCorner,
    direction: FeedDirection,
    row_direction: FeedRowDirection,
    offset_x: f32,
    offset_y: f32,
) -> (Pos2, bool, bool, bool, bool, bool) {
    let pad = FEED_PADDING;
    let vertical = matches!(direction, FeedDirection::Vertical);
    let horizontal_ltr = matches!(row_direction, FeedRowDirection::LeftToRight);

    if vertical {
        let (anchor, right_aligned, bottom_aligned) = match snap {
            SnapCorner::TopLeft => (
                Pos2::new(screen.left() + pad + offset_x, screen.top() + pad + offset_y),
                false,
                false,
            ),
            SnapCorner::TopRight => (
                Pos2::new(screen.right() - pad - offset_x, screen.top() + pad + offset_y),
                true,
                false,
            ),
            SnapCorner::BottomLeft => (
                Pos2::new(screen.left() + pad + offset_x, screen.bottom() - pad - offset_y),
                false,
                true,
            ),
            SnapCorner::BottomRight => (
                Pos2::new(screen.right() - pad - offset_x, screen.bottom() - pad - offset_y),
                true,
                true,
            ),
            SnapCorner::Free => (
                Pos2::new(screen.left() + offset_x, screen.top() + offset_y),
                false,
                false,
            ),
        };
        return (
            anchor,
            right_aligned,
            bottom_aligned,
            true,
            horizontal_ltr,
            false,
        );
    }

    let bottom_aligned = matches!(
        snap,
        SnapCorner::BottomLeft | SnapCorner::BottomRight
    );
    let horizontal_right_snap = matches!(
        snap,
        SnapCorner::TopRight | SnapCorner::BottomRight
    );
    let anchor = match snap {
        SnapCorner::Free => Pos2::new(screen.left() + offset_x, screen.top() + offset_y),
        _ => {
            let y = if bottom_aligned {
                screen.bottom() - pad - offset_y
            } else {
                screen.top() + pad + offset_y
            };
            let x = if horizontal_right_snap {
                screen.right() - pad - offset_x
            } else {
                screen.left() + pad + offset_x
            };
            Pos2::new(x, y)
        }
    };
    (
        anchor,
        false,
        bottom_aligned,
        false,
        horizontal_ltr,
        horizontal_right_snap,
    )
}

/// Top-left `(x, y)` of the keyboard's outer rounded panel (`panel_rect.min`).
fn keyboard_panel_min(
    screen: Rect,
    anchor: KeyboardAnchor,
    panel_w: f32,
    panel_h: f32,
    off_x: f32,
    off_y: f32,
) -> (f32, f32) {
    let edge = FEED_PADDING;
    let sx0 = screen.left();
    let sy0 = screen.top();
    let sx1 = screen.right();
    let sy1 = screen.bottom();
    let cx = screen.center().x;

    match anchor {
        KeyboardAnchor::Free => (
            sx0 + off_x,
            sy0 + off_y,
        ),
        KeyboardAnchor::TopLeft => (
            sx0 + edge + off_x,
            sy0 + edge + off_y,
        ),
        KeyboardAnchor::TopCenter => (
            cx - panel_w / 2.0 + off_x,
            sy0 + edge + off_y,
        ),
        KeyboardAnchor::TopRight => (
            sx1 - edge - panel_w - off_x,
            sy0 + edge + off_y,
        ),
        KeyboardAnchor::MiddleLeft => (
            sx0 + edge + off_x,
            screen.center().y - panel_h / 2.0 + off_y,
        ),
        KeyboardAnchor::Center => (
            cx - panel_w / 2.0 + off_x,
            screen.center().y - panel_h / 2.0 + off_y,
        ),
        KeyboardAnchor::MiddleRight => (
            sx1 - edge - panel_w - off_x,
            screen.center().y - panel_h / 2.0 + off_y,
        ),
        KeyboardAnchor::BottomLeft => (
            sx0 + edge + off_x,
            sy1 - edge - panel_h - off_y,
        ),
        KeyboardAnchor::BottomCenter => (
            cx - panel_w / 2.0 + off_x,
            sy1 - edge - panel_h - off_y,
        ),
        KeyboardAnchor::BottomRight => (
            sx1 - edge - panel_w - off_x,
            sy1 - edge - panel_h - off_y,
        ),
    }
}

// ============================================================================
// Drawing — Keyboard Overlay
// ============================================================================

impl OverlayApp {
    fn draw_keyboard(
        &self,
        painter: &Painter,
        screen: Rect,
        config: &Config,
        palette: &OverlayDrawPalette,
    ) {
        let primary_accent = parse_hex_color(&config.appearance.primary_accent)
            .unwrap_or(Color32::from_rgb(91, 139, 255));
        let modifier_accent = parse_hex_color(&config.appearance.modifier_accent)
            .unwrap_or(Color32::from_rgb(255, 126, 91));

        let scale = (config.display.keyboard_scale as f32 / 100.0).clamp(0.5, 1.5);
        let inactive_opacity = ((config.display.keyboard_inactive_opacity.min(100)) as f32 / 100.0)
            .clamp(0.0, 1.0);
        let active_opacity = ((config.display.keyboard_active_opacity.min(100)) as f32 / 100.0)
            .clamp(0.0, 1.0);
        // Dark panel uses inactive opacity only so it matches the idle keyboard layer.
        let panel_opacity = inactive_opacity;
        if inactive_opacity <= 0.0 && active_opacity <= 0.0 {
            return;
        }

        let unit = 40.0 * scale;
        let gap = 4.0 * scale;
        let pad = 12.0 * scale;
        let radius = 6.0 * scale;
        let panel_radius = 14.0 * scale;

        let layout = build_layout(config.display.keyboard_layout);

        // Each cluster is laid out independently; they sit side-by-side.
        let cluster_widths: Vec<f32> =
            layout.iter().map(|c| cluster_width(c, unit, gap)).collect();
        let cluster_heights: Vec<f32> =
            layout.iter().map(|c| cluster_height(c, unit, gap)).collect();
        let cluster_gap = 18.0 * scale;

        let total_width = cluster_widths.iter().sum::<f32>()
            + cluster_gap * (layout.len().saturating_sub(1) as f32);
        let total_height = cluster_heights.iter().cloned().fold(0.0_f32, f32::max);

        let panel_w = total_width + pad * 2.0;
        let panel_h = total_height + pad * 2.0;
        let (panel_min_x, panel_min_y) = keyboard_panel_min(
            screen,
            config.display.keyboard_anchor,
            panel_w,
            panel_h,
            config.display.keyboard_offset_x as f32,
            config.display.keyboard_offset_y as f32,
        );
        let kb_x = panel_min_x + pad;
        let kb_y = panel_min_y + pad;

        let panel_rect =
            Rect::from_min_size(Pos2::new(panel_min_x, panel_min_y), Vec2::new(panel_w, panel_h));
        // Palette at slider=100%; outer opacity scales RGBA uniformly.
        let panel_base = palette.kb_panel;
        let key_bg_base = palette.kb_key_bg;
        let key_border_base = palette.kb_key_border;
        let key_label_base = palette.kb_key_label;

        let panel_color = multiply_alpha(panel_base, panel_opacity);
        if panel_opacity > 0.0 {
            painter.rect_filled(panel_rect, Rounding::same(panel_radius), panel_color);
        }

        let mut x_cursor = kb_x;
        for (cluster, cw) in layout.iter().zip(cluster_widths.iter()) {
            let mut y = kb_y;
            for row in cluster {
                let mut x = x_cursor;
                for key in row {
                    let w = unit * key.width;
                    if !key.spacer {
                        let rect =
                            Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, unit));
                        let pressed = self.pressed.contains(key.code);

                        let (bg, border, label_color) = if pressed && key.is_modifier {
                            (
                                multiply_alpha(modifier_accent, active_opacity),
                                multiply_alpha(modifier_accent, active_opacity),
                                multiply_alpha(Color32::WHITE, active_opacity),
                            )
                        } else if pressed {
                            (
                                multiply_alpha(primary_accent, active_opacity),
                                multiply_alpha(primary_accent, active_opacity),
                                multiply_alpha(Color32::WHITE, active_opacity),
                            )
                        } else {
                            (
                                multiply_alpha(key_bg_base, inactive_opacity),
                                multiply_alpha(key_border_base, inactive_opacity),
                                multiply_alpha(key_label_base, inactive_opacity),
                            )
                        };

                        painter.rect_filled(rect, Rounding::same(radius), bg);
                        painter.rect_stroke(rect, Rounding::same(radius), Stroke::new(1.0, border));
                        let cap = display_label(key.label, config.appearance.use_symbols);
                        painter.text(
                            rect.center(),
                            Align2::CENTER_CENTER,
                            cap,
                            FontId::proportional((unit * 0.32).max(10.0)),
                            label_color,
                        );
                    }
                    x += w + gap;
                }
                y += unit + gap;
            }
            x_cursor += cw + cluster_gap;
        }
    }
}

// ============================================================================
// Helpers
// ============================================================================

fn fade_alpha(age: Duration, hold_ms: u64) -> f32 {
    let ms = age.as_millis() as f32;
    if ms < FADE_IN_MS {
        (ms / FADE_IN_MS).clamp(0.0, 1.0)
    } else if ms < FADE_IN_MS + hold_ms as f32 {
        1.0
    } else {
        let t = (ms - FADE_IN_MS - hold_ms as f32) / FADE_OUT_MS as f32;
        (1.0 - t).clamp(0.0, 1.0)
    }
}

fn multiply_alpha(c: Color32, a: f32) -> Color32 {
    let alpha = (c.a() as f32 * a.clamp(0.0, 1.0)) as u8;
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), alpha)
}

fn parse_hex_color(s: &str) -> Option<Color32> {
    let s = s.trim_start_matches('#');
    if s.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(s.get(0..2)?, 16).ok()?;
    let g = u8::from_str_radix(s.get(2..4)?, 16).ok()?;
    let b = u8::from_str_radix(s.get(4..6)?, 16).ok()?;
    Some(Color32::from_rgb(r, g, b))
}

fn passes_filter(entry: &FeedEntry, filter: FeedFilter) -> bool {
    match filter {
        FeedFilter::All => true,
        FeedFilter::ModifiersAndCombos => entry.is_modifier || entry.is_combo,
        FeedFilter::CombosOnly => entry.is_combo,
    }
}

/// Labels for the overlay: with `use_symbols`, map short ASCII names from
/// [`crate::keylistener::key_metadata`] (and the keyboard layout) to the same
/// Unicode symbols we strip when symbols are off. Combo strings built from
/// [`modifier_display`] pass through untouched.
///
/// Symbols must stay in sync with [`display_label`]'s `use_symbols == false` branch.
fn display_label(label: &str, use_symbols: bool) -> String {
    if use_symbols {
        return match label {
            // Glyph codepoints mirror **Noto Sans Symbols 2** `cmap` (avoid tofu).
            "Esc" => "\u{241b}".into(),
            "Tab" => "\u{2b7e}".into(),
            "Enter" => "\u{23ce}".into(),
            "Bksp" => "\u{232b}".into(),
            "Up" => "\u{2b06}".into(),
            "Down" => "\u{2b07}".into(),
            "Left" => "\u{2b05}".into(),
            "Right" => "\u{2b95}".into(),
            _ => label.to_string(),
        };
    }
    label
        .replace('⇧', "Shift")
        .replace('⌃', "Ctrl")
        .replace('\u{2324}', "Ctrl") // ⌤ substituted for ⌃ — see [`modifier_display`]
        .replace('⌥', "Alt")
        .replace('⌘', "Cmd")
        .replace('⊞', "Win")
        .replace('⇪', "Caps")
        .replace('\u{241b}', "Esc") // ␛ SYMBOL FOR ESCAPE
        .replace('\u{2b7e}', "Tab") // ⭾ HORIZONTAL TAB KEY
        .replace('\u{2409}', "Tab") // ␉ legacy control-picture tab
        .replace('\u{21a9}', "Enter")
        .replace('\u{23ce}', "Enter") // ⏎ RETURN SYMBOL
        .replace('↩', "Enter")
        .replace('⌫', "Bksp")
        .replace('⇥', "Tab")
        .replace('\u{21e5}', "Tab")
        .replace('⎋', "Esc")
        .replace('\u{238b}', "Esc")
        .replace('↑', "Up")
        .replace('↓', "Down")
        .replace('←', "Left")
        .replace('→', "Right")
        .replace('\u{2b06}', "Up") // Symbols2-heavy arrows used in glyph mode
        .replace('\u{2b07}', "Down")
        .replace('\u{2b05}', "Left")
        .replace('\u{2b95}', "Right")
}

/// Stable short text for modifiers when `use_symbols` is false; Unicode glyphs
/// when true (paired with [`display_label`]'s stripping pass).
fn modifier_glyph(code: &str) -> &'static str {
    match code {
        "ShiftLeft" | "ShiftRight" => "Shift",
        "ControlLeft" | "ControlRight" => "Ctrl",
        "AltLeft" | "AltRight" => "Alt",
        "MetaLeft" | "MetaRight" => "Win",
        "CapsLock" => "Caps",
        "Fn" => "Fn",
        _ => "?",
    }
}

#[inline]
fn modifier_display(code: &str, use_symbols: bool) -> &'static str {
    if !use_symbols {
        return modifier_glyph(code);
    }
    match code {
        "ShiftLeft" | "ShiftRight" => "\u{21e7}",
        // `U+2324` ⌤ — not Apple’s ⌃ (`U+2303`), but Symbols 2 outlines it; ⌃ renders as tofu.
        "ControlLeft" | "ControlRight" => "\u{2324}",
        "AltLeft" | "AltRight" => "\u{2325}",
        // Match Unicode cleanup in [`display_label`] (`⊞`, U+229E).
        "MetaLeft" | "MetaRight" => "\u{229e}",
        "CapsLock" => "\u{21ea}",
        "Fn" => "Fn",
        _ => "?",
    }
}

// ============================================================================
// Keyboard layout data
// ============================================================================

#[derive(Clone, Copy)]
struct KeyDef {
    code: &'static str,
    label: &'static str,
    /// Width as a multiple of `unit` (1.0u standard key, 1.5u Tab, etc.).
    width: f32,
    is_modifier: bool,
    /// `true` for invisible spacers between key clusters / arrow rows.
    spacer: bool,
}

const fn k(code: &'static str, label: &'static str) -> KeyDef {
    KeyDef {
        code,
        label,
        width: 1.0,
        is_modifier: false,
        spacer: false,
    }
}
const fn kw(code: &'static str, label: &'static str, width: f32) -> KeyDef {
    KeyDef {
        code,
        label,
        width,
        is_modifier: false,
        spacer: false,
    }
}
const fn km(code: &'static str, label: &'static str, width: f32) -> KeyDef {
    KeyDef {
        code,
        label,
        width,
        is_modifier: true,
        spacer: false,
    }
}
const fn ks(width: f32) -> KeyDef {
    KeyDef {
        code: "",
        label: "",
        width,
        is_modifier: false,
        spacer: true,
    }
}

type Row = Vec<KeyDef>;
type Cluster = Vec<Row>;

fn build_layout(size: KeyboardLayoutSize) -> Vec<Cluster> {
    let mut clusters = vec![main_cluster(size)];
    if matches!(size, KeyboardLayoutSize::Tkl | KeyboardLayoutSize::Full) {
        clusters.push(nav_arrow_cluster());
    }
    if matches!(size, KeyboardLayoutSize::Full) {
        clusters.push(numpad_cluster());
    }
    clusters
}

fn main_cluster(size: KeyboardLayoutSize) -> Cluster {
    let mut cluster: Cluster = Vec::new();

    if matches!(size, KeyboardLayoutSize::Tkl | KeyboardLayoutSize::Full) {
        cluster.push(vec![
            k("Escape", "Esc"),
            ks(0.5),
            k("F1", "F1"),
            k("F2", "F2"),
            k("F3", "F3"),
            k("F4", "F4"),
            ks(0.25),
            k("F5", "F5"),
            k("F6", "F6"),
            k("F7", "F7"),
            k("F8", "F8"),
            ks(0.25),
            k("F9", "F9"),
            k("F10", "F10"),
            k("F11", "F11"),
            k("F12", "F12"),
        ]);
    }

    cluster.push(vec![
        k("Backquote", "`"),
        k("Digit1", "1"),
        k("Digit2", "2"),
        k("Digit3", "3"),
        k("Digit4", "4"),
        k("Digit5", "5"),
        k("Digit6", "6"),
        k("Digit7", "7"),
        k("Digit8", "8"),
        k("Digit9", "9"),
        k("Digit0", "0"),
        k("Minus", "-"),
        k("Equal", "="),
        kw("Backspace", "Bksp", 2.0),
    ]);

    cluster.push(vec![
        kw("Tab", "Tab", 1.5),
        k("KeyQ", "Q"),
        k("KeyW", "W"),
        k("KeyE", "E"),
        k("KeyR", "R"),
        k("KeyT", "T"),
        k("KeyY", "Y"),
        k("KeyU", "U"),
        k("KeyI", "I"),
        k("KeyO", "O"),
        k("KeyP", "P"),
        k("BracketLeft", "["),
        k("BracketRight", "]"),
        kw("Backslash", "\\", 1.5),
    ]);

    cluster.push(vec![
        km("CapsLock", "Caps", 1.75),
        k("KeyA", "A"),
        k("KeyS", "S"),
        k("KeyD", "D"),
        k("KeyF", "F"),
        k("KeyG", "G"),
        k("KeyH", "H"),
        k("KeyJ", "J"),
        k("KeyK", "K"),
        k("KeyL", "L"),
        k("Semicolon", ";"),
        k("Quote", "'"),
        kw("Enter", "Enter", 2.25),
    ]);

    cluster.push(vec![
        km("ShiftLeft", "Shift", 2.25),
        k("KeyZ", "Z"),
        k("KeyX", "X"),
        k("KeyC", "C"),
        k("KeyV", "V"),
        k("KeyB", "B"),
        k("KeyN", "N"),
        k("KeyM", "M"),
        k("Comma", ","),
        k("Period", "."),
        k("Slash", "/"),
        km("ShiftRight", "Shift", 2.75),
    ]);

    cluster.push(vec![
        km("ControlLeft", "Ctrl", 1.25),
        km("MetaLeft", "Win", 1.25),
        km("AltLeft", "Alt", 1.25),
        kw("Space", "Space", 6.25),
        km("AltRight", "Alt", 1.25),
        km("MetaRight", "Win", 1.25),
        km("ControlRight", "Ctrl", 1.25),
    ]);

    cluster
}

fn nav_arrow_cluster() -> Cluster {
    vec![
        vec![
            k("PrintScreen", "PrtSc"),
            k("ScrollLock", "ScrLk"),
            k("Pause", "Pause"),
        ],
        vec![k("Insert", "Ins"), k("Home", "Home"), k("PageUp", "PgUp")],
        vec![k("Delete", "Del"), k("End", "End"), k("PageDown", "PgDn")],
        // Spacer row to align with QWERTY/ASDF gap above the arrow cluster.
        vec![ks(3.0)],
        vec![ks(1.0), k("ArrowUp", "Up"), ks(1.0)],
        vec![
            k("ArrowLeft", "Left"),
            k("ArrowDown", "Down"),
            k("ArrowRight", "Right"),
        ],
    ]
}

fn numpad_cluster() -> Cluster {
    vec![
        vec![
            k("NumLock", "Num"),
            k("NumpadDivide", "/"),
            k("NumpadMultiply", "*"),
            k("NumpadSubtract", "-"),
        ],
        vec![
            k("Numpad7", "7"),
            k("Numpad8", "8"),
            k("Numpad9", "9"),
            kw("NumpadAdd", "+", 1.0),
        ],
        vec![k("Numpad4", "4"), k("Numpad5", "5"), k("Numpad6", "6")],
        vec![
            k("Numpad1", "1"),
            k("Numpad2", "2"),
            k("Numpad3", "3"),
            kw("NumpadEnter", "Enter", 1.0),
        ],
        vec![
            kw("Numpad0", "0", 2.0),
            k("NumpadDecimal", "."),
        ],
    ]
}

fn cluster_width(cluster: &Cluster, unit: f32, gap: f32) -> f32 {
    cluster
        .iter()
        .map(|row| row_width(row, unit, gap))
        .fold(0.0_f32, f32::max)
}

fn cluster_height(cluster: &Cluster, unit: f32, gap: f32) -> f32 {
    if cluster.is_empty() {
        return 0.0;
    }
    cluster.len() as f32 * unit + gap * (cluster.len().saturating_sub(1) as f32)
}

fn row_width(row: &Row, unit: f32, gap: f32) -> f32 {
    let key_w: f32 = row.iter().map(|k| k.width * unit).sum();
    let gap_w = gap * (row.len().saturating_sub(1) as f32);
    key_w + gap_w
}
