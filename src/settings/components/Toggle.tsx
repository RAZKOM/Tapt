import type { CSSProperties } from "react";

interface Props {
  label: string;
  description?: string;
  checked: boolean;
  onChange: (next: boolean) => void;
}

export default function Toggle({ label, description, checked, onChange }: Props) {
  return (
    <label style={rowStyle}>
      <div style={{ display: "flex", flexDirection: "column", gap: 2 }}>
        <span style={{ fontSize: 14, fontWeight: 500 }}>{label}</span>
        {description && (
          <span style={{ fontSize: 12, color: "var(--kv-settings-muted)" }}>
            {description}
          </span>
        )}
      </div>
      <input
        type="checkbox"
        checked={checked}
        onChange={(e) => onChange(e.target.checked)}
        style={{ width: 18, height: 18 }}
      />
    </label>
  );
}

const rowStyle: CSSProperties = {
  display: "flex",
  alignItems: "center",
  justifyContent: "space-between",
  paddingBlock: 10,
  cursor: "pointer",
};
