import { create } from "zustand";

import type { LicenseInfo } from "../types";

interface AuthState {
  authenticated: boolean;
  reason: string;
  licenseInfo: LicenseInfo | null;
  setAuthenticated: (authenticated: boolean, reason?: string) => void;
  setLicenseInfo: (info: LicenseInfo | null) => void;
}

export const useAuthStore = create<AuthState>((set) => ({
  authenticated: false,
  reason: "",
  licenseInfo: null,
  setAuthenticated: (authenticated, reason = "") =>
    set({ authenticated, reason }),
  setLicenseInfo: (licenseInfo) => set({ licenseInfo }),
}));
