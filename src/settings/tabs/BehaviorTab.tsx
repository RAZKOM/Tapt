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
        title="Behavior"
        onReset={() => {
          resetBehaviorDefaults();
          void persistConfig().then(() =>
            applyAutostart(
              useConfigStore.getState().config.behavior.launch_at_login,
            ),
          );
        }}
      />

      <Section title="Startup">
        <Toggle
          label="Start with Windows"
          checked={behavior.launch_at_login}
          onChange={(v) => {
            update("launch_at_login", v);
            void applyAutostart(v);
          }}
        />
        <Toggle
          label="Open settings on startup"
          description="When off, Tapt starts quietly in the system tray."
          checked={behavior.open_settings_on_startup}
          onChange={(v) => update("open_settings_on_startup", v)}
        />
      </Section>

      <Section title="Shortcut">
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
        <div className="kv-hotkey-field__title">Show or hide the overlay</div>
        <p className="kv-hotkey-field__hint">
          Works from any app. Click Record, then press the keys. Esc cancels.
        </p>
      </div>
      <div className="kv-hotkey-field__controls-row">
        <div className="kv-hotkey-field__actions">
          <button
            type="button"
            className={capturing ? "kv-button kv-danger" : "kv-button"}
            onClick={() => setCapturing((c) => !c)}
          >
            {capturing ? "Cancel" : "Record"}
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
              <span className="kv-hotkey-listen-hint">Press a key combination</span>
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
