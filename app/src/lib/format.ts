import { currentLanguage, number, t } from "./i18n.ts";

export function clock(totalSeconds: number): string {
  const seconds = Math.max(0, Math.floor(totalSeconds));
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const rest = seconds % 60;
  const pad = (value: number) => value.toString().padStart(2, "0");
  return hours > 0 ? `${hours}:${pad(minutes)}:${pad(rest)}` : `${pad(minutes)}:${pad(rest)}`;
}

const KIB = 1024;
const MIB = KIB * 1024;
const GIB = MIB * 1024;
const TIB = GIB * 1024;

export function bytes(value: number, megabyteDigits = 0): string {
  if (value >= TIB) return t("units.tb", { value: number(value / TIB, 1) });
  if (value >= GIB) return t("units.gb", { value: number(value / GIB, 1) });
  if (value >= MIB) return t("units.mb", { value: number(value / MIB, megabyteDigits) });
  return t("units.kb", { value: number(value / KIB) });
}

export function decibels(linear: number): number {
  return linear > 0 ? 20 * Math.log10(linear) : Number.NEGATIVE_INFINITY;
}

export function decibelLabel(db: number): string {
  return Number.isFinite(db) && db > -60 ? t("units.db", { value: number(db, 1).replace("-", "−") }) : "−∞";
}

export function relativeTime(epochMs: number, now = Date.now()): string {
  const minutes = Math.round((now - epochMs) / 60_000);
  if (minutes < 1) return t("time.now");
  if (minutes < 60) return t("time.minutesAgo", { count: minutes });
  const hours = Math.round(minutes / 60);
  if (hours < 24) return t("time.hoursAgo", { count: hours });
  return new Date(epochMs).toLocaleDateString(currentLanguage(), { day: "numeric", month: "short" });
}
