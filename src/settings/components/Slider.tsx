import type { CSSProperties } from "react";

interface Props {
  label: string;
  value: number;
  min: number;
  max: number;
  step?: number;
  onChange: (next: number) => void;
  formatter?: (n: number) => string;
  disabled?: boolean;
}

export default function Slider({
  label,
  value,
  min,
  max,
  step = 1,
  onChange,
  formatter,
  disabled = false,
}: Props) {
  const display = formatter ? formatter(value) : String(value);
  return (
    <label style={{ ...fieldStyle, opacity: disabled ? 0.5 : 1 }}>
      <div style={rowStyle}>
        <span>{label}</span>
        <span style={{ color: "var(--kv-settings-muted)" }}>{display}</span>
      </div>
      <input
        type="range"
        min={min}
        max={max}
        step={step}
        value={value}
        disabled={disabled}
        onChange={(e) => onChange(Number(e.target.value))}
        style={{ width: "100%" }}
      />
    </label>
  );
}

const fieldStyle: CSSProperties = {
  display: "flex",
  flexDirection: "column",
  gap: 6,
  paddingBlock: 10,
};

const rowStyle: CSSProperties = {
  display: "flex",
  justifyContent: "space-between",
  fontSize: 14,
  fontWeight: 500,
};
