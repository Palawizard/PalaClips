import type { LucideIcon } from "lucide-react";
import { AppWindow, Gamepad2, Layers, Mic } from "lucide-react";
import en from "../locales/en.json";
import fr from "../locales/fr.json";
import type { EngineConfig, TrackKind } from "./ipc.ts";

const APP_COLORS = 11;

const DEFAULT_TITLES = new Map<string, TrackKind>(
  [fr, en].flatMap(({ tracks }) => [
    [tracks.mix, "mix"],
    [tracks.microphone, "microphone"],
    [tracks.game, "game"],
  ]),
);

function hash(text: string): number {
  let value = 0;
  for (const char of text) value = (value * 31 + char.charCodeAt(0)) >>> 0;
  return value;
}

export type TrackLabels = Pick<EngineConfig["audio"], "full_mix_label" | "microphone_label" | "game_label">;

export function kindOfTitle(title: string, labels?: TrackLabels | null): TrackKind {
  if (labels?.full_mix_label === title) return "mix";
  if (labels?.microphone_label === title) return "microphone";
  if (labels?.game_label === title) return "game";
  return DEFAULT_TITLES.get(title) ?? "application";
}

export function trackColor(kind: TrackKind, title: string, appOrder: string[]): string {
  if (kind === "mix") return "var(--track-mix)";
  if (kind === "microphone") return "var(--track-mic)";
  if (kind === "game") return "var(--track-game)";
  const index = appOrder.indexOf(title);
  const slot = (index >= 0 ? index : hash(title)) % APP_COLORS;
  return `var(--track-${slot + 1})`;
}

export function trackIcon(kind: TrackKind): LucideIcon {
  if (kind === "mix") return Layers;
  if (kind === "microphone") return Mic;
  if (kind === "game") return Gamepad2;
  return AppWindow;
}
