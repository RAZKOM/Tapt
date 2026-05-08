import { useCallback, useEffect, useState } from "react";

import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

import BrandWordmark from "./BrandWordmark";

function WinIconMinimize() {
  return (
    <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true">
      <path
        fill="currentColor"
        d="M1 4.5h8v1H1z"
      />
    </svg>
  );
}

function WinIconMaximize() {
  return (
    <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true">
      <path
        fill="none"
        stroke="currentColor"
        strokeWidth="1"
        d="M1.5 1.5h7v7h-7z"
      />
    </svg>
  );
}

function WinIconRestore() {
  return (
    <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true">
      <path
        fill="none"
        stroke="currentColor"
        strokeWidth="1"
        d="M2.5 2.5h5v5h-5zM3.5 3.5V2h5.5v5.5H7"
      />
    </svg>
  );
}

function WinIconClose() {
  return (
    <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true">
      <path
        fill="none"
        stroke="currentColor"
        strokeWidth="1.1"
        strokeLinecap="round"
        d="M2.5 2.5 7.5 7.5M7.5 2.5 2.5 7.5"
      />
    </svg>
  );
}

export default function SettingsTitleBar() {
  const [maximized, setMaximized] = useState(false);

  const syncMaximized = useCallback(async () => {
    if (!isTauri()) {
      return;
    }
    setMaximized(await getCurrentWindow().isMaximized());
  }, []);

  useEffect(() => {
    if (!isTauri()) {
      return;
    }
    const win = getCurrentWindow();
    let unlisten: (() => void) | undefined;
    void syncMaximized();
    void win.onResized(() => {
      void syncMaximized();
    }).then((fn) => {
      unlisten = fn;
    });
    return () => {
      unlisten?.();
    };
  }, [syncMaximized]);

  const onMinimize = () => {
    if (!isTauri()) {
      return;
    }
    void getCurrentWindow().minimize();
  };

  const onToggleMaximize = () => {
    if (!isTauri()) {
      return;
    }
    void getCurrentWindow().toggleMaximize();
  };

  const onClose = () => {
    if (!isTauri()) {
      return;
    }
    void getCurrentWindow().hide();
  };

  const onTitlebarDoubleClick = () => {
    if (!isTauri()) {
      return;
    }
    void getCurrentWindow().toggleMaximize();
  };

  return (
    <header className="kv-titlebar">
      <div
        className="kv-titlebar__drag"
        data-tauri-drag-region=""
        onDoubleClick={onTitlebarDoubleClick}
      >
        <div className="kv-titlebar__brand">
          <BrandWordmark />
        </div>
      </div>
      {isTauri() && (
        <div className="kv-titlebar__controls">
          <button
            type="button"
            className="kv-titlebar__btn"
            aria-label="Minimize"
            onClick={onMinimize}
          >
            <WinIconMinimize />
          </button>
          <button
            type="button"
            className="kv-titlebar__btn"
            aria-label={maximized ? "Restore" : "Maximize"}
            onClick={onToggleMaximize}
          >
            {maximized ? <WinIconRestore /> : <WinIconMaximize />}
          </button>
          <button
            type="button"
            className="kv-titlebar__btn kv-titlebar__btn--close"
            aria-label="Close"
            onClick={onClose}
          >
            <WinIconClose />
          </button>
        </div>
      )}
    </header>
  );
}
