import { invoke } from "@tauri-apps/api/core";
import { open as openShell } from "@tauri-apps/plugin-shell";
import { useEffect, useMemo, useState } from "react";

import {
  AUTHFORGE_CUSTOMER_PORTAL_URL,
  PRODUCT_DISPLAY_NAME,
} from "../../branding";
import { useAuthStore } from "../../store/authStore";
import AppVersionLine from "../components/AppVersionLine";
import type {
  LicenseFailure,
  LicenseInfo,
  LicenseType,
  PurchaseUrls,
  UpdateStatus,
} from "../../types";

const KEY_REGEX = /^[A-Z0-9-]+$/;

interface LicenseTabProps {
  updateStatus: UpdateStatus | null;
}

export default function LicenseTab({ updateStatus }: LicenseTabProps) {
  const authenticated = useAuthStore((s) => s.authenticated);
  const authReason = useAuthStore((s) => s.reason);
  const licenseInfo = useAuthStore((s) => s.licenseInfo);
  const checkingLicense = authReason === "checking";
  const setLicenseInfo = useAuthStore((s) => s.setLicenseInfo);
  const setAuthenticated = useAuthStore((s) => s.setAuthenticated);

  const [keyInput, setKeyInput] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [purchaseUrls, setPurchaseUrls] = useState<PurchaseUrls | null>(null);

  useEffect(() => {
    void invoke<PurchaseUrls>("get_purchase_urls").then(setPurchaseUrls);
  }, []);

  /** `deactivate_license` emits `auth-status-changed` before the invoke returns; clear busy so the UI does not stay on “Deactivating…” or the login form’s “Activating…”. */
  useEffect(() => {
    if (!authenticated) {
      setBusy(false);
    }
  }, [authenticated]);

  // Pull the latest LicenseInfo when transitioning to authenticated.
  useEffect(() => {
    if (!authenticated) return;
    void invoke<LicenseInfo | null>("get_license_info").then((info) => {
      if (info) setLicenseInfo(info);
    });
  }, [authenticated, setLicenseInfo]);

  const masked = useMemo(() => formatLicenseInput(keyInput), [keyInput]);

  async function activate() {
    if (busy) return;
    const trimmed = keyInput.replace(/\s+/g, "").toUpperCase();
    if (!KEY_REGEX.test(trimmed) || trimmed.length < 8) {
      setError("Enter a complete license key.");
      return;
    }
    setBusy(true);
    setError(null);
    try {
      const info = await invoke<LicenseInfo>("activate_license", {
        key: trimmed,
      });
      setLicenseInfo(info);
      setKeyInput("");
    } catch (rawErr) {
      const failure = rawErr as LicenseFailure | string;
      const message =
        typeof failure === "string"
          ? failure
          : (failure?.message ?? "Activation failed.");
      setError(message);
    } finally {
      setBusy(false);
    }
  }

  async function deactivate() {
    if (busy) return;
    setBusy(true);
    setError(null);
    try {
      await invoke("deactivate_license");
      setAuthenticated(false, "deactivated");
      setLicenseInfo(null);
    } catch (rawErr) {
      setError(String(rawErr));
    } finally {
      setBusy(false);
    }
  }

  async function buy(url: string) {
    if (!url) {
      setError(
        "Checkout URL is not configured in this build. Set STRIPE_MONTHLY_URL / STRIPE_LIFETIME_URL in src-tauri/.env.",
      );
      return;
    }
    try {
      await openShell(url);
    } catch (err) {
      setError(`Could not open checkout: ${String(err)}`);
    }
  }

  async function openAuthForgePortal() {
    try {
      setError(null);
      await openShell(AUTHFORGE_CUSTOMER_PORTAL_URL);
    } catch (err) {
      setError(`Could not open browser: ${String(err)}`);
    }
  }

  if (checkingLicense) {
    return (
      <div className="kv-license-tab">
        <div className="kv-license-tab__main">
          <h2 className="kv-section-title">License</h2>
          <div
            className="kv-license-status"
            data-state="checking"
            role="status"
            aria-live="polite"
          >
            <span className="kv-spinner" aria-hidden />
            <div>
              <div style={{ fontWeight: 600 }}>Checking license…</div>
              <div style={{ fontSize: 13, color: "var(--kv-settings-muted)" }}>
                Verifying your saved license with the server.
              </div>
            </div>
          </div>
        </div>
        <LicenseTabFooter
          updateStatus={updateStatus}
          appId={purchaseUrls?.authforge_app_id?.trim() ?? ""}
          onOpenPortal={() => void openAuthForgePortal()}
        />
      </div>
    );
  }

  if (authenticated && licenseInfo) {
    return (
      <ActiveState
        info={licenseInfo}
        onDeactivate={deactivate}
        onOpenPortal={() => void openAuthForgePortal()}
        authforgeAppId={purchaseUrls?.authforge_app_id?.trim() ?? ""}
        updateStatus={updateStatus}
        busy={busy}
        error={error}
      />
    );
  }

  return (
    <div className="kv-license-tab">
      <div className="kv-license-tab__main">
        <h2 className="kv-section-title">License</h2>
        <p style={{ color: "var(--kv-settings-muted)" }}>
          {PRODUCT_DISPLAY_NAME} requires an active license to capture and display
          your keystrokes on the overlay.
        </p>

        <div
          style={{
            display: "flex",
            alignItems: "center",
            gap: 12,
            padding: 16,
            marginTop: 8,
            marginBottom: 16,
            borderRadius: 12,
            background: "color-mix(in srgb, var(--kv-settings-danger) 8%, transparent)",
            border: "1px solid color-mix(in srgb, var(--kv-settings-danger) 30%, transparent)",
          }}
        >
          <span style={{ fontSize: 22 }}>🔒</span>
          <div>
            <div style={{ fontWeight: 600 }}>No active license</div>
            <div style={{ fontSize: 13, color: "var(--kv-settings-muted)" }}>
              Enter a license key to activate this device.
            </div>
          </div>
        </div>

        <label
          style={{ display: "block", fontSize: 14, fontWeight: 500, marginBottom: 6 }}
        >
          License key
        </label>
        <div style={{ display: "flex", gap: 8 }}>
          <input
            className="kv-license-input"
            placeholder="XXXX-XXXX-XXXX-XXXX"
            value={masked}
            onChange={(e) =>
              setKeyInput(e.target.value.replace(/[^A-Za-z0-9-]/g, ""))
            }
            onKeyDown={(e) => {
              if (e.key === "Enter") void activate();
            }}
            inputMode="text"
            autoComplete="off"
            spellCheck={false}
            maxLength={19}
          />
          <button
            type="button"
            className="kv-button"
            onClick={() => void activate()}
            disabled={busy}
            aria-busy={busy}
          >
            <LicenseButtonLabel
              busy={busy}
              idleLabel="Activate"
              busyLabel="Activating…"
            />
          </button>
        </div>

        {error && <div className="kv-license-error">{error}</div>}

        <div style={{ marginTop: 24 }}>
          <LicenseCheckoutButtons
            heading="Don't have a license?"
            purchaseUrls={purchaseUrls}
            onBuy={(url) => void buy(url)}
          />
        </div>
      </div>

      <LicenseTabFooter
        updateStatus={updateStatus}
        appId={purchaseUrls?.authforge_app_id?.trim() ?? ""}
        onOpenPortal={() => void openAuthForgePortal()}
      />
    </div>
  );
}

