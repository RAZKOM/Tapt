import { invoke } from "@tauri-apps/api/core";
import { useEffect, useMemo, useState } from "react";

import { persistConfig } from "../../hooks/useConfig";
import { useConfigStore } from "../../store/configStore";
import type { DisplayConfig, MonitorInfo, OverlayMonitorRect } from "../../types";
import { KEYBOARD_ANCHORS } from "../keyboardAnchors";
import {
  canNudge,
  clampInt,
  deltaFeedOffsetX,
  deltaFeedOffsetY,
  deltaKeyboardOffsetX,
  deltaKeyboardOffsetY,
} from "../offsetNudge";
import NudgeCrossRow from "../components/NudgeCrossRow";
import Section from "../components/Section";
import Select from "../components/Select";
import Slider from "../components/Slider";
import TabResetBar from "../components/TabResetBar";
import Toggle from "../components/Toggle";

const PRIMARY_VALUE = "__primary__";

function rectKey(r: OverlayMonitorRect): string {
  return `${r.left}\t${r.top}\t${r.width}\t${r.height}`;
}

function monitorToRect(m: MonitorInfo): OverlayMonitorRect {
  return {
    left: m.left,
    top: m.top,
    width: m.width,
    height: m.height,
  };
}

function rectMatchesSaved(m: MonitorInfo, saved: OverlayMonitorRect | null): boolean {
  if (!saved) return false;
  return (
    m.left === saved.left &&
    m.top === saved.top &&
    m.width === saved.width &&
    m.height === saved.height
  );
}

