import React from "react";
import ReactDOM from "react-dom/client";

import SettingsApp from "./settings/SettingsApp";

import "./styles/tokens.css";

const root = ReactDOM.createRoot(
  document.getElementById("root") as HTMLElement,
);

root.render(
  <React.StrictMode>
    <SettingsApp />
  </React.StrictMode>,
);
