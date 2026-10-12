import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect } from "react";

import { useAuthStore } from "../store/authStore";
import type { AuthStatusPayload, LicenseInfo, TrialStatus } from "../types";

const RESYNC_INTERVAL_MS = 60_000;

/**
 * Subscribe to the `auth-status-changed` and `trial-status-changed` Tauri
 * events and keep the Zustand auth slice synced. Call exactly once near
 * the root of the settings window.
 *
 * Also re-fetches `get_auth_status` / `get_trial_status` on focus /
 * visibility and on an interval while the window is visible, in case an
 * event is missed.
 */
export function useAuthGuard() {
  const setAuthenticated = useAuthStore((s) => s.setAuthenticated);
  const setLicenseInfo = useAuthStore((s) => s.setLicenseInfo);
  const setTrial = useAuthStore((s) => s.setTrial);

  const syncAuthFromBackend = useCallback(async () => {
    try {
      setTrial(await invoke<TrialStatus>("get_trial_status"));
      const status = await invoke<AuthStatusPayload>("get_auth_status");
      setAuthenticated(status.authenticated, status.reason, status.error);
      if (status.authenticated) {
        const info = await invoke<LicenseInfo | null>("get_license_info");
        setLicenseInfo(info);
      } else {
        setLicenseInfo(null);
      }
    } catch {
      setAuthenticated(false, "ipc-error");
      setLicenseInfo(null);
    }
  }, [setAuthenticated, setLicenseInfo, setTrial]);

  useEffect(() => {
    const unlisten: Array<() => void> = [];
    let disposed = false;

    void (async () => {
      await syncAuthFromBackend();

      const offTrial = await listen<TrialStatus>("trial-status-changed", (event) => {
        setTrial(event.payload);
      });
      const off = await listen<AuthStatusPayload>(
        "auth-status-changed",
        (event) => {
          setAuthenticated(
            event.payload.authenticated,
            event.payload.reason,
            event.payload.error,
          );
          if (!event.payload.authenticated) {
            setLicenseInfo(null);
          } else {
            void invoke<LicenseInfo | null>("get_license_info").then(
              setLicenseInfo,
            );
          }
        },
      );
      if (disposed) {
        offTrial();
        off();
      } else {
        unlisten.push(offTrial, off);
      }
    })();

    return () => {
      disposed = true;
      unlisten.forEach((fn) => fn());
    };
  }, [setAuthenticated, setLicenseInfo, setTrial, syncAuthFromBackend]);

  useEffect(() => {
    const onVisible = () => {
      if (document.visibilityState === "visible") void syncAuthFromBackend();
    };
    const onFocus = () => void syncAuthFromBackend();

    document.addEventListener("visibilitychange", onVisible);
    window.addEventListener("focus", onFocus);

    const interval = window.setInterval(() => {
      if (document.visibilityState === "visible") void syncAuthFromBackend();
    }, RESYNC_INTERVAL_MS);

    return () => {
      document.removeEventListener("visibilitychange", onVisible);
      window.removeEventListener("focus", onFocus);
      window.clearInterval(interval);
    };
  }, [syncAuthFromBackend]);
}
