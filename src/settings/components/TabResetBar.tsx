interface Props {
  title: string;
  onReset: () => void;
  resetDisabled?: boolean;
}

export default function TabResetBar({
  title,
  onReset,
  resetDisabled = false,
}: Props) {
  return (
    <div className="kv-tab-heading">
      <h2 className="kv-section-title">{title}</h2>
      <button
        type="button"
        className="kv-reset-defaults-btn"
        disabled={resetDisabled}
        onClick={() => onReset()}
      >
        Reset to defaults
      </button>
    </div>
  );
}
