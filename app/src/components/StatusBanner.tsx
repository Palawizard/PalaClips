import { HardDrive, Monitor, Save } from "lucide-react";
import { type ReactNode, useEffect, useState } from "react";
import type { Destination } from "../App.tsx";
import type { CaptureTarget } from "../lib/ipc.ts";
import { bytes, clock } from "../lib/format.ts";
import { message, t, useT } from "../lib/i18n.ts";
import type { Engine, ReplayPhase } from "../lib/useEngine.ts";

function Cell({ title, children, wide }: { title: string; children: ReactNode; wide?: boolean }) {
  return (
    <section
      aria-label={title}
      className={`flex min-w-0 flex-col gap-2 px-5 py-4 ${
        wide ? "flex-[2.2] max-[1000px]:basis-full" : "flex-1 max-[1000px]:basis-[220px]"
      }`}
    >
      {children}
    </section>
  );
}

function shortPath(path: string): string {
  const parts = path.split("\\").filter(Boolean);
  return parts.length > 3 ? [parts[0], "…", ...parts.slice(-2)].join("\\") : path;
}

function targetLabel(target: CaptureTarget | undefined): string {
  if (!target) return "—";
  if (target.kind === "primary_monitor") return t("messages.source_primary_screen");
  if (target.kind === "monitor") return target.device_name;
  return target.exe_name;
}

function ReplayWitness({ phase }: { phase: ReplayPhase }) {
  const armed = phase === "armed";
  return (
    <span
      aria-hidden
      className={`inline-block size-3 shrink-0 rounded-full border-2 ${
        armed ? "border-rec" : phase === "error" ? "border-warn" : "border-ink-3"
      }`}
    />
  );
}

const COUNTDOWN_TICK_MS = 250;

function ClipInProgress({ endsAt }: { endsAt: number }) {
  const t = useT();
  const [now, setNow] = useState(Date.now);
  useEffect(() => {
    const timer = window.setInterval(() => setNow(Date.now()), COUNTDOWN_TICK_MS);
    return () => window.clearInterval(timer);
  }, []);
  return (
    <p className="clip-progress flex items-center gap-2.5" role="status">
      <span aria-hidden className="inline-block size-2.5 shrink-0 rounded-full bg-rec" />
      <span className="font-semibold text-ink">{t("status.clipInProgress")}</span>
      <span className="score text-[20px] leading-none font-semibold text-ink">
        {clock(Math.max(0, Math.ceil((endsAt - now) / 1000)))}
      </span>
    </p>
  );
}