function LicenseTabFooter({
  updateStatus,
  appId,
  onOpenPortal,
}: {
  updateStatus: UpdateStatus | null;
  appId: string;
  onOpenPortal: () => void;
}) {
  return (
    <footer className="kv-license-footnote-wrap">
      <AppVersionLine status={updateStatus} />
      <LicensePortalFootnote appId={appId} onOpenPortal={onOpenPortal} />
    </footer>
  );
}

function LicenseCheckoutButtons({
  heading,
  purchaseUrls,
  onBuy,
}: {
  heading: string;
  purchaseUrls: PurchaseUrls | null;
  onBuy: (url: string) => void;
}) {
  return (
    <>
      <h3 style={{ fontSize: 14, color: "var(--kv-settings-muted)", margin: 0 }}>
        {heading}
      </h3>
      <div style={{ display: "flex", gap: 12, marginTop: 10, flexWrap: "wrap" }}>
        <button
          type="button"
          className="kv-button"
          onClick={() => onBuy(purchaseUrls?.monthly ?? "")}
        >
          1-month license — $2.99
        </button>
        <button
          type="button"
          className="kv-button kv-secondary"
          onClick={() => onBuy(purchaseUrls?.lifetime ?? "")}
        >
          Buy Lifetime — $9.99
        </button>
      </div>
    </>
  );
}

