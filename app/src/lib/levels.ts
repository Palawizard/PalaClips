import { decibelLabel, decibels } from "./format.ts";

const FLOOR_DB = -60;
const RELEASE_DB_PER_S = 26;
const HOLD_MS = 1200;
const HOLD_FALL_DB_PER_S = 14;
const TEXT_EVERY_MS = 120;
const IDLE_AFTER_MS = 600;

interface Meter {
  fill: HTMLElement;
  hold: HTMLElement;
  text: HTMLElement;
  display: number;
  peak: number;
  peakUntil: number;
  textAt: number;
}

const targets = new Map<string, number>();
const meters = new Map<string, Meter>();
const reducedMotion =
  typeof window !== "undefined" && window.matchMedia("(prefers-reduced-motion: reduce)").matches;
let frame = 0;
let previous = 0;
let lastEvent = 0;

function position(db: number): number {
  if (!Number.isFinite(db)) return 0;
  return (Math.min(0, Math.max(FLOOR_DB, db)) - FLOOR_DB) / -FLOOR_DB;
}

function step(meter: Meter, target: number, now: number, dt: number) {
  meter.display =
    reducedMotion || target >= meter.display
      ? target
      : Math.max(target, meter.display - RELEASE_DB_PER_S * dt);
  if (meter.display >= meter.peak) {
    meter.peak = meter.display;
    meter.peakUntil = now + HOLD_MS;
  } else if (now > meter.peakUntil) {
    meter.peak = Math.max(meter.display, meter.peak - HOLD_FALL_DB_PER_S * dt);
  }
  meter.fill.style.transform = `scaleX(${position(meter.display)})`;
  meter.hold.style.transform = `translateX(${position(meter.peak) * 100}%)`;
  if (now - meter.textAt >= TEXT_EVERY_MS) {
    meter.textAt = now;
    meter.text.textContent = decibelLabel(meter.peak);
  }
}

function tick(now: number) {
  const dt = previous ? Math.min(0.1, (now - previous) / 1000) : 0;
  previous = now;
  let active = now - lastEvent < IDLE_AFTER_MS;
  for (const [title, meter] of meters) {
    step(meter, targets.get(title) ?? Number.NEGATIVE_INFINITY, now, dt);
    if (position(meter.display) > 0 || position(meter.peak) > 0) active = true;
  }
  frame = active ? requestAnimationFrame(tick) : 0;
  if (!frame) previous = 0;
}

function wake() {
  if (!frame) frame = requestAnimationFrame(tick);
}

export function pushLevels(levels: [string, number][]) {
  for (const [title, peak] of levels) targets.set(title, decibels(peak));
  lastEvent = performance.now();
  wake();
}

export function clearLevels() {
  targets.clear();
  wake();
}

export function registerMeter(
  title: string,
  fill: HTMLElement,
  hold: HTMLElement,
  text: HTMLElement,
): () => void {
  meters.set(title, {
    fill,
    hold,
    text,
    display: Number.NEGATIVE_INFINITY,
    peak: Number.NEGATIVE_INFINITY,
    peakUntil: 0,
    textAt: 0,
  });
  wake();
  return () => {
    if (meters.get(title)?.fill === fill) meters.delete(title);
  };
}
