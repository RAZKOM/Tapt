import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";

import { useAuthGuard } from "../hooks/useAuthGuard";
import { useConfig } from "../hooks/useConfig";
import { useThemeTokens } from "../hooks/useThemeTokens";
import { useUpdateCheck } from "../hooks/useUpdateCheck";
import { useAuthStore } from "../store/authStore";
import OverlayModeSwitcher from "./components/OverlayModeSwitcher";
import SettingsTitleBar from "./components/SettingsTitleBar";
import UpdateBanner from "./components/UpdateBanner";
import ThemeModeSwitcher from "./components/ThemeModeSwitcher";
import BehaviorTab from "./tabs/BehaviorTab";
import DisplayTab from "./tabs/DisplayTab";
import LicenseTab from "./tabs/LicenseTab";
import PositionTab from "./tabs/PositionTab";

import "./settings.css";

const TABS = [
  { id: "license", label: "License" },
  { id: "behavior", label: "Behavior" },
  { id: "display", label: "Appearance" },
  { id: "position", label: "Position" },
] as const;

type TabId = (typeof TABS)[number]["id"];

export default function SettingsApp() {
  const [settingsRoot, setSettingsRoot] = useState<HTMLDivElement | null>(null);
  useAuthGuard();
  useConfig();
  useThemeTokens(settingsRoot);
  const updateStatus = useUpdateCheck();

  const authenticated = useAuthStore((s) => s.authenticated);
  const checkingLicense = useAuthStore((s) => s.reason) === "checking";
  const [active, setActive] = useState<TabId>(
    authenticated ? "display" : "license",
  );

  useEffect(() => {
    if ((!authenticated || checkingLicense) && active !== "license") {
      setActive("license");
    }
  }, [authenticated, checkingLicense, active]);

  // Rust can request the License tab via this event (e.g. when a
  // saved license rejects on startup).
  useEffect(() => {
    let off: (() => void) | null = null;
    void (async () => {
      off = await listen("navigate-license", () => setActive("license"));
    })();
    return () => {
      if (off) off();
    };
  }, []);

  return (
    <div ref={setSettingsRoot} className="kv-settings">
      <SettingsTitleBar />
      {updateStatus ? <UpdateBanner status={updateStatus} /> : null}
      <div className="kv-settings__body">
        <nav className="kv-tabs" aria-label="Settings tabs">
          <div className="kv-tabs__scroll">
            {TABS.map((t) => {
              const locked =
                (!authenticated || checkingLicense) && t.id !== "license";
              return (
                <button
                  key={t.id}
                  type="button"
                  className="kv-tab"
                  aria-selected={active === t.id}
                  disabled={locked}
                  title={
                    locked
                      ? "Activate a license to change behavior, appearance, and position."
                      : undefined
                  }
                  onClick={() => setActive(t.id)}
                >
                  {t.label}
                </button>
              );
            })}
          </div>
          {authenticated && (
            <div className="kv-tabs__quick" aria-label="Quick settings">
              <div className="kv-tabs__quick-group">
                <div className="kv-tabs__quick-label">Overlay mode</div>
                <OverlayModeSwitcher className="kv-overlay-mode--nav" />
              </div>
              <div className="kv-tabs__quick-group">
                <div className="kv-tabs__quick-label">Theme</div>
                <ThemeModeSwitcher />
              </div>
            </div>
          )}
        </nav>
        <main
          className={
            active === "license"
              ? "kv-tab-body kv-tab-body--license-footer"
              : "kv-tab-body"
          }
        >
          {active === "license" && <LicenseTab updateStatus={updateStatus} />}
          {active === "behavior" && <BehaviorTab />}
          {active === "display" && <DisplayTab />}
          {active === "position" && <PositionTab />}
        </main>
      </div>
    </div>
  );
}
