import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect } from "react";

import { useAuthStore } from "../store/authStore";
import type { AuthStatusPayload, LicenseInfo } from "../types";

const RESYNC_INTERVAL_MS = 60_000;

/**
 * Subscribe to the `auth-status-changed` Tauri event and keep the
 * Zustand auth slice synced. Call exactly once near the root of each
 * window (overlay + settings).
 *
 * Also re-fetches `get_auth_status` on focus / visibility and on an
 * interval while the window is visible. The overlay gates on the same
 * in-memory AuthForge session; if a heartbeat fails or the event is
 * missed, the License tab could otherwise still show “active”.
 */
export function useAuthGuard() {
  const setAuthenticated = useAuthStore((s) => s.setAuthenticated);
  const setLicenseInfo = useAuthStore((s) => s.setLicenseInfo);

  const syncAuthFromBackend = useCallback(async () => {
    try {
      const status = await invoke<AuthStatusPayload>("get_auth_status");
      setAuthenticated(status.authenticated, status.reason);
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
  }, [setAuthenticated, setLicenseInfo]);

  useEffect(() => {
    let unlistenFn: (() => void) | null = null;

    void (async () => {
      await syncAuthFromBackend();

      const off = await listen<AuthStatusPayload>(
        "auth-status-changed",
        (event) => {
          setAuthenticated(event.payload.authenticated, event.payload.reason);
          if (!event.payload.authenticated) {
            setLicenseInfo(null);
          } else {
            void invoke<LicenseInfo | null>("get_license_info").then(
              setLicenseInfo,
            );
          }
        },
      );
      unlistenFn = off;
    })();

    return () => {
      if (unlistenFn) {
        unlistenFn();
      }
    };
  }, [setAuthenticated, setLicenseInfo, syncAuthFromBackend]);

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
