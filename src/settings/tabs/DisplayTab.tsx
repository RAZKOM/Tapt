import { persistConfig } from "../../hooks/useConfig";
import { useConfigStore } from "../../store/configStore";
import type { DisplayConfig } from "../../types";
import ColorPicker from "../components/ColorPicker";
import Section from "../components/Section";
import Select from "../components/Select";
import Slider from "../components/Slider";
import TabResetBar from "../components/TabResetBar";
import Toggle from "../components/Toggle";

export default function DisplayTab() {
  const appearance = useConfigStore((s) => s.config.appearance);
  const behavior = useConfigStore((s) => s.config.behavior);
  const display = useConfigStore((s) => s.config.display);
  const setAppearance = useConfigStore((s) => s.setAppearance);
  const setBehavior = useConfigStore((s) => s.setBehavior);
  const setDisplay = useConfigStore((s) => s.setDisplay);
  const resetAppearanceDefaults = useConfigStore(
    (s) => s.resetAppearanceDefaults,
  );

  const isPills = display.mode === "key-feed";
  const isKeyboard = display.mode === "keyboard-overlay";

  function patchAppearance<K extends keyof typeof appearance>(
    key: K,
    value: (typeof appearance)[K],
  ) {
    setAppearance({ [key]: value } as Partial<typeof appearance>);
    void persistConfig();
  }

  function patchDisplay<K extends keyof DisplayConfig>(
    key: K,
    value: DisplayConfig[K],
  ) {
    setDisplay({ [key]: value } as Partial<DisplayConfig>);
    void persistConfig();
  }

  function patchBehavior<K extends keyof typeof behavior>(
    key: K,
    value: (typeof behavior)[K],
  ) {
    setBehavior({ [key]: value } as Partial<typeof behavior>);
    void persistConfig();
  }

  return (
    <div>
      <TabResetBar
        title="Appearance"
        onReset={() => {
          resetAppearanceDefaults();
          void persistConfig();
        }}
      />
      <p style={{ color: "var(--kv-settings-muted)" }}>
        Use the sidebar for overlay mode (pills vs keyboard) and bright vs dark
        theme — it follows your system until you tap Light or Dark. Accents
        below tweak highlight colours for both.
      </p>

      <Section
        title="Overlay colours"
        description="Primary and modifier accents for the key feed, keyboard overlay, and highlight tones in this UI."
      >
        <ColorPicker
          label="Primary accent"
          value={appearance.primary_accent}
          onChange={(v) => patchAppearance("primary_accent", v)}
        />
        <ColorPicker
          label="Modifier accent"
          value={appearance.modifier_accent}
          onChange={(v) => patchAppearance("modifier_accent", v)}
        />
      </Section>

      {isPills && (
        <>
          <Section
            title="Animation"
            description="How long keys stay visible in the feed and how many pills can show at once."
          >
            <Slider
              label="Key fade duration"
              min={500}
              max={5000}
              step={100}
              value={behavior.fade_ms}
              onChange={(v) => patchBehavior("fade_ms", v)}
              formatter={(n) => `${n} ms`}
            />
            <Slider
              label="Max visible keys"
              min={1}
              max={10}
              value={behavior.max_visible_keys}
              onChange={(v) => patchBehavior("max_visible_keys", v)}
            />
          </Section>
          <Section
            title="Key feed layout"
            description="Where pills stack and which keys qualify."
          >
            <Select
              label="Direction"
              value={display.feed_direction}
              onChange={(v) => patchDisplay("feed_direction", v)}
              options={[
                { value: "vertical", label: "Vertical" },
                { value: "horizontal", label: "Horizontal" },
              ]}
            />
            {display.feed_direction === "horizontal" && (
              <Select
                label="Along row"
                value={display.feed_row_direction}
                onChange={(v) => patchDisplay("feed_row_direction", v)}
                options={[
                  {
                    value: "left-to-right",
                    label: "Left to right (newest at the left edge)",
                  },
                  {
                    value: "right-to-left",
                    label: "Right to left (newest at the right edge)",
                  },
                ]}
              />
            )}
            <Toggle
              label="Deduplicate keys"
              description="Only one pill per physical key at a time (e.g. A, D, A shows A once). Turn off to show every press as its own pill."
              checked={display.feed_deduplicate_keys}
              onChange={(v) => patchDisplay("feed_deduplicate_keys", v)}
            />
            <Select
              label="Filter"
              value={display.feed_filter}
              onChange={(v) => patchDisplay("feed_filter", v)}
              options={[
                { value: "all", label: "All keys" },
                {
                  value: "modifiers-and-combos",
                  label: "Modifiers & combos only",
                },
                { value: "combos-only", label: "Combos only" },
              ]}
            />
          </Section>

          <Section title="Pill styling">
            <Slider
              label="Font size"
              min={12}
              max={32}
              value={appearance.font_size}
              onChange={(v) => patchAppearance("font_size", v)}
              formatter={(n) => `${n}px`}
            />
            <Slider
              label="Pill opacity"
              min={0}
              max={100}
              value={appearance.pill_opacity}
              onChange={(v) => patchAppearance("pill_opacity", v)}
              formatter={(n) => `${n}%`}
            />
            <Slider
              label="Corner radius"
              min={2}
              max={16}
              value={appearance.corner_radius}
              onChange={(v) => patchAppearance("corner_radius", v)}
              formatter={(n) => `${n}px`}
            />
            <Toggle
              label="Use key symbols"
              description="Key feed & keyboard overlay: on = modifier / nav glyphs (⇧, ⌃, …); off = short words (Shift, Ctrl, …). Fonts fall back automatically if a glyph cannot be drawn."
              checked={appearance.use_symbols}
              onChange={(v) => patchAppearance("use_symbols", v)}
            />
            <Toggle
              label="Backdrop blur (frosted glass)"
              description="Applied to pills when supported."
              checked={appearance.backdrop_blur}
              onChange={(v) => patchAppearance("backdrop_blur", v)}
            />
          </Section>
        </>
      )}

      {isKeyboard && (
        <>
          <Section
            title="Keyboard layout"
            description="Inactive keys are unpressed; active keys are currently held. The dark panel uses inactive opacity only."
          >
            <Select
              label="Hardware layout"
              value={display.keyboard_layout}
              onChange={(v) => patchDisplay("keyboard_layout", v)}
              options={[
                { value: "sixty", label: "60% (alphas + modifiers)" },
                { value: "tkl", label: "TKL / Tenkeyless" },
                { value: "full", label: "Full (with numpad)" },
              ]}
            />
            <Slider
              label="Inactive keys opacity"
              min={0}
              max={100}
              value={display.keyboard_inactive_opacity}
              onChange={(v) => patchDisplay("keyboard_inactive_opacity", v)}
              formatter={(n) => `${n}%`}
            />
            <Slider
              label="Active (pressed) keys opacity"
              min={0}
              max={100}
              value={display.keyboard_active_opacity}
              onChange={(v) => patchDisplay("keyboard_active_opacity", v)}
              formatter={(n) => `${n}%`}
            />
            <Slider
              label="Scale"
              min={50}
              max={150}
              value={display.keyboard_scale}
              onChange={(v) => patchDisplay("keyboard_scale", v)}
              formatter={(n) => `${n}%`}
            />
          </Section>
        </>
      )}
    </div>
  );
}
