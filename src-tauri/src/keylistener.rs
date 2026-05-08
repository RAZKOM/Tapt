//! Global keyboard hook with the **Silent Gate** pattern.
//!
//! `rdev::listen` is a process-wide global hook (only one allowed per
//! process), so the actual `rdev::listen` call lives in a single shared
//! thread spawned by [`crate::listener::ensure_listener_thread`].
//! `start_keylistener` registers with that dispatcher; [`crate::listener::ensure_listener_thread`]
//! owns the `rdev::listen` thread.
//!
//! Events are pushed into a `crossbeam_channel::Sender<InputEvent>` that
//! the eframe overlay thread reads on each frame.

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use crossbeam_channel::Sender;
use rdev::{Event, EventType, Key};

use crate::auth::overlay_license_gate;
use crate::listener::{ensure_listener_thread, register_handler};
use crate::{InputEvent, KeyEvent, SharedState};

pub fn start_keylistener(state: Arc<SharedState>, tx: Sender<InputEvent>) {
    let state_for_handler = state.clone();
    let tx_for_handler = tx.clone();
    register_handler(Box::new(move |event: &Event| {
        // Silent Gate: drop the event entirely if not authenticated.
        if !overlay_license_gate(&state_for_handler) {
            return;
        }
        match event.event_type {
            EventType::KeyPress(key) => {
                let _ =
                    tx_for_handler.try_send(InputEvent::KeyDown(key_to_payload(key)));
            }
            EventType::KeyRelease(key) => {
                let _ =
                    tx_for_handler.try_send(InputEvent::KeyUp(key_to_payload(key)));
            }
            _ => {}
        }
    }));
    ensure_listener_thread();
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn key_to_payload(key: Key) -> KeyEvent {
    let (code, label, is_modifier) = key_metadata(key);
    KeyEvent {
        code: code.into(),
        label: label.into(),
        is_modifier,
        timestamp: now_ms(),
    }
}

/// Returns `(code, label, is_modifier)` for the rdev key. `code` is a
/// stable physical identifier used by the on-screen keyboard renderer to
/// look up which key to highlight; `label` is the display string.
pub fn key_metadata(key: Key) -> (&'static str, &'static str, bool) {
    use Key::*;
    match key {
        // Letters
        KeyA => ("KeyA", "A", false),
        KeyB => ("KeyB", "B", false),
        KeyC => ("KeyC", "C", false),
        KeyD => ("KeyD", "D", false),
        KeyE => ("KeyE", "E", false),
        KeyF => ("KeyF", "F", false),
        KeyG => ("KeyG", "G", false),
        KeyH => ("KeyH", "H", false),
        KeyI => ("KeyI", "I", false),
        KeyJ => ("KeyJ", "J", false),
        KeyK => ("KeyK", "K", false),
        KeyL => ("KeyL", "L", false),
        KeyM => ("KeyM", "M", false),
        KeyN => ("KeyN", "N", false),
        KeyO => ("KeyO", "O", false),
        KeyP => ("KeyP", "P", false),
        KeyQ => ("KeyQ", "Q", false),
        KeyR => ("KeyR", "R", false),
        KeyS => ("KeyS", "S", false),
        KeyT => ("KeyT", "T", false),
        KeyU => ("KeyU", "U", false),
        KeyV => ("KeyV", "V", false),
        KeyW => ("KeyW", "W", false),
        KeyX => ("KeyX", "X", false),
        KeyY => ("KeyY", "Y", false),
        KeyZ => ("KeyZ", "Z", false),

        // Number row
        Num0 => ("Digit0", "0", false),
        Num1 => ("Digit1", "1", false),
        Num2 => ("Digit2", "2", false),
        Num3 => ("Digit3", "3", false),
        Num4 => ("Digit4", "4", false),
        Num5 => ("Digit5", "5", false),
        Num6 => ("Digit6", "6", false),
        Num7 => ("Digit7", "7", false),
        Num8 => ("Digit8", "8", false),
        Num9 => ("Digit9", "9", false),

        // Function row
        F1 => ("F1", "F1", false),
        F2 => ("F2", "F2", false),
        F3 => ("F3", "F3", false),
        F4 => ("F4", "F4", false),
        F5 => ("F5", "F5", false),
        F6 => ("F6", "F6", false),
        F7 => ("F7", "F7", false),
        F8 => ("F8", "F8", false),
        F9 => ("F9", "F9", false),
        F10 => ("F10", "F10", false),
        F11 => ("F11", "F11", false),
        F12 => ("F12", "F12", false),

        // Modifiers — short ASCII payloads; [`overlay_window`] maps codes to glyphs when
        // `use_symbols` is on (still uses ASCII when off for reliable fonts everywhere).
        ShiftLeft => ("ShiftLeft", "Shift", true),
        ShiftRight => ("ShiftRight", "Shift", true),
        ControlLeft => ("ControlLeft", "Ctrl", true),
        ControlRight => ("ControlRight", "Ctrl", true),
        Alt => ("AltLeft", "Alt", true),
        AltGr => ("AltRight", "Alt", true),
        MetaLeft => ("MetaLeft", "Win", true),
        MetaRight => ("MetaRight", "Win", true),
        Function => ("Fn", "Fn", true),
        CapsLock => ("CapsLock", "Caps", true),

        // Punctuation
        BackQuote => ("Backquote", "`", false),
        Minus => ("Minus", "-", false),
        Equal => ("Equal", "=", false),
        LeftBracket => ("BracketLeft", "[", false),
        RightBracket => ("BracketRight", "]", false),
        SemiColon => ("Semicolon", ";", false),
        Quote => ("Quote", "'", false),
        BackSlash => ("Backslash", "\\", false),
        IntlBackslash => ("IntlBackslash", "\\", false),
        Comma => ("Comma", ",", false),
        Dot => ("Period", ".", false),
        Slash => ("Slash", "/", false),

        // Special
        Return => ("Enter", "Enter", false),
        Backspace => ("Backspace", "Bksp", false),
        Tab => ("Tab", "Tab", false),
        Space => ("Space", "Space", false),
        Escape => ("Escape", "Esc", false),

        // Navigation
        UpArrow => ("ArrowUp", "Up", false),
        DownArrow => ("ArrowDown", "Down", false),
        LeftArrow => ("ArrowLeft", "Left", false),
        RightArrow => ("ArrowRight", "Right", false),
        Home => ("Home", "Home", false),
        End => ("End", "End", false),
        PageUp => ("PageUp", "PgUp", false),
        PageDown => ("PageDown", "PgDn", false),
        Insert => ("Insert", "Ins", false),
        Delete => ("Delete", "Del", false),

        // Numpad
        Kp0 => ("Numpad0", "0", false),
        Kp1 => ("Numpad1", "1", false),
        Kp2 => ("Numpad2", "2", false),
        Kp3 => ("Numpad3", "3", false),
        Kp4 => ("Numpad4", "4", false),
        Kp5 => ("Numpad5", "5", false),
        Kp6 => ("Numpad6", "6", false),
        Kp7 => ("Numpad7", "7", false),
        Kp8 => ("Numpad8", "8", false),
        Kp9 => ("Numpad9", "9", false),
        KpReturn => ("NumpadEnter", "Enter", false),
        KpMinus => ("NumpadSubtract", "-", false),
        KpPlus => ("NumpadAdd", "+", false),
        KpMultiply => ("NumpadMultiply", "*", false),
        KpDivide => ("NumpadDivide", "/", false),
        KpDelete => ("NumpadDecimal", ".", false),
        NumLock => ("NumLock", "Num", false),

        // System
        PrintScreen => ("PrintScreen", "PrtSc", false),
        ScrollLock => ("ScrollLock", "ScrLk", false),
        Pause => ("Pause", "Pause", false),

        _ => ("Unknown", "?", false),
    }
}
