import type { KeyboardAnchor, SnapCorner } from "../types";

/** Move key-feed overlay one pixel in a screen-relative direction (+1 / −1 to offset_x). */
export function deltaFeedOffsetX(
  direction: "left" | "right",
  snap: SnapCorner,
): number {
  const towardRight =
    snap === "top-left" ||
    snap === "bottom-left" ||
    snap === "free";
  const sign = towardRight ? 1 : -1;
  return direction === "right" ? sign : -sign;
}

/** Move key-feed overlay one pixel up/down (+1 / −1 to offset_y). */
export function deltaFeedOffsetY(direction: "up" | "down", snap: SnapCorner): number {
  const liftUpIncreases =
    snap === "bottom-left" || snap === "bottom-right";
  const towardUp = liftUpIncreases ? 1 : -1;
  return direction === "up" ? towardUp : -towardUp;
}

/** Horizontal nudge (+1 → right on screen except right-anchored rows). */
export function deltaKeyboardOffsetX(
  direction: "left" | "right",
  anchor: KeyboardAnchor,
): number {
  const flip =
    anchor === "top-right" ||
    anchor === "middle-right" ||
    anchor === "bottom-right";
  const towardRight = flip ? -1 : 1;
  return direction === "right" ? towardRight : -towardRight;
}

/** Vertical nudge (+1 ↑ moves up visually for typical bottom-pin anchors). */
export function deltaKeyboardOffsetY(
  direction: "up" | "down",
  anchor: KeyboardAnchor,
): number {
  const towardUp =
    anchor === "bottom-left" ||
    anchor === "bottom-center" ||
    anchor === "bottom-right"
      ? 1
      : -1;
  return direction === "up" ? towardUp : -towardUp;
}

export function clampInt(n: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, Math.trunc(n)));
}

/** True if applying `delta` would change the clamped value (button should stay enabled). */
export function canNudge(
  current: number,
  delta: number,
  min: number,
  max: number,
): boolean {
  return clampInt(current + delta, min, max) !== current;
}
