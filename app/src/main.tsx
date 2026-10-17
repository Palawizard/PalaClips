import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App.tsx";
import { getTheme } from "./lib/ipc.ts";
import { applyTheme } from "./lib/theme.ts";
import "./styles.css";

applyTheme("system");
getTheme().then(applyTheme, () => applyTheme("system"));

const root = document.getElementById("root");
if (!root) throw new Error("élément #root introuvable");

createRoot(root).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
