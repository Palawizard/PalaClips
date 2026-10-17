import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App.tsx";
import { setLanguage } from "./lib/i18n.ts";
import { getLanguage, getTheme } from "./lib/ipc.ts";
import { applyTheme } from "./lib/theme.ts";
import "./styles.css";

const STARTUP_WAIT_MS = 250;

const root = document.getElementById("root");
if (!root) throw new Error("#root");

const savedTheme = getTheme().catch(() => "system" as const);
savedTheme.then(applyTheme);
const savedLanguage = getLanguage().then(
  (view) => setLanguage(view.resolved),
  () => undefined,
);

Promise.race([
  Promise.all([savedTheme, savedLanguage]),
  new Promise((resolve) => window.setTimeout(resolve, STARTUP_WAIT_MS)),
]).then(() => {
  createRoot(root).render(
    <StrictMode>
      <App />
    </StrictMode>,
  );
});
