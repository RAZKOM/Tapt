import type { CSSProperties } from "react";

interface Props {
  label: string;
  value: string;
  onChange: (next: string) => void;
}

export default function ColorPicker({ label, value, onChange }: Props) {
  return (
    <label style={rowStyle}>
      <span style={{ fontSize: 14, fontWeight: 500 }}>{label}</span>
      <span style={{ display: "inline-flex", alignItems: "center", gap: 8 }}>
        <span
          style={{
            display: "inline-block",
            width: 22,
            height: 22,
            borderRadius: 6,
            background: value,
            border: "1px solid var(--kv-settings-border)",
          }}
        />
        <input
          type="color"
          value={value}
          onChange={(e) => onChange(e.target.value)}
          style={{ width: 36, height: 28, border: "none", background: "transparent" }}
        />
        <code style={{ fontSize: 12, color: "var(--kv-settings-muted)" }}>
          {value}
        </code>
      </span>
    </label>
  );
}

const rowStyle: CSSProperties = {
  display: "flex",
  alignItems: "center",
  justifyContent: "space-between",
  paddingBlock: 10,
};
