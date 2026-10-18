import { currentLanguage, t } from "./i18n.ts";
import type { Clip, ClipSort } from "./ipc.ts";

export type Period = "all" | "today" | "week" | "month";

const DAY_MS = 86_400_000;

function startOfDay(ms: number): number {
  const date = new Date(ms);
  date.setHours(0, 0, 0, 0);
  return date.getTime();
}

export function sinceOf(period: Period, now = Date.now()): number | null {
  if (period === "today") return startOfDay(now);
  if (period === "week") return startOfDay(now) - 6 * DAY_MS;
  if (period === "month") return startOfDay(now) - 29 * DAY_MS;
  return null;
}

export function dayLabel(ms: number, now = Date.now()): string {
  const day = startOfDay(ms);
  const today = startOfDay(now);
  if (day === today) return t("gallery.today");
  if (day === today - DAY_MS) return t("gallery.yesterday");
  const sameYear = new Date(ms).getFullYear() === new Date(now).getFullYear();
  return new Date(ms).toLocaleDateString(currentLanguage(), {
    day: "numeric",
    month: "short",
    ...(sameYear ? {} : { year: "numeric" }),
  });
}

const AUTO_NAME = /^(.*)_\d{4}-\d{2}-\d{2}_\d{2}-\d{2}-\d{2}(?:_replay)?(?:_\d+)?$/;

export function autoNamed(clip: Clip): boolean {
  const match = AUTO_NAME.exec(clip.name);
  return match !== null && match[1] === clip.game;
}

export function clipTitle(clip: Clip): string {
  return autoNamed(clip) ? clip.game || t("clips.fallbackName") : clip.name;
}

export function preciseTime(seconds: number, fps: number): string {
  const rate = fps > 0 ? fps : 60;
  const whole = Math.max(0, seconds);
  const frames = Math.floor((whole % 1) * rate + 1e-6);
  const minutes = Math.floor(whole / 60);
  const rest = Math.floor(whole % 60);
  const pad = (value: number) => value.toString().padStart(2, "0");
  return `${pad(minutes)}:${pad(rest)}.${pad(frames)}`;
}

export function timeOfDay(ms: number): string {
  return new Date(ms).toLocaleTimeString(currentLanguage(), { hour: "2-digit", minute: "2-digit" });
}

export type GridRow =
  | { kind: "day"; key: string; label: string; count: number }
  | { kind: "cards"; key: string; clips: Clip[]; first: number };

export function gridRows(clips: Clip[], columns: number, sort: ClipSort, now = Date.now()): GridRow[] {
  const byDay = sort === "newest" || sort === "oldest";
  const groups: { key: string; label: string; clips: Clip[]; first: number }[] = [];
  clips.forEach((clip, index) => {
    const key = byDay ? String(startOfDay(clip.modified_ms)) : "all";
    const last = groups[groups.length - 1];
    if (last && last.key === key) last.clips.push(clip);
    else groups.push({ key, label: byDay ? dayLabel(clip.modified_ms, now) : "", clips: [clip], first: index });
  });
  const rows: GridRow[] = [];
  for (const group of groups) {
    if (byDay) rows.push({ kind: "day", key: `day:${group.key}`, label: group.label, count: group.clips.length });
    for (let start = 0; start < group.clips.length; start += columns) {
      rows.push({
        kind: "cards",
        key: `cards:${group.key}:${start}`,
        clips: group.clips.slice(start, start + columns),
        first: group.first + start,
      });
    }
  }
  return rows;
}

export function neighbour(rows: GridRow[], index: number, step: "left" | "right" | "up" | "down"): number {
  const cardRows = rows.filter((row): row is Extract<GridRow, { kind: "cards" }> => row.kind === "cards");
  const total = cardRows.reduce((sum, row) => sum + row.clips.length, 0);
  if (step === "left") return Math.max(0, index - 1);
  if (step === "right") return Math.min(total - 1, index + 1);
  const at = cardRows.findIndex((row) => index >= row.first && index < row.first + row.clips.length);
  const row = cardRows[at];
  const target = cardRows[step === "up" ? at - 1 : at + 1];
  if (!row || !target) return index;
  return target.first + Math.min(index - row.first, target.clips.length - 1);
}
