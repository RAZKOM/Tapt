import { create } from "zustand";

import type { LicenseFailure, LicenseInfo, TrialStatus } from "../types";

interface AuthState {
  authenticated: boolean;
  reason: string;
  licenseInfo: LicenseInfo | null;
  /** Why activating the saved key at startup failed (reported by Rust). */
  licenseError: LicenseFailure | null;
  trial: TrialStatus | null;
  setAuthenticated: (
    authenticated: boolean,
    reason?: string,
    licenseError?: LicenseFailure | null,
  ) => void;
  setLicenseInfo: (info: LicenseInfo | null) => void;
  setTrial: (trial: TrialStatus | null) => void;
}

export const useAuthStore = create<AuthState>((set) => ({
  authenticated: false,
  reason: "",
  licenseInfo: null,
  licenseError: null,
  trial: null,
  setAuthenticated: (authenticated, reason = "", licenseError = null) =>
    set({ authenticated, reason, licenseError }),
  setLicenseInfo: (licenseInfo) => set({ licenseInfo }),
  setTrial: (trial) => set({ trial }),
}));

/** Overlay and settings are usable: licensed, or free-trial time left. */
export function canUseApp(s: Pick<AuthState, "authenticated" | "trial">): boolean {
  return s.authenticated || (s.trial?.active ?? false);
}