interface ActiveProps {
  info: LicenseInfo;
  busy: boolean;
  error: string | null;
  onDeactivate: () => void;
  onOpenPortal: () => void;
  authforgeAppId: string;
  updateStatus: UpdateStatus | null;
}

const SEC = 1;
const HOUR = 3600 * SEC;
const DAY = 86400 * SEC;

type ExpiryTone = "notice" | "warn" | "urgent";

interface ExpiryCopy {
  subtitle: string;
  banner: {
    tone: ExpiryTone;
    message: string;
  } | null;
}

function expiryCopyFor(
  licenseType: LicenseType,
  periodEnd: number | null,
  remainingSec: number | null,
): ExpiryCopy {
  if (licenseType === "lifetime") {
    return { subtitle: "Never expires", banner: null };
  }
  if (periodEnd === null || remainingSec === null) {
    return {
      subtitle: "Active license",
      banner: null,
    };
  }

  const endDate = mediumDateTime(periodEnd);
  const { actionLabel, shortNoun } = periodPhrase(licenseType);

  if (remainingSec <= 0) {
    return {
      subtitle: `${shortNoun} ended (${endDate}). Purchase a new license, then activate your new key on this device.`,
      banner: {
        tone: "urgent",
        message:
          "The license server reports this license term has ended. If you already purchased a new key, try re-activating or wait for the next sync.",
      },
    };
  }

  const dayCount = Math.max(1, Math.ceil(remainingSec / DAY));
  let subtitle: string;

  if (remainingSec <= 48 * HOUR) {
    const countdown = formatCountdown(remainingSec);
    subtitle =
      actionLabel === "expires"
        ? `Expires in ${countdown} · ${endDate}`
        : `${shortNoun} ends in ${countdown} · ${endDate}`;
  } else if (remainingSec <= 7 * DAY) {
    subtitle =
      actionLabel === "expires"
        ? `Expires in ${dayCount} day${dayCount === 1 ? "" : "s"} (${endDate})`
        : `${shortNoun} ends in ${dayCount} day${dayCount === 1 ? "" : "s"} (${endDate})`;
  } else if (remainingSec <= 14 * DAY) {
    subtitle =
      actionLabel === "expires"
        ? `Expires on ${endDate} (${dayCount} days left)`
        : `${shortNoun} ends ${endDate} (${dayCount} days left)`;
  } else {
    subtitle =
      actionLabel === "expires"
        ? `Expires on ${endDate}`
        : `${shortNoun} ends on ${endDate}`;
  }

  let banner: ExpiryCopy["banner"] = null;
  if (remainingSec <= 48 * HOUR) {
    banner = {
      tone: "urgent",
      message:
        licenseType === "trial"
          ? "Trial ending very soon — purchase a paid license, then activate your new key."
          : "License expires very soon — purchase another term, then activate your new key.",
    };
  } else if (remainingSec <= 7 * DAY) {
    banner = {
      tone: "warn",
      message:
        licenseType === "trial"
          ? "Trial ends this week — purchase a paid license when you’re ready."
          : "Your license ends this week — purchase another term when you’re ready.",
    };
  } else if (remainingSec <= 14 * DAY) {
    banner = {
      tone: "notice",
      message:
        licenseType === "trial"
          ? "Trial ending soon — purchase a paid license when you pick a plan."
          : "License ending soon — purchase another term before this one ends.",
    };
  }

  return { subtitle, banner };
}


function periodPhrase(
  t: Exclude<LicenseType, "lifetime">,
): { actionLabel: "expires" | "ends"; shortNoun: string } {
  switch (t) {
    case "monthly":
      return { actionLabel: "expires", shortNoun: "1-month license" };
    case "trial":
      return { actionLabel: "ends", shortNoun: "Trial" };
    case "unknown":
      return { actionLabel: "ends", shortNoun: "Paid period" };
    default: {
      const _x: never = t;
      return _x;
    }
  }
}

