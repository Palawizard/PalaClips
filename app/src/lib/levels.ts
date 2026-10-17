import { decibelLabel, decibels } from "./format.ts";

const FLOOR_DB = -60;
const RELEASE_DB_PER_S = 26;
const HOLD_MS = 1200;
const HOLD_FALL_DB_PER_S = 14;
const TEXT_EVERY_MS = 120;
const IDLE_AFTER_MS = 600;
const FRAME_MS = 1000 / 60;

interface Meter {
  fill: HTMLElement;
  hold: HTMLElement;
  text: HTMLElement;
  display: number;
  peak: number;
  peakUntil: number;
  textAt: number;
}

let targets: number[] = [];
const meters = new Map<number, Meter>();
const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
let focused = document.hasFocus();
let scheduled = false;
let lastFrame = 0;
let lastEvent = 0;

function position(db: number): number {
  if (!Number.isFinite(db)) return 0;
  return (Math.min(0, Math.max(FLOOR_DB, db)) - FLOOR_DB) / -FLOOR_DB;
}

function step(meter: Meter, target: number, now: number, dt: number, interpolate: boolean) {
  meter.display =
    !interpolate || target >= meter.display
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
  if (!interpolate || now - meter.textAt >= TEXT_EVERY_MS) {
    meter.textAt = now;
    meter.text.textContent = decibelLabel(meter.peak);
  }
}

function draw(now: number, interpolate: boolean): boolean {
  const dt = lastFrame ? Math.min(0.5, (now - lastFrame) / 1000) : 0;
  lastFrame = now;
  let moving = now - lastEvent < IDLE_AFTER_MS;
  for (const [index, meter] of meters) {
    step(meter, targets[index] ?? Number.NEGATIVE_INFINITY, now, dt, interpolate);
    if (position(meter.display) > 0 || position(meter.peak) > 0) moving = true;
  }
  return moving;
}

function tick(now: number) {
  scheduled = false;
  if (draw(now, !reducedMotion) && focused) schedule();
}

function schedule() {
  if (scheduled) return;
  scheduled = true;
  const wait = Math.max(0, FRAME_MS - (performance.now() - lastFrame));
  window.setTimeout(() => requestAnimationFrame(tick), wait);
}

window.addEventListener("focus", () => {
  focused = true;
  schedule();
});
window.addEventListener("blur", () => {
  focused = false;
});

export function pushLevels(levels: number[]) {
  targets = levels.map(decibels);
  lastEvent = performance.now();
  if (focused) schedule();
  else draw(lastEvent, false);
}

export function clearLevels() {
  targets = [];
  if (focused) schedule();
  else draw(performance.now(), false);
}

export function registerMeter(index: number, fill: HTMLElement, hold: HTMLElement, text: HTMLElement): () => void {
  const meter: Meter = {
    fill,
    hold,
    text,
    display: Number.NEGATIVE_INFINITY,
    peak: Number.NEGATIVE_INFINITY,
    peakUntil: 0,
    textAt: 0,
  };
  meters.set(index, meter);
  return () => {
    if (meters.get(index) === meter) meters.delete(index);
  };
}
