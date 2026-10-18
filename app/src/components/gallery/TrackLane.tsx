import { useEffect, useRef } from "react";
import { number, useT } from "../../lib/i18n.ts";
import { trackPeaks } from "../../lib/ipc.ts";
import type { TrackMix } from "../../lib/playerAudio.ts";
import type { Range } from "./Timeline.tsx";

function cssColor(element: HTMLElement, color: string): string {
  const probe = document.createElement("span");
  probe.style.color = color;
  element.appendChild(probe);
  const resolved = getComputedStyle(probe).color;
  probe.remove();
  return resolved;
}

function Waveform({ path, color, duration, range }: { path: string; color: string; duration: number; range: Range }) {
  const canvas = useRef<HTMLCanvasElement>(null);
  const peaks = useRef<Uint8Array | null>(null);

  useEffect(() => {
    const element = canvas.current;
    if (!element) return;
    let alive = true;
    const draw = () => {
      const values = peaks.current;
      const context = element.getContext("2d");
      if (!context) return;
      const scale = window.devicePixelRatio || 1;
      const width = Math.max(1, Math.round(element.clientWidth * scale));
      const height = Math.max(1, Math.round(element.clientHeight * scale));
      element.width = width;
      element.height = height;
      context.clearRect(0, 0, width, height);
      if (!values || values.length === 0) return;
      context.fillStyle = cssColor(element, color);
      const middle = height / 2;
      for (let x = 0; x < width; x++) {
        const from = Math.floor((x / width) * values.length);
        const to = Math.max(from + 1, Math.floor(((x + 1) / width) * values.length));
        let peak = 0;
        for (let index = from; index < to; index++) peak = Math.max(peak, values[index] ?? 0);
        const half = Math.max(scale * 0.5, (peak / 255) * middle);
        context.fillRect(x, middle - half, 1, half * 2);
      }
    };
    trackPeaks(path).then(
      (buffer) => {
        if (!alive) return;
        peaks.current = new Uint8Array(buffer);
        draw();
      },
      () => undefined,
    );
    const observer = new ResizeObserver(draw);
    observer.observe(element);
    const theme = new MutationObserver(draw);
    theme.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });
    return () => {
      alive = false;
      observer.disconnect();
      theme.disconnect();
    };
  }, [path, color]);

  const span = Math.max(duration, 0.001);
  return (
    <div className="relative h-9 min-w-0 flex-1 overflow-hidden rounded-[2px] bg-meter-ground">
      <canvas ref={canvas} aria-hidden className="waveform absolute inset-0 size-full" />
      <div aria-hidden className="absolute inset-y-0 left-0 bg-ground/55" style={{ width: `${(range.start / span) * 100}%` }} />
      <div aria-hidden className="absolute inset-y-0 right-0 bg-ground/55" style={{ width: `${(1 - range.end / span) * 100}%` }} />
      <span
        aria-hidden
        className="absolute inset-y-0 w-0.5 -translate-x-1/2 bg-ink"
        style={{ left: "calc(var(--progress, 0) * 100%)" }}
      />
    </div>
  );
}

export function TrackLane({
  index,
  title,
  path,
  color,
  duration,
  range,
  mix,
  audible,
  onMix,
}: {
  index: number;
  title: string;
  path: string;
  color: string;
  duration: number;
  range: Range;
  mix: TrackMix;
  audible: boolean;
  onMix: (mix: TrackMix) => void;
}) {
  const t = useT();
  const idle = "border-line-strong text-ink-2 hover:bg-surface-hover hover:text-ink";
  const toggle = "pressable score flex size-8 items-center justify-center rounded-md border text-[15px] font-semibold";
  return (
    <li
      data-audible={audible}
      className="track-lane grid grid-cols-[44px_minmax(110px,180px)_minmax(0,1fr)_auto_minmax(120px,170px)] items-center gap-x-4 border-b border-line px-5 py-2"
    >
      <span
        className={`score flex h-8 w-11 items-center justify-center rounded-[4px] text-[18px] font-semibold ${
          audible ? "text-ground" : "border border-line-strong bg-surface-2 text-ink-2"
        }`}
        style={audible ? { background: color } : undefined}
      >
        {String(index + 1).padStart(2, "0")}
      </span>
      <span className={`truncate text-[15px] font-semibold ${audible ? "text-ink" : "text-ink-2"}`} title={title}>
        {title}
      </span>
      <Waveform path={path} color={color} duration={duration} range={range} />
      <span className="flex gap-1.5">
        <button
          type="button"
          aria-pressed={mix.mute}
          aria-label={t("player.mute", { name: title })}
          onClick={() => onMix({ ...mix, mute: !mix.mute })}
          className={`${toggle} ${mix.mute ? "border-ink bg-ink text-ground" : idle}`}
        >
          {t("player.muteShort")}
        </button>
        <button
          type="button"
          aria-pressed={mix.solo}
          aria-label={t("player.solo", { name: title })}
          onClick={() => onMix({ ...mix, solo: !mix.solo })}
          className={`${toggle} ${mix.solo ? "text-ground" : idle}`}
          style={mix.solo ? { background: color, borderColor: color } : undefined}
        >
          {t("player.soloShort")}
        </button>
      </span>
      <label className="flex items-center gap-2.5">
        <input
          type="range"
          min={0}
          max={100}
          step={1}
          value={Math.round(mix.volume * 100)}
          aria-label={t("player.volume", { name: title })}
          onChange={(event) => onMix({ ...mix, volume: Number(event.target.value) / 100 })}
          className="volume-slider min-w-0 flex-1"
          style={{ accentColor: color }}
        />
        <span className="score w-12 text-right text-[15px] text-ink-2">
          {t("units.percent", { value: number(mix.volume * 100) })}
        </span>
      </label>
    </li>
  );
}
