import type { CSSProperties, ReactNode } from "react";

interface Props {
  title?: string;
  description?: string;
  children: ReactNode;
}

export default function Section({ title, description, children }: Props) {
  return (
    <section style={cardStyle}>
      {title && <h3 style={titleStyle}>{title}</h3>}
      {description && <p style={descStyle}>{description}</p>}
      <div style={{ display: "flex", flexDirection: "column" }}>{children}</div>
    </section>
  );
}

const cardStyle: CSSProperties = {
  background: "var(--kv-settings-card)",
  border: "1px solid var(--kv-settings-border)",
  borderRadius: 12,
  padding: 16,
  boxShadow: "var(--kv-settings-shadow)",
  marginBlockEnd: 16,
};

const titleStyle: CSSProperties = {
  margin: 0,
  fontSize: 14,
  fontWeight: 600,
  letterSpacing: "0.02em",
  textTransform: "uppercase",
  color: "var(--kv-settings-muted)",
};

const descStyle: CSSProperties = {
  margin: "4px 0 12px",
  fontSize: 13,
  color: "var(--kv-settings-muted)",
};
