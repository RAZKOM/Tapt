import { persistConfig } from "../../hooks/useConfig";
import { useConfigStore } from "../../store/configStore";
import type { DisplayMode } from "../../types";

function IconKeyFeed() {
  return (
    <svg width="18" height="18" viewBox="0 0 20 20" aria-hidden>
      <rect
        x="2"
        y="3"
        width="16"
        height="4"
        rx="2"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.2"
      />
      <rect
        x="2"
        y="8"
        width="12"
        height="4"
        rx="2"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.2"
      />
      <rect
        x="2"
        y="13"
        width="16"
        height="4"
        rx="2"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.2"
      />
    </svg>
  );
}

function IconKeyboardOverlay() {
  const s = "0.28";
  return (
    <svg width="18" height="18" viewBox="0 0 20 20" aria-hidden>
      <rect
        x="1.5"
        y="5"
        width="17"
        height="11"
        rx="2"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.2"
      />
      {/* top letter row */}
      <rect x="3" y="6.85" width="2.5" height="2.05" rx={s} fill="currentColor" />
      <rect x="6" y="6.85" width="2.5" height="2.05" rx={s} fill="currentColor" />
      <rect x="9" y="6.85" width="2.5" height="2.05" rx={s} fill="currentColor" />
      <rect x="12" y="6.85" width="2.5" height="2.05" rx={s} fill="currentColor" />
      <rect x="15" y="6.85" width="2" height="2.05" rx={s} fill="currentColor" />
      {/* home row */}
      <rect x="3" y="9.95" width="3.2" height="2.05" rx={s} fill="currentColor" />
      <rect x="6.7" y="9.95" width="7.35" height="2.05" rx={s} fill="currentColor" />
      <rect x="14.5" y="9.95" width="2.5" height="2.05" rx={s} fill="currentColor" />
      {/* space row */}
      <rect x="3" y="13.05" width="14" height="2.05" rx={s} fill="currentColor" />
    </svg>
  );
}

interface Props {
  className?: string;
}

export default function OverlayModeSwitcher({ className }: Props) {
  const mode = useConfigStore((s) => s.config.display.mode);
  const setDisplay = useConfigStore((s) => s.setDisplay);

  const commit = (next: DisplayMode) => {
    if (mode === next) {
      return;
    }
    setDisplay({ mode: next });
    void persistConfig();
  };

  return (
    <div
      className={["kv-overlay-mode", className ?? ""].filter(Boolean).join(" ")}
      role="radiogroup"
      aria-label="Overlay display mode"
    >
      <button
        type="button"
        role="radio"
        aria-checked={mode === "key-feed"}
        className={`kv-overlay-mode__seg ${mode === "key-feed" ? "kv-overlay-mode__seg--on" : ""}`}
        title="Key feed — animated pills"
        onClick={() => commit("key-feed")}
      >
        <IconKeyFeed />
      </button>
      <button
        type="button"
        role="radio"
        aria-checked={mode === "keyboard-overlay"}
        className={`kv-overlay-mode__seg ${mode === "keyboard-overlay" ? "kv-overlay-mode__seg--on" : ""}`}
        title="Keyboard overlay — on-screen layout"
        onClick={() => commit("keyboard-overlay")}
      >
        <IconKeyboardOverlay />
      </button>
    </div>
  );
}
