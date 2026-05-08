import { invoke } from "@tauri-apps/api/core";
import { useEffect } from "react";

import { useConfigStore } from "../store/configStore";
import type { Config } from "../types";
import { DEFAULT_CONFIG } from "../types";

/**
 * Load the live config from the Rust `SharedState` via the `get_config`
 * Tauri command and seed the Zustand store. The Rust side is the single
 * source of truth — it persists to disk via `tauri-plugin-store` and is
 * read by the eframe overlay every frame.
 *
 * Mutation flow: components patch via `useConfigStore.setX` then call
 * `persistConfig()`, which invokes `set_config` on Rust. The overlay
 * picks up the new config on its next ~16ms tick.
 */
export function useConfig() {
  const setConfig = useConfigStore((s) => s.setConfig);
  const markLoaded = useConfigStore((s) => s.markLoaded);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const cfg = await invoke<Config>("get_config");
        if (!cancelled) setConfig(cfg);
      } catch (err) {
        console.warn("[config] get_config failed", err);
        if (!cancelled) setConfig(DEFAULT_CONFIG);
      } finally {
        if (!cancelled) markLoaded();
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [setConfig, markLoaded]);
}

/**
 * Persist the current Zustand config snapshot to Rust. The Rust command
 * writes the in-memory `SharedState.config` (the eframe overlay reads
 * this on every frame) and saves to disk through `tauri-plugin-store`.
 */
export async function persistConfig(): Promise<void> {
  try {
    const snapshot = useConfigStore.getState().config;
    await invoke("set_config", { config: snapshot });
  } catch (err) {
    console.warn("[config] set_config failed", err);
  }
}

/** Convenience: re-export current config + dispatch helpers. */
export function useConfigSnapshot(): Config {
  return useConfigStore((s) => s.config);
}

export async function applyAutostart(enabled: boolean): Promise<void> {
  try {
    if (enabled) {
      await invoke("plugin:autostart|enable");
    } else {
      await invoke("plugin:autostart|disable");
    }
  } catch (err) {
    console.warn("[autostart] failed", err);
  }
}
