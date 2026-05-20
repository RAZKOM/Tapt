/** Visible product title (sentence / window chrome). Includes the branded period. */
export const PRODUCT_DISPLAY_NAME = "Tapt.";

/** Word before the accented period lockup (“Tapt” + colored “.”). */
export const PRODUCT_MARK = "Tapt";

/**
 * AuthForge self-service portal — sign in with license key + purchase email (magic code).
 * Purchases use Stripe checkout URLs in the app (“Buy” buttons); the portal is for help / device resets.
 *
 * @see https://docs.authforge.cc/features/portal
 */
export const AUTHFORGE_CUSTOMER_PORTAL_URL = "https://portal.authforge.cc";

/** Product download page — keep in sync with `src-tauri/src/version.rs` `RELEASES_URL`. */
export const RELEASES_URL = "https://razkom.com/tapt/";
