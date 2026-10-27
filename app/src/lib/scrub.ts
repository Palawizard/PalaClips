import type { PointerEvent } from "react";
import type { ScrubSession } from "./scrubSession.ts";

export type Scrubber = Pick<ScrubSession, "start" | "move" | "end">;

export const SNAP_PX = 8;

export function scrubTime(x: number, width: number, span: number, edges: number[]): number {
  if (width <= 0) return 0;
  const clamped = Math.min(width, Math.max(0, x));
  const near = edges.find((edge) => Math.abs((edge / span) * width - clamped) <= SNAP_PX);
  return near ?? (clamped / width) * span;
}

export function scrubHandlers(span: number, edges: number[], scrub: Scrubber) {
  const timeAt = (event: PointerEvent<HTMLElement>) => {
    const rect = event.currentTarget.getBoundingClientRect();
    return scrubTime(event.clientX - rect.left, rect.width, span, edges);
  };
  return {
    onPointerDown: (event: PointerEvent<HTMLElement>) => {
      if (event.button !== 0) return;
      event.preventDefault();
      event.currentTarget.setPointerCapture(event.pointerId);
      scrub.start(timeAt(event));
    },
    onPointerMove: (event: PointerEvent<HTMLElement>) => {
      if (event.currentTarget.hasPointerCapture(event.pointerId)) scrub.move(timeAt(event));
    },
    onPointerUp: () => scrub.end(),
    onPointerCancel: () => scrub.end(),
  };
}
