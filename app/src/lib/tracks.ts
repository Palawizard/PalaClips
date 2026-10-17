import type { LucideIcon } from "lucide-react";
import { AppWindow, Gamepad2, Layers, Mic } from "lucide-react";

export const MIX_TITLE = "Mix complet";
export const MIC_TITLE = "Micro";
export const GAME_TITLE = "Jeu";

const APP_COLORS = 11;

function hash(text: string): number {
  let value = 0;
  for (const char of text) value = (value * 31 + char.charCodeAt(0)) >>> 0;
  return value;
}

export function trackColor(title: string, appOrder: string[]): string {
  if (title === MIX_TITLE) return "var(--track-mix)";
  if (title === MIC_TITLE) return "var(--track-mic)";
  if (title === GAME_TITLE) return "var(--track-game)";
  const index = appOrder.indexOf(title);
  const slot = (index >= 0 ? index : hash(title)) % APP_COLORS;
  return `var(--track-${slot + 1})`;
}

export function trackIcon(title: string): LucideIcon {
  if (title === MIX_TITLE) return Layers;
  if (title === MIC_TITLE) return Mic;
  if (title === GAME_TITLE) return Gamepad2;
  return AppWindow;
}
