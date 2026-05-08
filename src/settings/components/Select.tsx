import type { CSSProperties } from "react";

interface Option<T extends string> {
  value: T;
  label: string;
}

interface Props<T extends string> {
  label: string;
  value: T;
  options: Option<T>[];
  onChange: (next: T) => void;
  disabled?: boolean;
}

export default function Select<T extends string>({
  label,
  value,
  options,
  onChange,
  disabled = false,
}: Props<T>) {
  return (
    <label style={{ ...rowStyle, opacity: disabled ? 0.5 : 1 }}>
      <span style={{ fontSize: 14, fontWeight: 500 }}>{label}</span>
      <select
        value={value}
        disabled={disabled}
        onChange={(e) => onChange(e.target.value as T)}
        style={selectStyle}
      >
        {options.map((opt) => (
          <option key={opt.value} value={opt.value}>
            {opt.label}
          </option>
        ))}
      </select>
    </label>
  );
}

const rowStyle: CSSProperties = {
  display: "flex",
  alignItems: "center",
  justifyContent: "space-between",
  paddingBlock: 10,
};

const selectStyle: CSSProperties = {
  background: "var(--kv-settings-card)",
  border: "1px solid var(--kv-settings-border)",
  borderRadius: 6,
  padding: "6px 10px",
  fontSize: 14,
  color: "var(--kv-settings-fg)",
  minWidth: 160,
};
