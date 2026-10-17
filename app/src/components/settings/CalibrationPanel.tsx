import { useEffect, useRef, useState } from "react";
import { errorText, t, useT } from "../../lib/i18n.ts";
import { type Calibration, type EngineConfig, calibrateAudio } from "../../lib/ipc.ts";
import { SettingRow } from "./Settings.tsx";
import type { ConfigDraft } from "./configDraft.tsx";

const PRIMARY =
  "pressable h-9 rounded-md bg-ink px-3.5 font-semibold text-ground hover:bg-ink/90 disabled:cursor-not-allowed disabled:opacity-50";
const SECONDARY =
  "pressable h-9 rounded-md border border-line-strong px-3.5 font-medium text-ink hover:bg-surface-hover disabled:cursor-not-allowed disabled:opacity-50";

type Phase = "idle" | "confirm" | "running" | "done";

function signed(ms: number): string {
  return t("units.ms", { value: `${ms > 0 ? "+" : ms < 0 ? "−" : ""}${Math.abs(Math.round(ms))}` });
}

interface ResultRow {
  key: string;
  title: string;
  measured: string;
  proposed: number | null;
}

function resultRows(calibration: Calibration, audio: EngineConfig["audio"]): ResultRow[] {
  const measuredOf = (kind: string) => {
    const track = calibration.tracks.find((entry) => entry.kind === kind);
    if (!track) return null;
    if (!track.measurement) return null;
    const late = Math.round(track.measurement.late_ms);
    return late < 0 ? t("settings.calibration.early", { ms: -late }) : t("settings.calibration.late", { ms: late });
  };
  const rows: ResultRow[] = [];
  if (audio.record_full_mix)
    rows.push({
      key: "mix",
      title: audio.full_mix_label,
      measured: measuredOf("mix") ?? t("settings.calibration.notMeasured"),
      proposed: calibration.mix_ms,
    });
  if (audio.record_microphone)
    rows.push({
      key: "microphone",
      title: audio.microphone_label,
      measured: measuredOf("microphone") ?? t("settings.calibration.micNotMeasured"),
      proposed: calibration.microphone_ms,
    });
  for (const track of audio.app_tracks)
    rows.push({
      key: `app:${track.exe_name}`,
      title: track.label,
      measured: t("settings.calibration.sameCapture"),
      proposed: calibration.apps.find((app) => app.exe_name === track.exe_name)?.offset_ms ?? null,
    });
  const reference = measuredOf("game");
  rows.push({
    key: "game",
    title: audio.game_label,
    measured: reference
      ? t("settings.calibration.reference", { measure: reference })
      : t("settings.calibration.referenceMissing"),
    proposed: calibration.game_ms,
  });
  return rows;
}

function withProposals(config: EngineConfig, calibration: Calibration): EngineConfig {
  const audio = config.audio;
  return {
    ...config,
    audio: {
      ...audio,
      full_mix_sync_offset_ms: calibration.mix_ms ?? audio.full_mix_sync_offset_ms,
      microphone_sync_offset_ms: calibration.microphone_ms ?? audio.microphone_sync_offset_ms,
      game_sync_offset_ms: calibration.game_ms ?? audio.game_sync_offset_ms,
      app_tracks: audio.app_tracks.map((track) => ({
        ...track,
        sync_offset_ms: calibration.apps.find((app) => app.exe_name === track.exe_name)?.offset_ms ?? track.sync_offset_ms,
      })),
    },
  };
}

