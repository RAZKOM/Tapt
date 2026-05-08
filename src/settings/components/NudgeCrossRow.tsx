interface Props {
  /** Per-axis disables so buttons don't fire past slider limits */
  horizontalDisabled?: [boolean, boolean];
  verticalDisabled?: [boolean, boolean];
  onHorizontal: (direction: "left" | "right") => void;
  onVertical: (direction: "up" | "down") => void;
}

/** Screen-relative (←→↑↓) nudge buttons, 1 px per click. */
export default function NudgeCrossRow({
  horizontalDisabled,
  verticalDisabled,
  onHorizontal,
  onVertical,
}: Props) {
  const [dhL = false, dhR = false] = horizontalDisabled ?? [];
  const [dvU = false, dvD = false] = verticalDisabled ?? [];

  return (
    <div className="kv-nudge-cross-wrap" aria-label="Nudge by one pixel">
      <div className="kv-nudge-cross">
        <div />
        <button
          type="button"
          className="kv-nudge-btn kv-nudge-btn--arrow"
          disabled={dvU}
          aria-label="Nudge one pixel upward on screen"
          onClick={() => onVertical("up")}
        >
          ↑
        </button>
        <div />
        <button
          type="button"
          className="kv-nudge-btn kv-nudge-btn--arrow"
          disabled={dhL}
          aria-label="Nudge one pixel left on screen"
          onClick={() => onHorizontal("left")}
        >
          ←
        </button>
        <div />
        <button
          type="button"
          className="kv-nudge-btn kv-nudge-btn--arrow"
          disabled={dhR}
          aria-label="Nudge one pixel right on screen"
          onClick={() => onHorizontal("right")}
        >
          →
        </button>
        <div />
        <button
          type="button"
          className="kv-nudge-btn kv-nudge-btn--arrow"
          disabled={dvD}
          aria-label="Nudge one pixel downward on screen"
          onClick={() => onVertical("down")}
        >
          ↓
        </button>
        <div />
      </div>
    </div>
  );
}
