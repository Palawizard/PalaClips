import { type KeyboardEvent, type PointerEvent, type RefObject, useRef } from "react";
import { preciseTime } from "../../lib/gallery.ts";
import { useT } from "../../lib/i18n.ts";
import { scrubTime } from "../../lib/scrub.ts";

export interface Range {
  start: number;
  end: number;
}

export const MIN_RANGE_SECS = 0.5;
const SECOND_STEPS = 1;

type Drag = "seek" | "start" | "end";

export function Timeline({
  duration,
  fps,
  range,
  position,
  onSeek,
  onRange,
}: {
  duration: number;
  fps: number;
  range: Range;
  position: RefObject<HTMLDivElement | null>;
  onSeek: (seconds: number) => void;
  onRange: (range: Range) => void;
}) {
  const t = useT();
  const track = useRef<HTMLDivElement>(null);
  const drag = useRef<Drag | null>(null);
  const span = Math.max(duration, 0.001);
  const frame = 1 / (fps > 0 ? fps : 60);
  const move = (kind: "start" | "end", seconds: number) => {
    if (kind === "start") {
      const start = Math.min(seconds, range.end - MIN_RANGE_SECS);
      onRange({ start, end: range.end });
      onSeek(start);
    } else onRange({ start: range.start, end: Math.max(seconds, range.start + MIN_RANGE_SECS) });
  };
  const apply = (kind: Drag, event: PointerEvent) => {
    const rect = track.current?.getBoundingClientRect();
    if (!rect) return;
    const x = event.clientX - rect.left;
    if (kind === "seek") onSeek(scrubTime(x, rect.width, span, [range.start, range.end]));
    else move(kind, scrubTime(x, rect.width, span, []));
  };
  const down = (kind: Drag) => (event: PointerEvent<HTMLElement>) => {
    event.preventDefault();
    event.stopPropagation();
    drag.current = kind;
    track.current?.setPointerCapture(event.pointerId);
    apply(kind, event);
  };
  const nudge = (kind: "start" | "end") => (event: KeyboardEvent<HTMLElement>) => {
    const step = event.key === "ArrowRight" ? 1 : event.key === "ArrowLeft" ? -1 : 0;
    if (!step) return;
    event.preventDefault();
    event.stopPropagation();
    const amount = event.shiftKey ? SECOND_STEPS : frame;
    move(kind, Math.min(span, Math.max(0, (kind === "start" ? range.start : range.end) + step * amount)));
  };
  const percent = (seconds: number) => `${(seconds / span) * 100}%`;

  return (
    <div
      ref={track}
      onPointerDown={down("seek")}
      onPointerMove={(event) => drag.current && apply(drag.current, event)}
      onPointerUp={() => {
        drag.current = null;
      }}
      onPointerCancel={() => {
        drag.current = null;
      }}
      className="timeline relative h-10 flex-1 cursor-pointer touch-none select-none"
    >
      <div
        ref={position}
        role="slider"
        tabIndex={0}
        aria-label={t("player.position")}
        aria-valuemin={0}
        aria-valuemax={Math.round(span)}
        aria-valuenow={0}
        className="absolute inset-x-0 top-1/2 h-2 -translate-y-1/2 rounded-[2px] bg-meter-ground focus-visible:outline-offset-4"
      />
      <div
        aria-hidden
        className="pointer-events-none absolute top-1/2 h-4 -translate-y-1/2 rounded-[2px] bg-ink/20"
        style={{ left: percent(range.start), width: percent(range.end - range.start) }}
      />
      <div
        aria-hidden
        data-played
        className="pointer-events-none absolute top-1/2 left-0 h-2 w-0 -translate-y-1/2 rounded-[2px] bg-ink"
      />
      <span
        aria-hidden
        data-playhead
        className="pointer-events-none absolute inset-y-1 left-0 w-0.5 -translate-x-1/2 rounded-full bg-ink"
      />
      {(["start", "end"] as const).map((kind) => {
        const value = kind === "start" ? range.start : range.end;
        return (
          <span
            key={kind}
            role="slider"
            tabIndex={0}
            aria-label={kind === "start" ? t("player.cutStart") : t("player.cutEnd")}
            aria-valuemin={0}
            aria-valuemax={Math.round(span)}
            aria-valuenow={Math.round(value * 10) / 10}
            aria-valuetext={preciseTime(value, fps)}
            aria-keyshortcuts={kind === "start" ? "I" : "O"}
            onPointerDown={down(kind)}
            onKeyDown={nudge(kind)}
            className={`cut-handle absolute top-0 bottom-0 flex w-3 cursor-ew-resize items-center justify-center ${
              kind === "start" ? "-translate-x-full" : ""
            }`}
            style={{ left: percent(value) }}
          >
            <span
              aria-hidden
              className={`h-7 w-1.5 bg-ink ${kind === "start" ? "rounded-l-[3px]" : "rounded-r-[3px]"}`}
            />
          </span>
        );
      })}
    </div>
  );
}
