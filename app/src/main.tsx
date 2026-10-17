import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App.tsx";
import { getTheme } from "./lib/ipc.ts";
import { applyTheme } from "./lib/theme.ts";
import "./styles.css";

const THEME_WAIT_MS = 250;

const root = document.getElementById("root");
if (!root) throw new Error("élément #root introuvable");

const savedTheme = getTheme().catch(() => "system" as const);
savedTheme.then(applyTheme);

Promise.race([savedTheme, new Promise((resolve) => window.setTimeout(resolve, THEME_WAIT_MS))]).then(() => {
  createRoot(root).render(
    <StrictMode>
      <App />
    </StrictMode>,
  );
});
