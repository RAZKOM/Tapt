import { open as openShell } from "@tauri-apps/plugin-shell";

import type { UpdateStatus } from "../../types";

interface UpdateBannerProps {
  status: UpdateStatus;
}

export default function UpdateBanner({ status }: UpdateBannerProps) {
  if (!status.update_available || !status.latest_version) {
    return null;
  }

  return (
    <div className="kv-update-banner" role="status">
      <div className="kv-update-banner__text">
        <strong>Tapt {status.latest_version} is available.</strong>
        <span> You have {status.current_version}.</span>
      </div>
      <button
        type="button"
        className="kv-button kv-update-banner__btn"
        onClick={() => void openShell(status.releases_url)}
      >
        Download
      </button>
    </div>
  );
}
