import { invoke } from "@tauri-apps/api/core";
import { open as openShell } from "@tauri-apps/plugin-shell";
import { useEffect, useMemo, useState } from "react";

import { RELEASES_URL, SUPPORT_EMAIL } from "../../branding";
import { useAuthStore } from "../../store/authStore";
import AppVersionLine from "../components/AppVersionLine";
import type {
  LicenseFailure,
  LicenseInfo,
  PurchaseUrls,
  TrialStatus,
  UpdateStatus,
} from "../../types";

const KEY_REGEX = /^[A-Z0-9-]+$/;

function toFailure(raw: unknown, fallback: string): LicenseFailure {
  if (typeof raw === "string") return { code: "other", message: raw };
  const failure = raw as Partial<LicenseFailure> | null;
  return {
    code: failure?.code ?? "other",
    message: failure?.message ?? fallback,
  };
}

interface LicenseTabProps {
  updateStatus: UpdateStatus | null;
}

export default function LicenseTab({ updateStatus }: LicenseTabProps) {
  const authenticated = useAuthStore((s) => s.authenticated);
  const authReason = useAuthStore((s) => s.reason);
  const licenseInfo = useAuthStore((s) => s.licenseInfo);
  const startupError = useAuthStore((s) => s.licenseError);
  const trial = useAuthStore((s) => s.trial);
  const checkingLicense = authReason === "checking";
  const setLicenseInfo = useAuthStore((s) => s.setLicenseInfo);
  const setAuthenticated = useAuthStore((s) => s.setAuthenticated);

  const [keyInput, setKeyInput] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<LicenseFailure | null>(null);
  const [purchaseUrls, setPurchaseUrls] = useState<PurchaseUrls | null>(null);

  useEffect(() => {
    void invoke<PurchaseUrls>("get_purchase_urls").then(setPurchaseUrls);
  }, []);

  /** `deactivate_license` emits `auth-status-changed` before the invoke returns; clear busy so the UI does not stay on “Removing…” or the login form’s “Activating…”. */
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
      setError({ code: "invalid_key", message: "Enter a complete license key." });
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
      setError(toFailure(rawErr, "Activation failed."));
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
      setError(toFailure(rawErr, "Couldn't remove the license."));
    } finally {
      setBusy(false);
    }
  }

  async function buy(url: string) {
    if (!url) {
      setError({
        code: "other",
        message: `Checkout isn't available right now. You can buy a license at ${RELEASES_URL}`,
      });
      return;
    }
    try {
      await openShell(url);
    } catch {
      setError({
        code: "other",
        message: `Couldn't open your browser. You can buy a license at ${RELEASES_URL}`,
      });
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
              <div style={{ fontWeight: 600 }}>Checking your license…</div>
            </div>
          </div>
        </div>
        <LicenseTabFooter
          updateStatus={updateStatus}
        />
      </div>
    );
  }

  if (authenticated && licenseInfo) {
    return (
      <ActiveState
        onDeactivate={deactivate}
        updateStatus={updateStatus}
        busy={busy}
        error={error}
      />
    );
  }

  // Activation errors from this tab win over the startup error from Rust.
  const shownError = error ?? startupError;

  return (
    <div className="kv-license-tab">
      <div className="kv-license-tab__main">
        <h2 className="kv-section-title">License</h2>

        <TrialStatusCard trial={trial} />

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

        {shownError ? <LicenseErrorMessage error={shownError} /> : null}

        <div style={{ marginTop: 24 }}>
          <LicenseCheckoutButtons
            heading="Don't have a key yet?"
            purchaseUrls={purchaseUrls}
            onBuy={(url) => void buy(url)}
          />
        </div>
      </div>

      <LicenseTabFooter
        updateStatus={updateStatus}
      />
    </div>
  );
}

function LicenseTabFooter({ updateStatus }: { updateStatus: UpdateStatus | null }) {
  return (
    <footer className="kv-license-footnote-wrap">
      <p className="kv-license-help">
        Need help? Email <SupportEmailLink />
      </p>
      <AppVersionLine status={updateStatus} />
    </footer>
  );
}

