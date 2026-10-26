import type { PointerEvent } from "react";

export const SNAP_PX = 8;

export function scrubTime(x: number, width: number, span: number, edges: number[]): number {
  if (width <= 0) return 0;
  const clamped = Math.min(width, Math.max(0, x));
  const near = edges.find((edge) => Math.abs((edge / span) * width - clamped) <= SNAP_PX);
  return near ?? (clamped / width) * span;
}

export function scrubHandlers(span: number, edges: number[], onSeek: (seconds: number) => void) {
  const seekAt = (event: PointerEvent<HTMLElement>) => {
    const rect = event.currentTarget.getBoundingClientRect();
    onSeek(scrubTime(event.clientX - rect.left, rect.width, span, edges));
  };
  return {
    onPointerDown: (event: PointerEvent<HTMLElement>) => {
      if (event.button !== 0) return;
      event.preventDefault();
      event.currentTarget.setPointerCapture(event.pointerId);
      seekAt(event);
    },
    onPointerMove: (event: PointerEvent<HTMLElement>) => {
      if (event.currentTarget.hasPointerCapture(event.pointerId)) seekAt(event);
    },
  };
}
