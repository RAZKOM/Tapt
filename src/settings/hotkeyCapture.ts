/**
 * Build a string understood by `global-hotkey` / tauri-plugin-global-shortcut.
 * @see https://docs.rs/global-hotkey/latest/global_hotkey/hotkey/struct.HotKey.html
 */

const MODIFIER_CODES = new Set([
  "ControlLeft",
  "ControlRight",
  "ShiftLeft",
  "ShiftRight",
  "AltLeft",
  "AltRight",
  "MetaLeft",
  "MetaRight",
]);

const ENGINE_MODIFIER_TOKENS = new Set(["Ctrl", "Alt", "Shift", "Super"]);

const NUMPAD_TAIL_LABELS: Record<string, string> = {
  Add: "+",
  Subtract: "-",
  Multiply: "*",
  Divide: "/",
  Decimal: ".",
  Enter: "Enter",
  Equal: "=",
};

const NAMED_KEY_LABELS: Record<string, string> = {
  Space: "Space",
  Enter: "Enter",
  Tab: "Tab",
  Backspace: "Bksp",
  Escape: "Esc",
  Delete: "Del",
  Insert: "Ins",
  Home: "Home",
  End: "End",
  PageUp: "PgUp",
  PageDown: "PgDn",
  ArrowUp: "Up",
  ArrowDown: "Down",
  ArrowLeft: "Left",
  ArrowRight: "Right",
  CapsLock: "Caps",
  PrintScreen: "PrtSc",
  ScrollLock: "ScrLk",
  Pause: "Pause",
  ContextMenu: "Menu",
};

const BRACKET_STYLE_LABELS: Record<string, string> = {
  BracketLeft: "[",
  BracketRight: "]",
  Backslash: "\\",
  Semicolon: ";",
  Quote: "'",
  Comma: ",",
  Period: ".",
  Slash: "/",
  Backquote: "`",
  Minus: "-",
  Equal: "=",
  IntlBackslash: "\\",
};

/** macOS only: Apple-style glyphs; Windows/WebView2 often lacks coverage for ⇧ ⌃ ⌥. */
function useSymbolicModifierGlyphs(): boolean {
  return platformIsMac();
}

function mapEngineTokenToLabel(tok: string, isMac: boolean): string {
  if (tok === "Super") {
    return isMac ? "⌘" : "Win";
  }
  if (tok === "Ctrl") {
    return useSymbolicModifierGlyphs() ? "⌃" : "Ctrl";
  }
  if (tok === "Alt") {
    return useSymbolicModifierGlyphs() ? "⌥" : "Alt";
  }
  if (tok === "Shift") {
    return useSymbolicModifierGlyphs() ? "⇧" : "Shift";
  }

  const keyMatch = /^Key([A-Z])$/.exec(tok);
  if (keyMatch) {
    return keyMatch[1]!;
  }
  const digitMatch = /^Digit(\d)$/.exec(tok);
  if (digitMatch) {
    return digitMatch[1]!;
  }

  const numpadMatch = /^Numpad(.+)$/.exec(tok);
  if (numpadMatch) {
    const rest = numpadMatch[1]!;
    if (/^\d$/.test(rest)) {
      return `Num ${rest}`;
    }
    const named = NUMPAD_TAIL_LABELS[rest] ?? NAMED_KEY_LABELS[rest] ?? rest;
    return `Num ${named}`;
  }

  const fnMatch = /^F(\d{1,2})$/.exec(tok);
  if (fnMatch) {
    return `F${fnMatch[1]}`;
  }

  if (NAMED_KEY_LABELS[tok]) {
    return NAMED_KEY_LABELS[tok];
  }
  if (BRACKET_STYLE_LABELS[tok]) {
    return BRACKET_STYLE_LABELS[tok];
  }

  return tok;
}

function platformIsMac(): boolean {
  return (
    typeof navigator !== "undefined" &&
    /Mac|iPhone|iPod|iPad/i.test(navigator.platform)
  );
}

export interface HotkeyDisplaySegment {
  label: string;
  modifier: boolean;
}

/** One visual key per chord segment (for keycap UI). */
export function hotkeyDisplaySegments(spec: string): HotkeyDisplaySegment[] {
  const isMac = platformIsMac();
  return spec
    .split("+")
    .filter(Boolean)
    .map((tok) => ({
      label: mapEngineTokenToLabel(tok, isMac),
      modifier: ENGINE_MODIFIER_TOKENS.has(tok),
    }));
}

/**
 * Returns `null` until a non-modifier key is pressed with the current chord.
 * `Escape` clears capture (handled by UI). Ignores repeat events.
 */
export function hotkeyChordFromKeyboardEvent(
  e: KeyboardEvent,
): string | null {
  if (e.repeat) {
    return null;
  }
  if (MODIFIER_CODES.has(e.code)) {
    return null;
  }

  const mods: string[] = [];
  if (e.ctrlKey) {
    mods.push("Ctrl");
  }
  if (e.altKey) {
    mods.push("Alt");
  }
  if (e.shiftKey) {
    mods.push("Shift");
  }
  if (e.metaKey) {
    mods.push("Super");
  }
  mods.push(e.code);
  return mods.join("+");
}

/** Pretty single-line label (still stores engine format). */
export function formatHotkeyForDisplay(spec: string): string {
  const isMac = platformIsMac();
  const parts = hotkeyDisplaySegments(spec).map((s) => s.label);
  return parts.join(isMac ? "" : "+");
}