function mediumDateTime(unix: number): string {
  return new Date(unix * 1000).toLocaleString(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  });
}

/** Compact countdown suitable for hourly / sub-day windows. */
function formatCountdown(secs: number): string {
  if (secs <= 0) return "0:00:00";
  const d = Math.floor(secs / DAY);
  const h = Math.floor((secs % DAY) / HOUR);
  const m = Math.floor((secs % HOUR) / 60);
  const s = secs % 60;
  if (d > 0) return `${d}d ${h}h ${m}m`;
  if (h > 0) return `${h}h ${m}m ${s}s`;
  if (m > 0) return `${m}m ${s}s`;
  return `${s}s`;
}

function ExpiryAlertBanner({
  tone,
  message,
}: {
  tone: ExpiryTone;
  message: string;
}) {
  const icon = tone === "urgent" ? "⏳" : tone === "warn" ? "⚠️" : "ℹ️";

  return (
    <div className="kv-license-expiry-alert" data-tone={tone} role="status">
      <span className="kv-license-expiry-alert__icon" aria-hidden>
        {icon}
      </span>
      <div className="kv-license-expiry-alert__body">
        <p>{message}</p>
      </div>
    </div>
  );
}

function ActiveState({
  info,
  busy,
  error,
  onDeactivate,
  onOpenPortal,
  authforgeAppId,
  updateStatus,
}: ActiveProps) {
  const isLifetime = info.license_type === "lifetime";
  const periodEnd = info.subscription_period_end_unix;

  const [tick, setTick] = useState(0);

  useEffect(() => {
    if (isLifetime || periodEnd === null) return;
    const id = window.setInterval(() => setTick((x) => x + 1), 1000);
    return () => window.clearInterval(id);
  }, [isLifetime, periodEnd]);

  const remainingSec = useMemo(() => {
    if (isLifetime || periodEnd === null) return null;
    return Math.floor(periodEnd - Date.now() / 1000);
  }, [isLifetime, periodEnd, tick]);

  const expiry = expiryCopyFor(info.license_type, periodEnd, remainingSec);

  const [deactivateConfirmOpen, setDeactivateConfirmOpen] = useState(false);

  return (
    <div className="kv-license-tab">
      <div className="kv-license-tab__main">
        <h2 className="kv-section-title">License</h2>
        <div
          className="kv-license-status"
          data-state={isLifetime ? "lifetime" : "active"}
        >
          <span style={{ fontSize: 22 }}>{isLifetime ? "★" : "✓"}</span>
          <div>
            <div style={{ fontWeight: 600 }}>
              License Active — {labelFor(info.license_type)}
            </div>
            <div style={{ fontSize: 13, color: "var(--kv-settings-muted)" }}>
              {expiry.subtitle}
            </div>
            <div
              style={{ fontSize: 12, color: "var(--kv-settings-muted)", marginTop: 4 }}
            >
              Bound to this device (HWID: {info.hwid_masked})
            </div>
          </div>
        </div>

        {expiry.banner ? (
          <ExpiryAlertBanner tone={expiry.banner.tone} message={expiry.banner.message} />
        ) : null}

        <div style={{ marginTop: 16 }}>
          {deactivateConfirmOpen ? (
            <div
              role="dialog"
              aria-modal="false"
              aria-labelledby="kv-deactivate-confirm-title"
              aria-describedby="kv-deactivate-confirm-desc"
              style={{
                padding: 14,
                borderRadius: 10,
                border:
                  "1px solid color-mix(in srgb, var(--kv-settings-danger) 35%, transparent)",
                background:
                  "color-mix(in srgb, var(--kv-settings-danger) 6%, transparent)",
              }}
            >
              <div
                id="kv-deactivate-confirm-title"
                style={{
                  margin: "0 0 6px",
                  fontWeight: 600,
                  fontSize: 14,
                  lineHeight: 1.4,
                }}
              >
                Deactivate this license?
              </div>
              <p
                id="kv-deactivate-confirm-desc"
                style={{
                  margin: "0 0 14px",
                  fontSize: 13,
                  color: "var(--kv-settings-muted)",
                  lineHeight: 1.45,
                }}
              >
                This removes the license from this device until you activate again with your
                license key.
              </p>
              <div style={{ display: "flex", flexWrap: "wrap", gap: 8 }}>
                <button
                  type="button"
                  className="kv-button kv-secondary"
                  disabled={busy}
                  onClick={() => setDeactivateConfirmOpen(false)}
                >
                  Cancel
                </button>
                <button
                  type="button"
                  className="kv-button kv-danger"
                  onClick={() => void onDeactivate()}
                  disabled={busy}
                  aria-busy={busy}
                >
                  <LicenseButtonLabel
                    busy={busy}
                    idleLabel="Deactivate License"
                    busyLabel="Deactivating…"
                  />
                </button>
              </div>
            </div>
          ) : (
            <button
              type="button"
              className="kv-button kv-danger"
              onClick={() => setDeactivateConfirmOpen(true)}
              disabled={busy}
              aria-busy={busy}
            >
              <LicenseButtonLabel
                busy={busy}
                idleLabel="Deactivate License"
                busyLabel="Deactivating…"
              />
            </button>
          )}
        </div>

        {error && <div className="kv-license-error">{error}</div>}
      </div>

      <LicenseTabFooter
        updateStatus={updateStatus}
        appId={authforgeAppId}
        onOpenPortal={onOpenPortal}
      />
    </div>
  );
}

