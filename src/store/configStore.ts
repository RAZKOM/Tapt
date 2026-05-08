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
  markLoaded: () => set({ loaded: true }),
}));
