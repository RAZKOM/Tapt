import { create } from "zustand";

import type {
  AppearanceConfig,
  BehaviorConfig,
  Config,
  DisplayConfig,
  PositionConfig,
} from "../types";
import { DEFAULT_CONFIG } from "../types";

interface ConfigState {
  config: Config;
  loaded: boolean;
  setConfig: (next: Config) => void;
  setAppearance: (patch: Partial<AppearanceConfig>) => void;
  setDisplay: (patch: Partial<DisplayConfig>) => void;
  setBehavior: (patch: Partial<BehaviorConfig>) => void;
  setPosition: (patch: Partial<PositionConfig>) => void;
  /** Appearance tab covers theme + overlays + feeds + keyboard sizing; fades live in Behavior. */
  resetAppearanceDefaults: () => void;
  resetBehaviorDefaults: () => void;
  /** Position tab: monitors + snaps + feeds + keyboard placement + persistence. */
  resetPositionDefaults: () => void;
  markLoaded: () => void;
}

export const useConfigStore = create<ConfigState>((set) => ({
  config: DEFAULT_CONFIG,
  loaded: false,
  setConfig: (config) => set({ config }),
  setAppearance: (patch) =>
    set((s) => ({
      config: { ...s.config, appearance: { ...s.config.appearance, ...patch } },
    })),
  setDisplay: (patch) =>
    set((s) => ({
      config: { ...s.config, display: { ...s.config.display, ...patch } },
    })),
  setBehavior: (patch) =>
    set((s) => ({
      config: { ...s.config, behavior: { ...s.config.behavior, ...patch } },
    })),
  setPosition: (patch) =>
    set((s) => ({
      config: { ...s.config, position: { ...s.config.position, ...patch } },
    })),
  resetAppearanceDefaults: () =>
    set((s) => ({
      config: {
        ...s.config,
        appearance: { ...DEFAULT_CONFIG.appearance },
        display: { ...DEFAULT_CONFIG.display },
        behavior: {
          ...s.config.behavior,
          fade_ms: DEFAULT_CONFIG.behavior.fade_ms,
          max_visible_keys: DEFAULT_CONFIG.behavior.max_visible_keys,
        },
      },
    })),
  resetBehaviorDefaults: () =>
    set((s) => ({
      config: { ...s.config, behavior: { ...DEFAULT_CONFIG.behavior } },
    })),
  resetPositionDefaults: () =>
    set((s) => ({
      config: {
        ...s.config,
        position: { ...DEFAULT_CONFIG.position },
        display: {
          ...s.config.display,
          keyboard_anchor: DEFAULT_CONFIG.display.keyboard_anchor,
          keyboard_offset_x: DEFAULT_CONFIG.display.keyboard_offset_x,
          keyboard_offset_y: DEFAULT_CONFIG.display.keyboard_offset_y,
        },
      },
    })),
  markLoaded: () => set({ loaded: true }),
}));
