import { useSyncExternalStore } from "react";

import { persistConfig } from "../../hooks/useConfig";
import { useConfigStore } from "../../store/configStore";
import type { Theme } from "../../types";

interface Props {
  className?: string;
}

function subscribePrefersDark(cb: () => void) {
  const mq = window.matchMedia("(prefers-color-scheme: dark)");
  mq.addEventListener("change", cb);
  return () => mq.removeEventListener("change", cb);
}

function prefersDarkSnapshot(): boolean {
  return window.matchMedia("(prefers-color-scheme: dark)").matches;
}

/** Which sidebar segment is visually highlighted (stored `system` / `custom` follow OS). */
function resolvedLightDark(
  theme: Theme,
  osDark: boolean,
): "light" | "dark" {
  if (theme === "light") {
    return "light";
  }
  if (theme === "dark") {
    return "dark";
  }
  return osDark ? "dark" : "light";
}

export default function ThemeModeSwitcher({ className }: Props) {
  const theme = useConfigStore((s) => s.config.appearance.theme);
  const setAppearance = useConfigStore((s) => s.setAppearance);

  const osDark = useSyncExternalStore(subscribePrefersDark, prefersDarkSnapshot, () => false);

  const active = resolvedLightDark(theme, osDark);

  const commit = (next: Extract<Theme, "light" | "dark">) => {
    if (theme === next) {
      return;
    }
    setAppearance({ theme: next });
    void persistConfig();
  };

  return (
    <div
      className={["kv-nav-theme", "kv-nav-theme--binary", className ?? ""]
        .filter(Boolean)
        .join(" ")}
      role="radiogroup"
      aria-label="Light or dark appearance (matches your system until you pick one)"
    >
      <button
        type="button"
        role="radio"
        aria-checked={active === "light"}
        className={`kv-nav-theme__seg ${active === "light" ? "kv-nav-theme__seg--on" : ""}`}
        title={
          theme === "system" || theme === "custom"
            ? "Matches your system — click to pin light appearance"
            : "Light appearance"
        }
        onClick={() => commit("light")}
      >
        Light
      </button>
      <button
        type="button"
        role="radio"
        aria-checked={active === "dark"}
        className={`kv-nav-theme__seg ${active === "dark" ? "kv-nav-theme__seg--on" : ""}`}
        title={
          theme === "system" || theme === "custom"
            ? "Matches your system — click to pin dark appearance"
            : "Dark appearance"
        }
        onClick={() => commit("dark")}
      >
        Dark
      </button>
    </div>
  );
}
