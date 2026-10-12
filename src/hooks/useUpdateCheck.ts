import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useState } from "react";

import type { UpdateStatus } from "../types";

const RESYNC_INTERVAL_MS = 15 * 60_000;

/**
 * Compare installed version (`CARGO_PKG_VERSION`) with the latest version
 * Rust fetched from the latest `RAZKOM/Tapt.` GitHub release. Independent of the
 * license and the trial.
 */
export function useUpdateCheck() {
  const [status, setStatus] = useState<UpdateStatus | null>(null);

  const refresh = useCallback(async () => {
    try {
      const next = await invoke<UpdateStatus>("get_update_status");
      setStatus(next);
    } catch {
      setStatus(null);
    }
  }, []);

  useEffect(() => {
    let unlistenFn: (() => void) | null = null;

    void (async () => {
      await refresh();

      const off = await listen<UpdateStatus>("update-status-changed", (event) => {
        setStatus(event.payload);
      });
      unlistenFn = off;
    })();

    const interval = window.setInterval(() => {
      if (document.visibilityState === "visible") void refresh();
    }, RESYNC_INTERVAL_MS);

    const onVisible = () => {
      if (document.visibilityState === "visible") void refresh();
    };
    document.addEventListener("visibilitychange", onVisible);

    return () => {
      if (unlistenFn) unlistenFn();
      window.clearInterval(interval);
      document.removeEventListener("visibilitychange", onVisible);
    };
  }, [refresh]);

  return status;
}
