import type { Theme } from "./ipc.ts";

const systemLight = window.matchMedia("(prefers-color-scheme: light)");
let chosen: Theme = "system";

function paint() {
  const light = chosen === "light" || (chosen === "system" && systemLight.matches);
  document.documentElement.dataset.theme = light ? "light" : "dark";
}

systemLight.addEventListener("change", paint);

export function applyTheme(theme: Theme) {
  chosen = theme;
  paint();
}
