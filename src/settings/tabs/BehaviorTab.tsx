import { useCallback, useEffect, useState } from "react";

import { persistConfig, applyAutostart } from "../../hooks/useConfig";
import { useConfigStore } from "../../store/configStore";
import {
  hotkeyChordFromKeyboardEvent,
  hotkeyDisplaySegments,
} from "../hotkeyCapture";
import Section from "../components/Section";
import TabResetBar from "../components/TabResetBar";
import Toggle from "../components/Toggle";

export default function BehaviorTab() {
  const behavior = useConfigStore((s) => s.config.behavior);
  const setBehavior = useConfigStore((s) => s.setBehavior);
  const resetBehaviorDefaults = useConfigStore((s) => s.resetBehaviorDefaults);

  function update<K extends keyof typeof behavior>(
    key: K,
    value: (typeof behavior)[K],
  ) {
    setBehavior({ [key]: value } as Partial<typeof behavior>);
    void persistConfig();
  }

  return (
    <div>
      <TabResetBar
        title="Behaviour"
        onReset={() => {
          resetBehaviorDefaults();
          void persistConfig().then(() =>
            applyAutostart(
              useConfigStore.getState().config.behavior.launch_at_login,
            ),
          );
        }}
      />
      <p style={{ color: "var(--kv-settings-muted)" }}>
        Startup, autostart, and a system-wide shortcut to show or hide the
        overlay.
      </p>

      <Section title="System">
        <Toggle
          label="Open settings when the app starts"
          description="When off, only the tray icon and overlay run until you open Settings from the tray."
          checked={behavior.open_settings_on_startup}
          onChange={(v) => update("open_settings_on_startup", v)}
        />
        <Toggle
          label="Launch at login"
          checked={behavior.launch_at_login}
          onChange={(v) => {
            update("launch_at_login", v);
            void applyAutostart(v);
          }}
        />
        <HotkeyField
          value={behavior.global_toggle_hotkey}
          onChange={(v) => update("global_toggle_hotkey", v)}
        />
      </Section>
    </div>
  );
}

interface HotkeyFieldProps {
  value: string | null;
  onChange: (next: string | null) => void;
}

function HotkeyField({ value, onChange }: HotkeyFieldProps) {
  const [capturing, setCapturing] = useState(false);

  const stopCapture = useCallback(() => {
    setCapturing(false);
  }, []);

  useEffect(() => {
    if (!capturing) {
      return undefined;
    }

    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        stopCapture();
        return;
      }
      e.preventDefault();
      e.stopPropagation();
      const chord = hotkeyChordFromKeyboardEvent(e);
      if (!chord) {
        return;
      }
      onChange(chord);
      stopCapture();
    };

    window.addEventListener("keydown", onKeyDown, true);
    return () => window.removeEventListener("keydown", onKeyDown, true);
  }, [capturing, onChange, stopCapture]);

  const segments =
    value != null && value.trim() !== "" ? hotkeyDisplaySegments(value) : [];

  return (
    <div className="kv-hotkey-field">
      <div className="kv-hotkey-field__copy">
        <div className="kv-hotkey-field__title">Global toggle hotkey</div>
        <p className="kv-hotkey-field__hint">
          Press the shortcut while recording — same format as system global
          hotkeys. Esc cancels capture.
        </p>
        {value && !capturing && (
          <div className="kv-hotkey-field__engine">
            Engine: <code>{value}</code>
          </div>
        )}
      </div>
      <div className="kv-hotkey-field__controls-row">
        <div className="kv-hotkey-field__actions">
          <button
            type="button"
            className={capturing ? "kv-button kv-danger" : "kv-button"}
            onClick={() => setCapturing((c) => !c)}
          >
            {capturing ? "Cancel" : "Record shortcut"}
          </button>
          <button
            type="button"
            className="kv-button kv-secondary"
            disabled={!value || capturing}
            onClick={() => onChange(null)}
          >
            Clear
          </button>
        </div>
        <div className="kv-hotkey-field__preview">
          {capturing ? (
            <div
              className="kv-hotkey-keycaps kv-hotkey-keycaps--listen"
              role="status"
            >
              <kbd className="kv-keycap kv-keycap--listening">Listening…</kbd>
              <span className="kv-hotkey-listen-hint">Press keys</span>
            </div>
          ) : (
            <div
              className={`kv-hotkey-keycaps${
                segments.length === 0 ? " kv-hotkey-keycaps--empty" : ""
              }`}
              aria-label={
                segments.length === 0
                  ? "No shortcut set"
                  : `Shortcut: ${segments.map((s) => s.label).join(" ")}`
              }
            >
              {segments.length === 0 ? (
                <span className="kv-keycap kv-keycap--placeholder">None</span>
              ) : (
                segments.map((seg, i) => (
                  <kbd
                    key={`${seg.label}-${i}`}
                    className={
                      seg.modifier
                        ? "kv-keycap kv-keycap--modifier"
                        : "kv-keycap"
                    }
                  >
                    {seg.label}
                  </kbd>
                ))
              )}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
