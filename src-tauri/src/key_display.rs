//! Resolves the **printed character** for a physical key when building modifier
//! combos (e.g. Shift + `/` → `?`). On Windows we ask the active keyboard layout
//! via `ToUnicodeEx`; elsewhere we use a US QWERTY shift/caps table.

#[cfg(windows)]
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyboardLayout, GetKeyState, MapVirtualKeyExW, ToUnicodeEx, MAPVK_VK_TO_VSC,
    VK_CAPITAL, VK_SHIFT,
};

/// Keys where Shift + key is treated as **typography** (`!`, `?`, …): the feed
/// shows the shifted character alone. Letters, F-keys, space, etc. keep a full
/// `Shift + …` chord.
pub fn shift_collapses_to_symbol_only(code: &str) -> bool {
    if let Some(d) = code.strip_prefix("Digit") {
        return d.len() == 1 && d.chars().next().is_some_and(|c| c.is_ascii_digit());
    }
    matches!(
        code,
        "Backquote"
            | "Minus"
            | "Equal"
            | "BracketLeft"
            | "BracketRight"
            | "Backslash"
            | "IntlBackslash"
            | "Semicolon"
            | "Quote"
            | "Comma"
            | "Period"
            | "Slash"
    )
}

/// Whether Caps Lock is toggled on (Windows only; other platforms always false).
pub fn caps_lock_engaged() -> bool {
    #[cfg(windows)]
    {
        win_caps_lock()
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// Label for the non-modifier key in a combo. **Shift + typing row**
/// (`123…`, `` `-=[]…`, etc.) maps to the shifted symbol (`!@#…`, `?`, …) using
/// the active layout on Windows. **Letter keys** keep the usual key-cap label
/// from the listener (uppercase) so Shift + L still shows `L`, not `l`.
pub fn combo_main_key_display(
    code: &str,
    shift_held: bool,
    caps_lock_on: bool,
    fallback_label: &str,
) -> String {
    if physical_code_is_letter(code) {
        return fallback_label.to_string();
    }

    let vk = physical_code_to_vk(code);
    let Some(vk) = vk else {
        return combo_string_us_qwerty(code, shift_held)
            .unwrap_or_else(|| fallback_label.to_string());
    };

    #[cfg(windows)]
    if let Some(s) = win_to_unicode_label(vk, shift_held, caps_lock_on) {
        if combo_label_is_useful(&s) {
            return s;
        }
    }

    combo_string_us_qwerty(code, shift_held)
        .unwrap_or_else(|| fallback_label.to_string())
}

fn combo_label_is_useful(s: &str) -> bool {
    !s.is_empty() && !s.chars().all(|c| c.is_control())
}

fn combo_string_us_qwerty(code: &str, shift: bool) -> Option<String> {
    if shift {
        punctuation_shift_us(code)
    } else {
        None
    }
}

fn physical_code_is_letter(code: &str) -> bool {
    let Some(rest) = code.strip_prefix("Key") else {
        return false;
    };
    let mut it = rest.chars();
    let Some(c) = it.next() else {
        return false;
    };
    if it.next().is_some() {
        return false;
    }
    c.is_ascii_uppercase()
}

fn punctuation_shift_us(code: &str) -> Option<String> {
    let ch = match code {
        "Digit0" => ')',
        "Digit1" => '!',
        "Digit2" => '@',
        "Digit3" => '#',
        "Digit4" => '$',
        "Digit5" => '%',
        "Digit6" => '^',
        "Digit7" => '&',
        "Digit8" => '*',
        "Digit9" => '(',
        "Backquote" => '~',
        "Minus" => '_',
        "Equal" => '+',
        "BracketLeft" => '{',
        "BracketRight" => '}',
        "Backslash" => '|',
        "IntlBackslash" => '|',
        "Semicolon" => ':',
        "Quote" => '"',
        "Comma" => '<',
        "Period" => '>',
        "Slash" => '?',
        _ => return None,
    };
    Some(ch.to_string())
}

/// Maps Web-style `KeyboardEvent.code` values to Win32 virtual-key codes
/// (same scan positions as typical US 104-key hardware).
fn physical_code_to_vk(code: &str) -> Option<u32> {
    if let Some(rest) = code.strip_prefix("Key") {
        let mut ch = rest.chars();
        let c = ch.next()?;
        if ch.next().is_some() {
            return None;
        }
        return if c.is_ascii_uppercase() {
            Some(u32::from(c))
        } else {
            None
        };
    }
    if let Some(d) = code.strip_prefix("Digit") {
        let n: u32 = d.parse().ok()?;
        return (n <= 9).then_some(0x30 + n);
    }

    match code {
        "Backquote" => Some(0xC0),
        "Minus" => Some(0xBD),
        "Equal" => Some(0xBB),
        "BracketLeft" => Some(0xDB),
        "BracketRight" => Some(0xDD),
        "Backslash" => Some(0xDC),
        "IntlBackslash" => Some(0xE2),
        "Semicolon" => Some(0xBA),
        "Quote" => Some(0xDE),
        "Comma" => Some(0xBC),
        "Period" => Some(0xBE),
        "Slash" => Some(0xBF),

        "Space" => Some(0x20),

        "Numpad0" => Some(0x60),
        "Numpad1" => Some(0x61),
        "Numpad2" => Some(0x62),
        "Numpad3" => Some(0x63),
        "Numpad4" => Some(0x64),
        "Numpad5" => Some(0x65),
        "Numpad6" => Some(0x66),
        "Numpad7" => Some(0x67),
        "Numpad8" => Some(0x68),
        "Numpad9" => Some(0x69),
        "NumpadMultiply" => Some(0x6A),
        "NumpadAdd" => Some(0x6B),
        "NumpadSubtract" => Some(0x6D),
        "NumpadDecimal" => Some(0x6E),
        "NumpadDivide" => Some(0x6F),

        _ => None,
    }
}

#[cfg(windows)]
fn win_caps_lock() -> bool {
    unsafe { (GetKeyState(i32::from(VK_CAPITAL)) & 1) != 0 }
}

#[cfg(windows)]
fn win_to_unicode_label(vk: u32, shift: bool, caps: bool) -> Option<String> {
    unsafe {
        let hkl = GetKeyboardLayout(0);
        let sc = MapVirtualKeyExW(vk, MAPVK_VK_TO_VSC, hkl);
        let mut state = [0u8; 256];
        if shift {
            state[usize::from(VK_SHIFT)] = 0x80;
        }
        if caps {
            state[usize::from(VK_CAPITAL)] |= 0x01;
        }

        let mut buf = [0u16; 8];
        let n = ToUnicodeEx(
            vk,
            sc,
            state.as_ptr(),
            buf.as_mut_ptr(),
            i32::try_from(buf.len()).unwrap_or(8),
            0,
            hkl,
        );
        if n <= 0 {
            return None;
        }
        let n = usize::try_from(n).ok()?;
        String::from_utf16(&buf[..n]).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn us_shift_slash_is_question() {
        assert_eq!(
            combo_main_key_display("Slash", true, true, "/"),
            "?"
        );
    }

    #[test]
    fn shift_collapse_only_on_number_and_oem_row() {
        assert!(shift_collapses_to_symbol_only("Digit1"));
        assert!(shift_collapses_to_symbol_only("Slash"));
        assert!(!shift_collapses_to_symbol_only("KeyF"));
        assert!(!shift_collapses_to_symbol_only("F1"));
    }

    #[test]
    fn shift_letter_uses_keycap_label_not_typed_case() {
        assert_eq!(
            combo_main_key_display("KeyL", true, false, "L"),
            "L"
        );
        assert_eq!(
            combo_main_key_display("KeyL", true, true, "L"),
            "L"
        );
    }
}
