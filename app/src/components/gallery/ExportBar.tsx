import { Check, ChevronDown, Copy, FolderOpen, Shrink } from "lucide-react";
import { useEffect, useId, useState } from "react";
import type { DraftRequest } from "../../lib/compressDrafts.ts";
import { clock } from "../../lib/format.ts";
import { preciseTime } from "../../lib/gallery.ts";
import { errorText, number, useT } from "../../lib/i18n.ts";
import {
  type Clip,
  type TrackFile,
  type TrackKind,
  cancelExport,
  copyClip,
  onExportFinished,
  onExportProgress,
  revealClip,
  startExport,
} from "../../lib/ipc.ts";
import { Segmented } from "../settings/Settings.tsx";
import type { Range } from "./Timeline.tsx";

type Phase = { kind: "idle" } | { kind: "running"; id: number; progress: number } | { kind: "done"; path: string };

const WHOLE_CLIP_SLACK = 0.05;

const SECONDARY =
  "pressable inline-flex h-9 items-center gap-2 rounded-md border border-line-strong px-3.5 font-medium text-ink hover:bg-surface-hover";

export function ExportBar({
  clip,
  tracks,
  titles,
  kinds,
  colors,
  audible,
  volumes,
  range,
  onCompress,
}: {
  clip: Clip;
  tracks: TrackFile[] | null;
  titles: string[];
  kinds: TrackKind[];
  colors: string[];
  audible: boolean[];
  volumes: number[];
  range: Range;
  onCompress: (requests: DraftRequest[]) => void;
}) {
  const t = useT();
  const nameId = useId();
  const optionsId = useId();
  const [precise, setPrecise] = useState(false);
  const [mixed, setMixed] = useState(false);
  const [manual, setManual] = useState<number[] | null>(null);
  const [expanded, setExpanded] = useState(false);
  const [name, setName] = useState(`${clip.name}_${t("export.suffix")}`);
  const [phase, setPhase] = useState<Phase>({ kind: "idle" });
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const all = (tracks ?? []).map((_, index) => index);
  const chosen = manual ?? all.filter((index) => audible[index]);
  const doubled =
    mixed &&
    chosen.some((index) => kinds[index] === "mix") &&
    chosen.some((index) => kinds[index] === "application" || kinds[index] === "game");
  const time = (seconds: number) => (precise ? preciseTime(seconds, clip.fps) : clock(seconds));

  useEffect(() => {
    const progress = onExportProgress((id, value) =>
      setPhase((current) => (current.kind === "running" && current.id === id ? { ...current, progress: value } : current)),
    );
    const finished = onExportFinished((done) =>
      setPhase((current) => {
        if (current.kind !== "running" || current.id !== done.id) return current;
        if (done.path) return { kind: "done", path: done.path };
        if (done.error && done.error.code !== "export_cancelled") setError(errorText(done.error));
        return { kind: "idle" };
      }),
    );
    return () => {
      progress.then((stop) => stop());
      finished.then((stop) => stop());
    };
  }, []);

  const toggleTrack = (index: number) => {
    const current = manual ?? chosen;
    setManual(current.includes(index) ? current.filter((item) => item !== index) : [...current, index].sort((a, b) => a - b));
  };

  const start = () => {
    setError(null);
    setNotice(null);
    startExport({
      path: clip.path,
      name: name.trim(),
      start_secs: range.start,
      end_secs: range.end,
      precise,
      tracks: chosen,
      gains: chosen.map((index) => volumes[index] ?? 1),
      mix: mixed,
      mix_title: t("tracks.mix"),
    }).then(
      (id) => setPhase({ kind: "running", id, progress: 0 }),
      (failure) => setError(errorText(failure)),
    );
  };

  const fileName = phase.kind === "done" ? (phase.path.split("\\").pop() ?? phase.path) : "";

  return (
    <section aria-labelledby="export-title" className="flex flex-col gap-3 border-t border-line bg-surface px-5 py-3">
      <div className="flex flex-wrap items-center gap-x-4 gap-y-2">
        <h2 id="export-title" className="flex items-baseline gap-2.5 text-[15px] font-semibold">
          {t("export.title")}
          <span className="score text-[17px] font-medium">{`${time(range.start)} → ${time(range.end)}`}</span>
          <span className="text-[13px] font-normal text-ink-2">
            {t("export.duration", { value: clock(range.end - range.start) })}
          </span>
        </h2>
        <Segmented<"fast" | "precise">
          label={t("export.mode")}
          value={precise ? "precise" : "fast"}
          options={[
            { value: "fast", label: t("export.fast") },
            { value: "precise", label: t("export.precise") },
          ]}
          onChange={(value) => setPrecise(value === "precise")}
        />
        <Segmented<"separate" | "mixed">
          label={t("export.audio")}
          value={mixed ? "mixed" : "separate"}
          options={[
            { value: "separate", label: t("export.separate") },
            { value: "mixed", label: t("export.mixed") },
          ]}
          onChange={(value) => setMixed(value === "mixed")}
        />
        <button
          type="button"
          aria-expanded={expanded}
          aria-controls={optionsId}
          onClick={() => setExpanded((current) => !current)}
          className="pressable inline-flex h-9 items-center gap-1.5 rounded-md px-2 text-[13px] font-medium text-ink-2 hover:bg-surface-hover hover:text-ink"
        >
          {t("export.options", { count: chosen.length, total: all.length })}
          <ChevronDown size={15} aria-hidden className={`export-chevron ${expanded ? "rotate-180" : ""}`} />
        </button>
        <span className="ml-auto flex items-center gap-3">
          {phase.kind === "running" ? (
            <>
              <span
                role="progressbar"
                aria-label={t("export.progress")}
                aria-valuemin={0}
                aria-valuemax={100}
                aria-valuenow={Math.round(phase.progress * 100)}
                className="relative h-2 w-40 overflow-hidden rounded-[2px] bg-meter-ground"
              >
                <span
                  className="export-fill absolute inset-0 origin-left bg-ink"
                  style={{ transform: `scaleX(${phase.progress})` }}
                />
              </span>
              <span className="score w-12 text-right text-[15px] text-ink-2">
                {t("units.percent", { value: number(phase.progress * 100) })}
              </span>
              <button type="button" onClick={() => cancelExport(phase.id)} className={SECONDARY}>
                {t("settings.cancel")}
              </button>
            </>
          ) : (
            <>
            <button
              type="button"
              onClick={() => {
                const whole = range.start <= WHOLE_CLIP_SLACK && range.end >= clip.duration_secs - WHOLE_CLIP_SLACK;
                onCompress([whole ? { path: clip.path } : { path: clip.path, startSecs: range.start, endSecs: range.end }]);
              }}
              className={SECONDARY}
            >
              <Shrink size={16} aria-hidden />
              {t("gallery.compress")}
            </button>
            <button
              type="button"
              disabled={name.trim() === ""}
              onClick={start}
              className="pressable h-9 rounded-md bg-ink px-3.5 font-semibold text-ground hover:bg-ink/90 disabled:cursor-not-allowed disabled:bg-surface-2 disabled:text-ink-3"
            >
              {t("export.start")}
            </button>
            </>
          )}
        </span>
      </div>
      {doubled && <p className="text-[13px] text-warn">{t("export.mixDoubled")}</p>}
      {chosen.length === 0 && tracks && tracks.length > 0 && (
        <p className="text-[13px] text-warn">{t("export.noTracks")}</p>
      )}
      {expanded && (
        <div id={optionsId} className="flex flex-col gap-3">
          <p className="text-[13px] text-ink-2">{precise ? t("export.preciseHelp") : t("export.fastHelp")}</p>
          {tracks && tracks.length > 0 && (
            <fieldset className="flex flex-wrap items-center gap-2">
              <legend className="sr-only">{t("export.tracks")}</legend>
              {tracks.map((track, index) => {
                const on = chosen.includes(index);
                return (
                  <button
                    key={track.path}
                    type="button"
                    role="checkbox"
                    aria-checked={on}
                    onClick={() => toggleTrack(index)}
                    className={`pressable inline-flex h-8 items-center gap-2 rounded-md border px-2.5 text-[13px] font-medium ${
                      on ? "border-line-strong bg-surface-2 text-ink" : "border-line text-ink-2 hover:text-ink"
                    }`}
                  >
                    <span
                      aria-hidden
                      className="flex size-4 items-center justify-center rounded-[3px] border"
                      style={on ? { background: colors[index], borderColor: colors[index] } : { borderColor: "var(--ink-3)" }}
                    >
                      {on && <Check size={12} strokeWidth={3} className="text-ground" />}
                    </span>
                    {titles[index]}
                  </button>
                );
              })}
              {manual && (
                <button
                  type="button"
                  onClick={() => setManual(null)}
                  className="pressable h-8 rounded-md px-2 text-[13px] font-medium text-ink-2 underline-offset-4 hover:text-ink hover:underline"
                >
                  {t("export.followMix")}
                </button>
              )}
            </fieldset>
          )}
          <div className="flex flex-wrap items-center gap-2">
            <label htmlFor={nameId} className="text-[13px] text-ink-2">
              {t("export.name")}
            </label>
            <span className="flex items-center rounded-md border border-ink-3 bg-surface pr-2.5 hover:border-ink-2">
              <input
                id={nameId}
                type="text"
                value={name}
                spellCheck={false}
                onChange={(event) => setName(event.target.value)}
                className="h-9 w-[26rem] max-w-full rounded-md bg-transparent px-2.5 text-[14px] text-ink outline-none"
              />
              <span className="text-[14px] text-ink-3">{t("export.extension")}</span>
            </span>
          </div>
        </div>
      )}
      {phase.kind === "done" && (
        <div role="status" className="flex flex-wrap items-center gap-3 text-[14px]">
          <span className="min-w-0 truncate">{t("export.done", { name: fileName })}</span>
          <button type="button" onClick={() => revealClip(phase.path).catch((failure) => setError(errorText(failure)))} className={SECONDARY}>
            <FolderOpen size={16} aria-hidden />
            {t("export.show")}
          </button>
          <button
            type="button"
            onClick={() =>
              copyClip(phase.path).then(
                () => setNotice(t("gallery.copied")),
                (failure) => setError(errorText(failure)),
              )
            }
            className={SECONDARY}
          >
            <Copy size={16} aria-hidden />
            {t("export.copy")}
          </button>
          {notice && <span className="text-ink-2">{notice}</span>}
        </div>
      )}
      {error && (
        <p role="alert" className="text-[13px] text-warn">
          {error}
        </p>
      )}
    </section>
  );
}