function LicenseButtonLabel({
  busy,
  idleLabel,
  busyLabel,
}: {
  busy: boolean;
  idleLabel: string;
  busyLabel: string;
}) {
  return (
    <>
      {busy ? <span className="kv-spinner" aria-hidden /> : null}
      {busy ? busyLabel : idleLabel}
    </>
  );
}

async function copyTextToClipboard(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    try {
      const ta = document.createElement("textarea");
      ta.value = text;
      ta.setAttribute("readonly", "");
      ta.style.position = "fixed";
      ta.style.left = "-9999px";
      document.body.appendChild(ta);
      ta.select();
      const ok = document.execCommand("copy");
      document.body.removeChild(ta);
      return ok;
    } catch {
      return false;
    }
  }
}

function LicensePortalFootnote({
  appId,
  onOpenPortal,
}: {
  appId: string;
  onOpenPortal: () => void;
}) {
  const [copyLabel, setCopyLabel] = useState<"idle" | "copied" | "failed">("idle");

  async function copyAppId() {
    if (!appId) return;
    setCopyLabel("idle");
    const ok = await copyTextToClipboard(appId);
    setCopyLabel(ok ? "copied" : "failed");
    window.setTimeout(() => setCopyLabel("idle"), ok ? 1800 : 2400);
  }

  return (
    <div className="kv-license-footnote">
      <p className="kv-license-footnote__line">
        Need device resets or license help? Use the{" "}
        <button type="button" className="kv-license-footnote__link" onClick={onOpenPortal}>
          customer portal
        </button>{" "}
        with your license key and checkout email.
      </p>
      {appId ? (
        <div className="kv-license-footnote__meta">
          <span className="kv-license-footnote__meta-label">App ID</span>
          <button
            type="button"
            className="kv-license-footnote__id"
            onClick={() => void copyAppId()}
            title="Copy app ID"
          >
            {copyLabel === "copied"
              ? "Copied"
              : copyLabel === "failed"
                ? "Copy failed, try again"
                : appId}
          </button>
        </div>
      ) : null}
    </div>
  );
}

function labelFor(t: LicenseType): string {
  switch (t) {
    case "monthly":
      return "1-month license";
    case "lifetime":
      return "Lifetime";
    case "trial":
      return "Trial";
    case "unknown":
      return "Active";
  }
}

/**
 * Format a raw input as `XXXX-XXXX-XXXX-XXXX`. Lets the user paste
 * with or without dashes; we always render the dashed form.
 */
function formatLicenseInput(s: string): string {
  const compact = s.replace(/[^A-Za-z0-9]/g, "").toUpperCase();
  const groups: string[] = [];
  for (let i = 0; i < compact.length && i < 16; i += 4) {
    groups.push(compact.slice(i, i + 4));
  }
  return groups.join("-");
}