export default function PositionTab() {
  const display = useConfigStore((s) => s.config.display);
  const position = useConfigStore((s) => s.config.position);
  const setDisplay = useConfigStore((s) => s.setDisplay);
  const setPosition = useConfigStore((s) => s.setPosition);
  const resetPositionDefaults = useConfigStore((s) => s.resetPositionDefaults);
  const [monitors, setMonitors] = useState<MonitorInfo[] | null>(null);
  const [monitorLoadError, setMonitorLoadError] = useState<string | null>(null);

  const isPills = display.mode === "key-feed";
  const isKeyboard = display.mode === "keyboard-overlay";

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const list = await invoke<MonitorInfo[]>("list_monitors");
        if (!cancelled) {
          setMonitorLoadError(null);
          setMonitors(list);
        }
      } catch (err) {
        if (!cancelled) {
          setMonitors(null);
          setMonitorLoadError(
            err instanceof Error ? err.message : String(err),
          );
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  function updatePosition<K extends keyof typeof position>(
    key: K,
    value: (typeof position)[K],
  ) {
    setPosition({ [key]: value } as Partial<typeof position>);
    void persistConfig();
  }

  function patchDisplay<K extends keyof DisplayConfig>(
    key: K,
    value: DisplayConfig[K],
  ) {
    setDisplay({ [key]: value } as Partial<DisplayConfig>);
    void persistConfig();
  }

  const selectedMonitorKey = useMemo(() => {
    const om = position.overlay_monitor;
    if (!om) return PRIMARY_VALUE;
    return rectKey(om);
  }, [position.overlay_monitor]);

  const monitorSelectOptions = useMemo(() => {
    const list = monitors ?? [];
    const opts: { value: string; label: string }[] = [
      {
        value: PRIMARY_VALUE,
        label: "Primary display (default)",
      },
    ];
    for (let i = 0; i < list.length; i++) {
      const m = list[i]!;
        const shortName = m.device_name.replace(/^.*\\/, "");
      opts.push({
        value: rectKey(monitorToRect(m)),
        label: `Display ${i + 1}: ${m.width}×${m.height}${m.is_primary ? " — primary" : ""} (${shortName})`,
      });
    }
    const cur = position.overlay_monitor;
    if (
      cur &&
      !list.some((m) => rectMatchesSaved(m, cur))
    ) {
      opts.push({
        value: rectKey(cur),
        label: `Saved display (${cur.width}×${cur.height}) — not detected now`,
      });
    }
    return opts;
  }, [monitors, position.overlay_monitor]);

  function onMonitorSelect(key: string) {
    if (key === PRIMARY_VALUE) {
      updatePosition("overlay_monitor", null);
      return;
    }
    const [ls, ts, ws, hs] = key.split("\t");
    const left = Number(ls);
    const top = Number(ts);
    const width = Number(ws);
    const height = Number(hs);
    if ([left, top, width, height].some((n) => Number.isNaN(n))) return;
    updatePosition("overlay_monitor", { left, top, width, height });
  }

  const schematic = useMemo(() => {
    const list = monitors ?? [];
    if (list.length <= 1) return null;
    let minL = Infinity;
    let minT = Infinity;
    let maxR = -Infinity;
    let maxB = -Infinity;
    for (const m of list) {
      minL = Math.min(minL, m.left);
      minT = Math.min(minT, m.top);
      maxR = Math.max(maxR, m.left + m.width);
      maxB = Math.max(maxB, m.top + m.height);
    }
    const vw = Math.max(1, maxR - minL);
    const vh = Math.max(1, maxB - minT);
    const pad = 6;
    const boxW = 280;
    const boxH = 140;
    const innerW = boxW - pad * 2;
    const innerH = boxH - pad * 2;
    const scale = Math.min(innerW / vw, innerH / vh);
    const contentW = vw * scale;
    const contentH = vh * scale;
    // Center silhouette when virtual-desktop aspect ≠ preview box (avoids uneven empty margins).
    const originX = pad + Math.max(0, (innerW - contentW) / 2);
    const originY = pad + Math.max(0, (innerH - contentH) / 2);
    return { list, minL, minT, scale, originX, originY, boxW, boxH };
  }, [monitors]);

  return (
    <div>
      <TabResetBar
        title="Position"
        onReset={() => {
          resetPositionDefaults();
          void persistConfig();
        }}
      />

      <Section
        title="Overlay display"
        description="Monitors come from Windows in virtual-desktop coordinates (same spatial layout as in Display Settings). The preview scales that layout to fit the box below and centers it — relative positions match your desk; margin inside the preview is symmetric."
      >
        {monitorLoadError != null ? (
          <p style={{ color: "var(--kv-settings-danger, #f87171)", margin: 0 }}>
            Could not list displays ({monitorLoadError}). Try restarting the
            app.
          </p>
        ) : monitors === null ? (
          <p style={{ color: "var(--kv-settings-muted)", margin: 0 }}>
            Detecting displays…
          </p>
        ) : monitors.length <= 1 ? (
          <p style={{ color: "var(--kv-settings-muted)", margin: 0 }}>
            Only one display was reported by the system. If you use multiple
            monitors, check cabling / “Extend these displays” in Windows
            settings.
          </p>
        ) : (
          <>
            {schematic && (
              <div
                style={{
                  marginBottom: 16,
                  width: schematic.boxW,
                  height: schematic.boxH,
                  position: "relative",
                  background: "var(--kv-settings-card)",
                  border: "1px solid var(--kv-settings-border)",
                  borderRadius: 8,
                  overflow: "hidden",
                }}
              >
                {schematic.list.map((m, i) => {
                  const selected = rectMatchesSaved(m, position.overlay_monitor);
                  const isDefault =
                    !position.overlay_monitor && m.is_primary;
                  const highlight = selected || isDefault;
                  return (
                    <button
                      key={`${m.left},${m.top},${i}`}
                      type="button"
                      title={`Display ${i + 1}${m.is_primary ? " (primary)" : ""}`}
                      onClick={() => onMonitorSelect(rectKey(monitorToRect(m)))}
                      style={{
                        position: "absolute",
                        left:
                          schematic.originX +
                          (m.left - schematic.minL) * schematic.scale,
                        top:
                          schematic.originY +
                          (m.top - schematic.minT) * schematic.scale,
                        width: m.width * schematic.scale,
                        height: m.height * schematic.scale,
                        margin: 0,
                        padding: 0,
                        border: highlight
                          ? "2px solid var(--kv-settings-accent, #04cfe5)"
                          : "1px solid var(--kv-settings-border)",
                        borderRadius: 4,
                        background: highlight
                          ? "color-mix(in srgb, var(--kv-settings-accent, #04cfe5) 22%, transparent)"
                          : "color-mix(in srgb, var(--kv-settings-fg) 8%, transparent)",
                        cursor: "pointer",
                        boxSizing: "border-box",
                      }}
                    />
                  );
                })}
              </div>
            )}
            <Select
              label="Monitor"
              value={selectedMonitorKey}
              onChange={onMonitorSelect}
              options={monitorSelectOptions}
            />
          </>
        )}
      </Section>

      {isPills && (
        <p style={{ color: "var(--kv-settings-muted)" }}>
          Snap the key feed pills to a screen corner or place them freely.
        </p>
      )}

      {isKeyboard && (
        <p style={{ color: "var(--kv-settings-muted)" }}>
          Anchor the keyboard panel on screen and nudge it with pixel offsets.
        </p>
      )}

      {isPills && (
        <>
          <Section title="Snap (key feed)">
            <Select
              label="Corner"
              value={position.snap}
              onChange={(v) => updatePosition("snap", v)}
              options={[
                { value: "top-left", label: "Top Left" },
                { value: "top-right", label: "Top Right" },
                { value: "bottom-left", label: "Bottom Left" },
                { value: "bottom-right", label: "Bottom Right" },
                { value: "free", label: "Free / manual" },
              ]}
            />
          </Section>

          <Section title="Offset (key feed)">
            <div style={{ display: "flex", alignItems: "center", gap: 24 }}>
              <div style={{ flex: 1, display: "flex", flexDirection: "column", gap: 12 }}>
                <Slider
                  label="Offset X"
                  min={0}
                  max={400}
                  value={position.offset_x}
                  onChange={(v) => updatePosition("offset_x", v)}
                  formatter={(n) => `${n}px`}
                />
                <Slider
                  label="Offset Y"
                  min={0}
                  max={400}
                  value={position.offset_y}
                  onChange={(v) => updatePosition("offset_y", v)}
                  formatter={(n) => `${n}px`}
                />
              </div>
              <div
                style={{
                  flexShrink: 0,
                  display: "flex",
                  flexDirection: "column",
                  justifyContent: "center",
                  height: "100%",
                }}
              >
                <NudgeCrossRow
                  horizontalDisabled={[
                    !canNudge(
                      position.offset_x,
                      deltaFeedOffsetX("left", position.snap),
                      0,
                      400,
                    ),
                    !canNudge(
                      position.offset_x,
                      deltaFeedOffsetX("right", position.snap),
                      0,
                      400,
                    ),
                  ]}
                  verticalDisabled={[
                    !canNudge(
                      position.offset_y,
                      deltaFeedOffsetY("up", position.snap),
                      0,
                      400,
                    ),
                    !canNudge(
                      position.offset_y,
                      deltaFeedOffsetY("down", position.snap),
                      0,
                      400,
                    ),
                  ]}
                  onHorizontal={(dir) => {
                    const d = deltaFeedOffsetX(dir, position.snap);
                    const n = clampInt(position.offset_x + d, 0, 400);
                    if (n !== position.offset_x) updatePosition("offset_x", n);
                  }}
                  onVertical={(dir) => {
                    const d = deltaFeedOffsetY(dir, position.snap);
                    const n = clampInt(position.offset_y + d, 0, 400);
                    if (n !== position.offset_y) updatePosition("offset_y", n);
                  }}
                />
              </div>
      
            </div>
    
          </Section>
        </>
      )}

      {isKeyboard && (
        <>
          <Section title="Snap (keyboard overlay)">
            <Select
              label="Anchor"
              value={display.keyboard_anchor}
              onChange={(v) => patchDisplay("keyboard_anchor", v)}
              options={KEYBOARD_ANCHORS}
            />
          </Section>

          <Section
            title="Offset (keyboard overlay)"
            description={
              display.keyboard_anchor === "free"
                ? "Offsets are pixels from the top-left of the overlay (full screen)."
                : "Nudge from the anchored spot in pixels — positive vertical values lift the panel up from bottom-aligned edges."
            }
          >
            <div style={{ display: "flex", alignItems: "center", gap: 24 }}>
              <div style={{ flex: 1, display: "flex", flexDirection: "column", gap: 12 }}>
                <Slider
                  label="Fine-tune horizontal (px)"
                  min={-400}
                  max={400}
                  value={display.keyboard_offset_x}
                  onChange={(v) => patchDisplay("keyboard_offset_x", v)}
                  formatter={(n) => `${n}px`}
                />
                <Slider
                  label="Fine-tune vertical (px)"
                  min={-400}
                  max={400}
                  value={display.keyboard_offset_y}
                  onChange={(v) => patchDisplay("keyboard_offset_y", v)}
                  formatter={(n) => `${n}px`}
                />
              </div>
              <div
                style={{
                  flexShrink: 0,
                  display: "flex",
                  flexDirection: "column",
                  justifyContent: "center",
                  height: "100%",
                }}
              >
                <NudgeCrossRow
                  horizontalDisabled={[
                    !canNudge(
                      display.keyboard_offset_x,
                      deltaKeyboardOffsetX("left", display.keyboard_anchor),
                      -400,
                      400,
                    ),
                    !canNudge(
                      display.keyboard_offset_x,
                      deltaKeyboardOffsetX("right", display.keyboard_anchor),
                      -400,
                      400,
                    ),
                  ]}
                  verticalDisabled={[
                    !canNudge(
                      display.keyboard_offset_y,
                      deltaKeyboardOffsetY("up", display.keyboard_anchor),
                      -400,
                      400,
                    ),
                    !canNudge(
                      display.keyboard_offset_y,
                      deltaKeyboardOffsetY("down", display.keyboard_anchor),
                      -400,
                      400,
                    ),
                  ]}
                  onHorizontal={(dir) => {
                    const d = deltaKeyboardOffsetX(dir, display.keyboard_anchor);
                    const n = clampInt(display.keyboard_offset_x + d, -400, 400);
                    if (n !== display.keyboard_offset_x) {
                      patchDisplay("keyboard_offset_x", n);
                    }
                  }}
                  onVertical={(dir) => {
                    const d = deltaKeyboardOffsetY(dir, display.keyboard_anchor);
                    const n = clampInt(display.keyboard_offset_y + d, -400, 400);
                    if (n !== display.keyboard_offset_y) {
                      patchDisplay("keyboard_offset_y", n);
                    }
                  }}
                />
              </div>
            </div>
          </Section>
        </>
      )}

      <Section title="Persistence">
        <Toggle
          label="Remember overlay position settings between launches"
          checked={position.remember_position}
          onChange={(v) => updatePosition("remember_position", v)}
        />
      </Section>
    </div>
  );
}
