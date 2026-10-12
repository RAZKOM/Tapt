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

      <Section
        title="Colors"
        description="The modifier color is used for Shift, Ctrl, Alt and Win."
      >
        <ColorPicker
          label="Accent color"
          value={appearance.primary_accent}
          onChange={(v) => patchAppearance("primary_accent", v)}
        />
        <ColorPicker
          label="Modifier color"
          value={appearance.modifier_accent}
          onChange={(v) => patchAppearance("modifier_accent", v)}
        />
      </Section>

      <Section title="Key labels">
        <Toggle
          label="Use symbols"
          description="Show ⇧ and ⌃ instead of Shift and Ctrl."
          checked={appearance.use_symbols}
          onChange={(v) => patchAppearance("use_symbols", v)}
        />
      </Section>

      {isPills && (
        <>
          <Section title="Key feed">
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
                label="New keys appear"
                value={display.feed_row_direction}
                onChange={(v) => patchDisplay("feed_row_direction", v)}
                options={[
                  { value: "left-to-right", label: "On the left" },
                  { value: "right-to-left", label: "On the right" },
                ]}
              />
            )}
            <Select
              label="Keys to show"
              value={display.feed_filter}
              onChange={(v) => patchDisplay("feed_filter", v)}
              options={[
                { value: "all", label: "All keys" },
                {
                  value: "modifiers-and-combos",
                  label: "Modifier keys and combinations",
                },
                { value: "combos-only", label: "Key combinations only" },
              ]}
            />
            <Toggle
              label="Show each key once"
              description="Pressing a key again updates it instead of adding another."
              checked={display.feed_deduplicate_keys}
              onChange={(v) => patchDisplay("feed_deduplicate_keys", v)}
            />
            <Slider
              label="Keys shown at once"
              min={1}
              max={10}
              value={behavior.max_visible_keys}
              onChange={(v) => patchBehavior("max_visible_keys", v)}
            />
            <Slider
              label="Fade out after"
              min={500}
              max={5000}
              step={100}
              value={behavior.fade_ms}
              onChange={(v) => patchBehavior("fade_ms", v)}
              formatter={(n) => `${(n / 1000).toFixed(1)} s`}
            />
          </Section>

          <Section title="Style">
            <Slider
              label="Text size"
              min={12}
              max={32}
              value={appearance.font_size}
              onChange={(v) => patchAppearance("font_size", v)}
              formatter={(n) => `${n}px`}
            />
            <Slider
              label="Background opacity"
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
          </Section>
        </>
      )}

      {isKeyboard && (
        <Section title="Keyboard">
          <Select
            label="Layout"
            value={display.keyboard_layout}
            onChange={(v) => patchDisplay("keyboard_layout", v)}
            options={[
              { value: "sixty", label: "60%" },
              { value: "tkl", label: "Tenkeyless" },
              { value: "full", label: "Full size" },
            ]}
          />
          <Slider
            label="Size"
            min={50}
            max={150}
            value={display.keyboard_scale}
            onChange={(v) => patchDisplay("keyboard_scale", v)}
            formatter={(n) => `${n}%`}
          />
          <Slider
            label="Key opacity"
            min={0}
            max={100}
            value={display.keyboard_inactive_opacity}
            onChange={(v) => patchDisplay("keyboard_inactive_opacity", v)}
            formatter={(n) => `${n}%`}
          />
          <Slider
            label="Pressed key opacity"
            min={0}
            max={100}
            value={display.keyboard_active_opacity}
            onChange={(v) => patchDisplay("keyboard_active_opacity", v)}
            formatter={(n) => `${n}%`}
          />
        </Section>
      )}
    </div>
  );
}