/** "Free trial: MM:SS left", or a lock card once the trial is used up. */
function TrialStatusCard({ trial }: { trial: TrialStatus | null }) {
  if (trial?.active) {
    return (
      <div className="kv-license-status" data-state="trial" role="status">
        <span style={{ fontSize: 22 }} aria-hidden>
          ⏱
        </span>
        <div>
          <div style={{ fontWeight: 600 }}>
            Free trial: {formatTrialClock(trial.remaining_secs)} left
          </div>
          <div style={{ fontSize: 13, color: "var(--kv-settings-muted)" }}>
            Only counts down while the overlay is on.
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="kv-license-status" data-state="locked">
      <span style={{ fontSize: 22 }} aria-hidden>
        🔒
      </span>
      <div>
        <div style={{ fontWeight: 600 }}>
          {trial ? "Free trial ended" : "No active license"}
        </div>
        <div style={{ fontSize: 13, color: "var(--kv-settings-muted)" }}>
          Buy a lifetime license, then enter your key to activate this PC.
        </div>
      </div>
    </div>
  );
}

/** `MM:SS` (minutes reach 60 for a full one-hour trial). */
function formatTrialClock(secs: number): string {
  const clamped = Math.max(0, Math.floor(secs));
  const m = Math.floor(clamped / 60);
  const s = clamped % 60;
  return `${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
}

/** Opens a new email to the support address. */
function SupportEmailLink() {
  return (
    <button
      type="button"
      className="kv-license-footnote__link"
      onClick={() => void openShell(`mailto:${SUPPORT_EMAIL}`).catch(() => {})}
    >
      {SUPPORT_EMAIL}
    </button>
  );
}

/** Error text from Rust, with the support address turned into a link. */
function LicenseErrorMessage({ error }: { error: LicenseFailure }) {
  const parts = error.message.split(SUPPORT_EMAIL);
  return (
    <div className="kv-license-error" role="alert">
      {parts.map((part, i) => (
        <span key={i}>
          {i > 0 ? <SupportEmailLink /> : null}
          {part}
        </span>
      ))}
    </div>
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
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: 12,
          marginTop: 10,
          flexWrap: "wrap",
        }}
      >
        <button
          type="button"
          className="kv-button"
          onClick={() => onBuy(purchaseUrls?.lifetime ?? "")}
        >
          Buy a lifetime license for $2.99
        </button>
        <span style={{ fontSize: 13, color: "var(--kv-settings-muted)" }}>
          One-time payment. Your key arrives by email.
        </span>
      </div>
    </>
  );
}

interface ActiveProps {
  busy: boolean;
  error: LicenseFailure | null;
  onDeactivate: () => void;
  updateStatus: UpdateStatus | null;
}

function ActiveState({
  busy,
  error,
  onDeactivate,
  updateStatus,
}: ActiveProps) {
  const [deactivateConfirmOpen, setDeactivateConfirmOpen] = useState(false);

  return (
    <div className="kv-license-tab">
      <div className="kv-license-tab__main">
        <h2 className="kv-section-title">License</h2>
        <div className="kv-license-status" data-state="lifetime">
          <span style={{ fontSize: 22 }}>★</span>
          <div>
            <div style={{ fontWeight: 600 }}>Lifetime license</div>
            <div style={{ fontSize: 13, color: "var(--kv-settings-muted)" }}>
              Activated on this PC.
            </div>
          </div>
        </div>

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
                Remove the license from this PC?
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
                Tapt will stop working on this PC until you activate it again. This
                doesn't free up an activation on your key, but activating this PC
                again later is free.
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
                    idleLabel="Remove license"
                    busyLabel="Removing…"
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
                idleLabel="Remove license from this PC"
                busyLabel="Removing…"
              />
            </button>
          )}
        </div>

        {error ? <LicenseErrorMessage error={error} /> : null}
      </div>

      <LicenseTabFooter updateStatus={updateStatus} />
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
