import { open as openShell } from "@tauri-apps/plugin-shell";

import { RELEASES_URL } from "../../branding";
import type { UpdateStatus } from "../../types";

interface AppVersionLineProps {
  status: UpdateStatus | null;
}

export default function AppVersionLine({ status }: AppVersionLineProps) {
  const version = status?.current_version ?? "…";

  return (
    <p className="kv-about-version">
      <span className="kv-about-version__label">Version</span>{" "}
      <span className="kv-about-version__value">{version}</span>
      {status?.update_available && status.latest_version ? (
        <span className="kv-about-version__hint">
          {" · "}
          Version {status.latest_version} is available.{" "}
          <button
            type="button"
            className="kv-license-footnote__link"
            onClick={() => void openShell(status.releases_url || RELEASES_URL)}
          >
            Download
          </button>
        </span>
      ) : (
        <span className="kv-about-version__hint">
          {" · "}
          <button
            type="button"
            className="kv-license-footnote__link"
            onClick={() => void openShell(RELEASES_URL)}
          >
            razkom.com/tapt
          </button>
        </span>
      )}
    </p>
  );
}
