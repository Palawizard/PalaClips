import { type RefObject, useEffect, useRef } from "react";
import { registerMeter } from "../lib/levels.ts";

const TICKS = [-48, -36, -24, -12, -6];

export function useTrackMeter(index: number | null) {
  const fill = useRef<HTMLSpanElement>(null);
  const hold = useRef<HTMLSpanElement>(null);
  const peak = useRef<HTMLSpanElement>(null);

  useEffect(() => {
    if (index === null || !fill.current || !hold.current || !peak.current) return;
    return registerMeter(index, fill.current, hold.current, peak.current);
  }, [index]);

  return { fill, hold, peak };
}

export function MeterBar({
  color,
  fill,
  hold,
  cell = false,
  className = "h-2.5",
}: {
  color: string;
  fill: RefObject<HTMLSpanElement | null>;
  hold: RefObject<HTMLSpanElement | null>;
  cell?: boolean;
  className?: string;
}) {
  return (
    <span
      role={cell ? "cell" : undefined}
      aria-hidden={cell ? undefined : true}
      className={`relative block overflow-hidden rounded-[2px] bg-meter-ground ${className}`}
    >
      <span ref={fill} className="meter-fill absolute inset-0" style={{ background: color, transform: "scaleX(0)" }} />
      {TICKS.map((db) => (
        <span
          key={db}
          aria-hidden
          className="absolute inset-y-0 w-px bg-ground/70"
          style={{ left: `${((db + 60) / 60) * 100}%` }}
        />
      ))}
      <span ref={hold} className="meter-hold absolute inset-0" style={{ transform: "translateX(0%)" }}>
        <span className="absolute inset-y-0 left-0 w-[2px] -translate-x-full" style={{ background: color }} />
      </span>
    </span>
  );
}
