import { type CSSProperties, useEffect, useRef, useState } from "react";
import type { TrackInfo, TrackState } from "../lib/ipc.ts";
import { registerMeter } from "../lib/levels.ts";
import { trackColor, trackIcon } from "../lib/tracks.ts";

const STATE_LABEL: Record<TrackState, string> = {
  capturing: "Capture",
  waiting: "En attente de l'appli",
  silent: "Muette : vérifie la sortie",
  unavailable: "Périphérique indisponible",
};

const STATE_TONE: Record<TrackState, string> = {
  capturing: "text-ink-2",
  waiting: "text-ink-3",
  silent: "text-warn",
  unavailable: "text-warn",
};

const TICKS = [-48, -36, -24, -12, -6];

const COLUMNS =
  "grid grid-cols-[52px_minmax(0,1.3fr)_minmax(140px,2fr)_76px_minmax(0,1fr)] items-center gap-x-4";

function TrackRow({
  track,
  index,
  color,
  flashing,
}: {
  track: TrackInfo;
  index: number;
  color: string;
  flashing: boolean;
}) {
  const fill = useRef<HTMLDivElement>(null);
  const hold = useRef<HTMLDivElement>(null);
  const peak = useRef<HTMLSpanElement>(null);
  const Icon = trackIcon(track.title);

  useEffect(() => {
    if (!fill.current || !hold.current || !peak.current) return;
    return registerMeter(track.title, fill.current, hold.current, peak.current);
  }, [track.title]);

  return (
    <div
      role="row"
      data-flash={flashing}
      style={{ "--row-color": color } as CSSProperties}
      className={`roster-row ${COLUMNS} border-b border-line px-5 py-3`}
    >
      <span
        role="cell"
        className="score flex h-8 w-11 items-center justify-center rounded-[4px] text-[18px] font-semibold text-ground"
        style={{ background: color }}
      >
        {String(index + 1).padStart(2, "0")}
      </span>
      <span role="cell" className="flex min-w-0 items-center gap-2.5">
        <Icon size={18} className="shrink-0" style={{ color }} aria-hidden />
        <span className="min-w-0">
          <span className="block truncate font-semibold">{track.title}</span>
          {track.source && (
            <span className="block truncate text-[12px] text-ink-3" title={track.source}>
              {track.source}
            </span>
          )}
        </span>
      </span>
      <span role="cell" className="relative block h-2.5 overflow-hidden rounded-[2px] bg-meter-ground">
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
      <span role="cell" ref={peak} className="score text-right text-[19px] leading-none font-semibold text-ink">
        −∞
      </span>
      <span role="cell" className={`truncate text-[13px] ${track.state ? STATE_TONE[track.state] : "text-ink-3"}`}>
        {track.state ? STATE_LABEL[track.state] : "—"}
      </span>
    </div>
  );
}

export function Roster({
  tracks,
  appOrder,
  flashAt,
}: {
  tracks: TrackInfo[];
  appOrder: string[];
  flashAt: number;
}) {
  const [flashing, setFlashing] = useState(false);

  useEffect(() => {
    if (!flashAt) return;
    setFlashing(true);
    const timer = window.setTimeout(() => setFlashing(false), 650);
    return () => window.clearTimeout(timer);
  }, [flashAt]);

  return (
    <section aria-labelledby="roster-title" className="min-w-0 border-r border-line max-[1000px]:border-r-0">
      <div className="flex items-baseline justify-between px-5 pt-5 pb-3">
        <h2 id="roster-title" className="text-[17px] font-semibold">
          Pistes audio
        </h2>
        <p className="text-[13px] text-ink-3">
          {tracks.length} {tracks.length > 1 ? "pistes" : "piste"} dans chaque clip, dans cet ordre
        </p>
      </div>
      <div role="table" aria-label="Pistes audio capturées" className="text-[14px]">
        <div
          role="row"
          className={`${COLUMNS} border-y border-line px-5 py-2`}
        >
          <span role="columnheader" className="label">
            N°
          </span>
          <span role="columnheader" className="label">
            Piste
          </span>
          <span role="columnheader" className="label">
            Niveau
          </span>
          <span role="columnheader" className="label text-right">
            Crête
          </span>
          <span role="columnheader" className="label">
            État
          </span>
        </div>
        {tracks.map((track, index) => (
          <TrackRow
            key={track.title}
            track={track}
            index={index}
            color={trackColor(track.title, appOrder)}
            flashing={flashing}
          />
        ))}
      </div>
    </section>
  );
}