export function CalibrationPanel({
  config,
  onRunning,
}: {
  config: ConfigDraft;
  onRunning: (running: boolean) => void;
}) {
  const t = useT();
  const [phase, setPhase] = useState<Phase>("idle");
  const [result, setResult] = useState<Calibration | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [calibrated, setCalibrated] = useState(false);
  const trigger = useRef<HTMLButtonElement>(null);
  const running = useRef<HTMLParagraphElement>(null);
  const results = useRef<HTMLElement>(null);
  const settled = useRef(false);

  useEffect(() => {
    if (!settled.current) {
      settled.current = true;
      return;
    }
    onRunning(phase === "running");
    if (phase === "running") running.current?.focus();
    else if (phase === "done") results.current?.focus();
    else if (phase === "idle") trigger.current?.focus();
  }, [phase]);

  const start = () => {
    setPhase("running");
    setError(null);
    calibrateAudio().then(
      (calibration) => {
        setResult(calibration);
        setCalibrated(true);
        setPhase("done");
      },
      (failure) => {
        setError(errorText(failure));
        setPhase("idle");
      },
    );
  };

  const rows = result ? resultRows(result, config.draft.audio) : [];
  const anyProposal = rows.some((row) => row.proposed !== null);

  return (
    <>
      <SettingRow label={t("settings.calibration.title")} error={error}>
        <button
          ref={trigger}
          type="button"
          disabled={phase === "running" || config.pendingRestart}
          onClick={() => setPhase("confirm")}
          className={SECONDARY}
        >
          {calibrated ? t("settings.calibration.recalibrate") : t("settings.calibration.calibrate")}
        </button>
      </SettingRow>

      {phase === "confirm" && (
        <div className="flex flex-col gap-3 border-b border-line bg-surface-2 px-5 py-4">
          <p className="text-[14px] text-ink">
            {t("settings.calibration.warning")}
            {config.armed && ` ${t("settings.calibration.warningArmed")}`}
          </p>
          {config.draft.audio.record_microphone && (
            <p className="text-[13px] text-ink-2">{t("settings.calibration.micHint")}</p>
          )}
          <p className="text-[13px] text-ink-2">{t("settings.calibration.voicemeeter")}</p>
          <span className="flex gap-2">
            <button type="button" onClick={start} className={PRIMARY}>
              {t("settings.calibration.start")}
            </button>
            <button type="button" onClick={() => setPhase(result ? "done" : "idle")} className={SECONDARY}>
              {t("settings.cancel")}
            </button>
          </span>
        </div>
      )}

      {phase === "running" && (
        <p
          ref={running}
          tabIndex={-1}
          role="status"
          className="border-b border-line bg-surface-2 px-5 py-4 text-[14px] text-ink outline-none"
        >
          {t("settings.calibration.running")}
        </p>
      )}

      {phase === "done" && result && (
        <section
          ref={results}
          tabIndex={-1}
          aria-label={t("settings.calibration.result")}
          className="border-b border-line bg-surface-2 px-5 py-4 outline-none"
        >
          <table className="w-full text-left text-[14px]">
            <thead>
              <tr>
                <th scope="col" className="label pb-2 font-medium text-ink-2">
                  {t("settings.calibration.track")}
                </th>
                <th scope="col" className="label pb-2 font-medium text-ink-2">
                  {t("settings.calibration.measure")}
                </th>
                <th scope="col" className="label pb-2 text-right font-medium text-ink-2">
                  {t("settings.calibration.proposed")}
                </th>
              </tr>
            </thead>
            <tbody>
              {rows.map((row) => (
                <tr key={row.key} className="border-t border-line">
                  <th scope="row" className="py-2 pr-4 font-semibold">
                    {row.title}
                  </th>
                  <td className="py-2 pr-4 text-ink-2">{row.measured}</td>
                  <td className="score py-2 text-right text-[16px]">
                    {row.proposed === null ? "—" : signed(row.proposed)}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
          {config.armed && (
            <p className="mt-3 text-[13px] text-ink">{t("settings.restartWarning")}</p>
          )}
          <span className="mt-3 flex gap-2">
            <button
              type="button"
              disabled={!anyProposal || config.pendingRestart}
              onClick={() => {
                config.commit(withProposals(config.draft, result));
                setPhase("idle");
              }}
              className={PRIMARY}
            >
              {t("settings.apply")}
            </button>
            <button type="button" onClick={() => setPhase("idle")} className={SECONDARY}>
              {t("settings.calibration.ignore")}
            </button>
          </span>
        </section>
      )}
    </>
  );
}
