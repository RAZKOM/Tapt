import { useEffect } from "react";

import { useConfigStore } from "../store/configStore";

/**
 * Applies appearance CSS variables and `data-theme` on the settings root.
 * The same `appearance.theme` is read by the native overlay (egui) for light/dark
 * there; accents and sizing apply in both places.
 */
export function useThemeTokens(settingsRoot: HTMLDivElement | null) {
  const appearance = useConfigStore((s) => s.config.appearance);

  useEffect(() => {
    if (!settingsRoot) {
      return;
    }
    settingsRoot.style.setProperty("--kv-primary-accent", appearance.primary_accent);
    settingsRoot.style.setProperty(
      "--kv-modifier-accent",
      appearance.modifier_accent,
    );
    settingsRoot.style.setProperty("--kv-font-size", `${appearance.font_size}px`);
    settingsRoot.style.setProperty(
      "--kv-pill-opacity",
      String(appearance.pill_opacity / 100),
    );
    settingsRoot.style.setProperty(
      "--kv-pill-radius",
      `${appearance.corner_radius}px`,
    );
    settingsRoot.style.setProperty(
      "--kv-blur",
      appearance.backdrop_blur ? "12px" : "0px",
    );
    settingsRoot.dataset.theme = appearance.theme;
  }, [appearance, settingsRoot]);
}
