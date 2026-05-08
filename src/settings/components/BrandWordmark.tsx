import type { CSSProperties } from "react";
import { useCallback, useState } from "react";

import { PRODUCT_DISPLAY_NAME, PRODUCT_MARK } from "../../branding";

/** Vite serves `public/` at site root. */
const SETTINGS_LOGO_URL = `${import.meta.env.BASE_URL}brand-logo.png`;

interface Props {
  className?: string;
  /** Additional style on the wrapping span (e.g. layout in headers). */
  style?: CSSProperties;
}

/**
 * Optional sidebar glyph + “Tapt” + accent “.” Put `public/brand-logo.png`
 * for the tray-style mark; omit the file if you prefer text only.
 */
export default function BrandWordmark({ className, style }: Props) {
  const [hideLogoImg, setHideLogoImg] = useState(false);
  const onLogoError = useCallback(() => setHideLogoImg(true), []);

  return (
    <span
      className={`kv-brand-header ${className ?? ""}`.trim()}
      style={style}
      role="group"
      aria-label={PRODUCT_DISPLAY_NAME}
    >
      {!hideLogoImg && (
        <img
          className="kv-brand-logo"
          src={SETTINGS_LOGO_URL}
          alt=""
          width={28}
          height={28}
          decoding="async"
          onError={onLogoError}
        />
      )}
      <span className="kv-brand-wordmark" aria-hidden="true">
        <span className="kv-brand-wordmark__name">{PRODUCT_MARK}</span>
        <span className="kv-brand-wordmark__dot">.</span>
      </span>
    </span>
  );
}
