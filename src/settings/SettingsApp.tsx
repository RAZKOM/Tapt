import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";

import { useAuthGuard } from "../hooks/useAuthGuard";
import { useConfig } from "../hooks/useConfig";
import { useThemeTokens } from "../hooks/useThemeTokens";
import { useAuthStore } from "../store/authStore";
import SettingsTitleBar from "./components/SettingsTitleBar";
import BehaviorTab from "./tabs/BehaviorTab";
import DisplayTab from "./tabs/DisplayTab";
import LicenseTab from "./tabs/LicenseTab";
import PositionTab from "./tabs/PositionTab";

import "./settings.css";

const TABS = [
  { id: "license", label: "License" },
  { id: "behavior", label: "Behaviour" },
  { id: "display", label: "Appearance" },
  { id: "position", label: "Position" },
] as const;

type TabId = (typeof TABS)[number]["id"];

export default function SettingsApp() {
  const [settingsRoot, setSettingsRoot] = useState<HTMLDivElement | null>(null);
  useAuthGuard();
  useConfig();
  useThemeTokens(settingsRoot);

  const authenticated = useAuthStore((s) => s.authenticated);
  const [active, setActive] = useState<TabId>(
    authenticated ? "display" : "license",
  );

  useEffect(() => {
    if (!authenticated && active !== "license") {
      setActive("license");
    }
  }, [authenticated, active]);

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
      <div className="kv-settings__body">
        <nav className="kv-tabs" aria-label="Settings tabs">
          {TABS.map((t) => {
            const locked = !authenticated && t.id !== "license";
            return (
              <button
                key={t.id}
                type="button"
                className="kv-tab"
                aria-selected={active === t.id}
                disabled={locked}
                title={
                  locked
                    ? "Activate a license to change behaviour, appearance, and position."
                    : undefined
                }
                onClick={() => setActive(t.id)}
              >
                {t.label}
              </button>
            );
          })}
        </nav>
        <main
          className={
            active === "license"
              ? "kv-tab-body kv-tab-body--license-footer"
              : "kv-tab-body"
          }
        >
          {active === "license" && <LicenseTab />}
          {active === "behavior" && <BehaviorTab />}
          {active === "display" && <DisplayTab />}
          {active === "position" && <PositionTab />}
        </main>
      </div>
    </div>
  );
}