export function StatusBanner({ engine, onOpen }: { engine: Engine; onOpen: (destination: Destination) => void }) {
  const t = useT();
  const { status, config, phase, storage } = engine;
  const total = config?.replay.duration_secs ?? 0;
  const buffered = Math.min(status?.buffered_secs ?? 0, total);
  const armed = phase === "armed";
  const busy = phase === "arming" || phase === "disarming";
  const recording = engine.recording;
  const capture = status?.capture;
  const fellBack = status?.alerts.some((alert) => alert.code === "capture_fallback_monitor");
  const source = capture ? message(capture.source) : targetLabel(config?.capture);
  const clipEndsAt = status?.clip_ends_at_ms ?? null;

  return (
    <header className="flex border-b border-line bg-surface max-[1000px]:flex-wrap [&>section+section]:border-l [&>section+section]:border-line max-[1000px]:[&>section:nth-child(2)]:border-l-0 max-[1000px]:[&>section:not(:first-child)]:border-t">
      <Cell title={t("status.replay")} wide>
        <div className="flex items-center gap-2.5">
          <ReplayWitness phase={phase} />
          <h2 className="text-[15px] font-semibold">
            <span role="status">{t(`status.phase.${phase}`)}</span>
          </h2>
        </div>
        <div className="flex flex-wrap items-end gap-x-6 gap-y-2">
          <p className="flex items-baseline gap-2">
            <span className={`score text-[40px] leading-none font-semibold ${armed ? "text-ink" : "text-ink-3"}`}>
              {clock(buffered)}
            </span>
            <span className="score text-[20px] leading-none font-medium text-ink-3">/ {clock(total)}</span>
            <span className="text-ink-2">{t("status.inMemory")}</span>
          </p>
          <p className="pb-1 text-ink-2">
            <span className="score text-[17px] font-medium text-ink">{bytes(status?.buffer_bytes ?? 0)}</span>{" "}
            {t("status.inRam")}
          </p>
        </div>
        {clipEndsAt !== null && <ClipInProgress endsAt={clipEndsAt} />}
        <div className="mt-1 flex flex-wrap items-center gap-2">
          <button
            type="button"
            onClick={engine.save}
            disabled={!armed}
            className="pressable inline-flex h-9 items-center gap-2 rounded-md bg-ink px-3.5 font-semibold text-ground hover:bg-ink/90 disabled:cursor-not-allowed disabled:bg-surface-2 disabled:text-ink-3"
          >
            <Save size={16} strokeWidth={2.2} aria-hidden />
            {clipEndsAt !== null ? t("status.extend") : t("status.save")}
            {engine.saveKeys && (
              <span className="score text-[13px] font-medium opacity-70">{engine.saveKeys}</span>
            )}
          </button>
          {clipEndsAt !== null && (
            <button
              type="button"
              onClick={engine.finishClip}
              className="pressable h-9 rounded-md border border-line-strong px-3.5 font-medium text-ink hover:bg-surface-hover"
            >
              {t("status.finishClip")}
            </button>
          )}
          <button
            type="button"
            onClick={armed ? engine.disarm : engine.arm}
            disabled={busy}
            className="pressable h-9 rounded-md border border-line-strong px-3.5 font-medium text-ink hover:bg-surface-hover disabled:cursor-wait disabled:text-ink-3"
          >
            {armed || phase === "disarming" ? t("status.disarm") : t("status.arm")}
          </button>
        </div>
      </Cell>

      <Cell title={t("status.recording")}>
        <div className="flex items-center gap-2.5">
          <span
            aria-hidden
            className={`inline-block size-3 shrink-0 rounded-full border-2 ${
              recording ? "border-rec bg-rec" : "border-ink-3"
            }`}
          />
          <h2 className={`text-[15px] font-semibold ${recording ? "text-ink" : "text-ink-2"}`}>
            <span role="status">{recording ? t("status.recordingRunning") : t("status.recordingStopped")}</span>
          </h2>
        </div>
        <p className="flex items-baseline gap-2">
          <span className={`score text-[28px] leading-none font-semibold ${recording ? "text-ink" : "text-ink-3"}`}>
            {clock(recording ? (status?.recording_secs ?? 0) : 0)}
          </span>
          {recording && (
            <span className="score text-[15px] font-medium text-ink-2">{bytes(status?.recording_bytes ?? 0)}</span>
          )}
        </p>
        <button
          type="button"
          onClick={recording ? engine.stopRecording : engine.record}
          disabled={engine.recordPending || busy}
          className="pressable mt-auto inline-flex h-9 items-center gap-2 self-start rounded-md border border-line-strong px-3.5 font-medium text-ink hover:bg-surface-hover disabled:cursor-wait disabled:text-ink-3"
        >
          {recording ? t("status.stop") : t("status.start")}
          {engine.recordKeys && (
            <span className="score text-[13px] font-medium opacity-70">{engine.recordKeys}</span>
          )}
        </button>
      </Cell>

      <Cell title={t("status.source")}>
        <h2 className="flex min-w-0 items-center gap-2 text-[15px] font-semibold">
          <Monitor size={16} className="shrink-0 text-ink-3" aria-hidden />
          <span className="truncate" title={source}>
            {source}
          </span>
        </h2>
        <p className="score truncate text-[17px] font-medium whitespace-nowrap text-ink-2">
          {capture ? `${capture.width}×${capture.height} · ${t("units.fps", { value: capture.fps })}` : t("status.captureStopped")}
        </p>
        {fellBack && <p className="text-[13px] text-warn">{t("status.fellBack")}</p>}
        <button
          type="button"
          onClick={() => onOpen({ view: "capture" })}
          className="pressable mt-auto inline-flex h-9 items-center self-start rounded-md border border-line-strong px-3.5 font-medium text-ink hover:bg-surface-hover"
        >
          {t("status.changeSource")}
        </button>
      </Cell>

      <Cell title={t("status.disk")}>
        <h2 className="flex items-center gap-2 text-[15px] font-semibold">
          <HardDrive size={16} className="shrink-0 text-ink-3" aria-hidden />
          <span className="score text-[20px] leading-none">{storage ? bytes(storage.free_bytes) : "—"}</span>
          <span className="font-normal text-ink-2">{t("status.free")}</span>
        </h2>
        <p className="truncate text-[13px] text-ink-3" title={storage?.clips_dir}>
          {storage ? shortPath(storage.clips_dir) : ""}
        </p>
      </Cell>
    </header>
  );
}

