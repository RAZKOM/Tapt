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
    <div className="kv-nudge-cross-wrap" aria-label="Move one pixel">
      <div className="kv-nudge-cross">
        <div />
        <button
          type="button"
          className="kv-nudge-btn kv-nudge-btn--arrow"
          disabled={dvU}
          aria-label="Move up one pixel"
          onClick={() => onVertical("up")}
        >
          ↑
        </button>
        <div />
        <button
          type="button"
          className="kv-nudge-btn kv-nudge-btn--arrow"
          disabled={dhL}
          aria-label="Move left one pixel"
          onClick={() => onHorizontal("left")}
        >
          ←
        </button>
        <div />
        <button
          type="button"
          className="kv-nudge-btn kv-nudge-btn--arrow"
          disabled={dhR}
          aria-label="Move right one pixel"
          onClick={() => onHorizontal("right")}
        >
          →
        </button>
        <div />
        <button
          type="button"
          className="kv-nudge-btn kv-nudge-btn--arrow"
          disabled={dvD}
          aria-label="Move down one pixel"
          onClick={() => onVertical("down")}
        >
          ↓
        </button>
        <div />
      </div>
    </div>
  );
}
