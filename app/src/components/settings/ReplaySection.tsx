import { useEffect, useRef, useState } from "react";
import { bytes } from "../../lib/format.ts";
import { number, t, useT } from "../../lib/i18n.ts";
import {
  type EngineConfig,
  type GpuPriority,
  type HardwareEncoder,
  type MemoryStatus,
  type OutputResolution,
  type ReplayEstimate,
  type ReplayStorage,
  type VideoCodec,
  type VideoQuality,
  estimateReplay,
  listEncoders,
  memoryStatus,
} from "../../lib/ipc.ts";
import { NumberField, Segmented, Select, SettingRow } from "./Settings.tsx";
import { ConfigFooter, LoadError, useConfigDraft } from "./configDraft.tsx";

const RAM_SHARE_LIMIT = 0.4;

const DURATIONS = [5, 10, 15, 30, 45, 60, 90, 120, 180, 300, 600, 900, 1200, 1800, 2700, 3600];
const FPS = [30, 60, 120, 144, 165, 240];
const HEIGHTS = [720, 1080, 1440, 2160];
const KEYFRAMES = [0.5, 1, 2, 4];

const CODECS: VideoCodec[] = ["h264", "hevc", "av1"];
const QUALITIES: VideoQuality[] = ["quality", "balanced", "light", "custom"];

function durationLabel(seconds: number): string {
  if (seconds < 60) return t("units.seconds", { value: seconds });
  const minutes = seconds / 60;
  return Number.isInteger(minutes)
    ? t("units.minutes", { value: minutes })
    : t("units.minutesSeconds", { minutes: Math.floor(minutes), seconds: seconds % 60 });
}

function withCurrent(presets: number[], current: number): number[] {
  return presets.includes(current) ? presets : [...presets, current].sort((a, b) => a - b);
}

function resolutionKey(resolution: OutputResolution): string {
  if (resolution.kind === "native") return "native";
  return HEIGHTS.includes(resolution.height) ? String(resolution.height) : "free";
}

export function ReplaySection() {
  const t = useT();
  const { config, loadError } = useConfigDraft();
  const [encoders, setEncoders] = useState<HardwareEncoder[]>([]);
  const [memory, setMemory] = useState<MemoryStatus | null>(null);
  const [estimate, setEstimate] = useState<ReplayEstimate | null>(null);
  const estimateRef = useRef<HTMLElement>(null);
  const draft = config?.draft ?? null;

  useEffect(() => {
    listEncoders().then(setEncoders, () => setEncoders([]));
    memoryStatus().then(setMemory, () => setMemory(null));
  }, []);

  useEffect(() => {
    if (!draft) return;
    let current = true;
    estimateReplay(draft).then(
      (value) => current && setEstimate(value),
      () => current && setEstimate(null),
    );
    return () => {
      current = false;
    };
  }, [draft]);

  if (!config || !draft) return <LoadError message={loadError} />;
  const change = config.change;

  const video = (patch: Partial<EngineConfig["video"]>) => change({ ...draft, video: { ...draft.video, ...patch } });
  const replay = (patch: Partial<EngineConfig["replay"]>) => change({ ...draft, replay: { ...draft.replay, ...patch } });

  const available = (codec: VideoCodec) => encoders.some((encoder) => encoder.codec === codec && !encoder.failure);
  const unavailableCodecs = CODECS.filter((codec) => encoders.length > 0 && !available(codec)).map((codec) =>
    t(`codecs.${codec}`),
  );
  const ramShare = estimate && memory ? estimate.bytes / memory.available_bytes : 0;
  const tooMuchRam = draft.replay.storage === "ram" && ramShare > RAM_SHARE_LIMIT;
  const sourceHeight = estimate?.source_height ?? 4320;
  const currentHeight = draft.video.resolution.kind === "height" ? draft.video.resolution.height : null;
  const heights = HEIGHTS.filter((height) => height < sourceHeight || height === currentHeight);

  return (
    <>
      <section
        ref={estimateRef}
        tabIndex={-1}
        aria-label={t("settings.replay.estimate")}
        className="border-b border-line px-5 py-4"
      >
        <p className="flex flex-wrap items-baseline gap-x-2">
          <span className="score text-[28px] leading-none font-semibold">
            {estimate ? t("settings.replay.approx", { size: bytes(estimate.bytes) }) : "—"}
          </span>
          <span className="text-ink-2">
            {draft.replay.storage === "ram" ? t("settings.replay.inRam") : t("settings.replay.onDisk")}
          </span>
        </p>
        {estimate && (
          <p className="score mt-1 text-[15px] text-ink-2">
            {estimate.width}×{estimate.height} · {t("units.fps", { value: draft.video.fps })} ·{" "}
            {t("units.mbps", { value: number(estimate.video_kbps / 1000, 1) })} ·{" "}
            {t("settings.replay.audioTracks", { count: estimate.audio_tracks })}
          </p>
        )}
        {tooMuchRam && memory && (
          <div role="alert" className="mt-3 flex flex-wrap items-center gap-3">
            <p className="flex-1 text-[13px] text-warn">
              {t("settings.replay.ramAlert", {
                percent: Math.round(ramShare * 100),
                free: bytes(memory.available_bytes),
              })}
            </p>
            <button
              type="button"
              onClick={() => replay({ storage: "disk" })}
              className="pressable h-9 rounded-md border border-line-strong px-3.5 font-medium text-ink hover:bg-surface-hover"
            >
              {t("settings.replay.toDisk")}
            </button>
          </div>
        )}
      </section>

      <SettingRow label={t("settings.replay.duration")}>
        <Select
          label={t("settings.replay.duration")}
          value={draft.replay.duration_secs}
          options={withCurrent(DURATIONS, draft.replay.duration_secs).map((seconds) => ({ value: seconds, label: durationLabel(seconds) }))}
          onChange={(duration_secs) => replay({ duration_secs })}
        />
      </SettingRow>
      <SettingRow label={t("settings.replay.buffer")} help={t("settings.replay.bufferHelp")}>
        <Segmented<ReplayStorage>
          label={t("settings.replay.buffer")}
          value={draft.replay.storage}
          options={[
            { value: "ram", label: t("settings.replay.ram") },
            { value: "disk", label: t("settings.replay.disk") },
          ]}
          onChange={(storage) => replay({ storage })}
        />
      </SettingRow>
      <SettingRow
        label={t("settings.replay.codec")}
        help={
          unavailableCodecs.length > 0
            ? t("settings.replay.codecUnavailable", { codecs: unavailableCodecs.join(", ") })
            : undefined
        }
      >
        <Segmented<VideoCodec>
          label={t("settings.replay.codec")}
          value={draft.video.codec}
          options={CODECS.map((codec) => ({
            value: codec,
            label: t(`codecs.${codec}`),
            disabled: encoders.length > 0 && !available(codec),
          }))}
          onChange={(codec) => video({ codec })}
        />
      </SettingRow>
      <SettingRow label={t("settings.replay.quality")}>
        <span className="flex items-center gap-2">
          <Segmented<VideoQuality>
            label={t("settings.replay.quality")}
            value={draft.video.quality}
            options={QUALITIES.map((value) => ({ value, label: t(`settings.replay.qualities.${value}`) }))}
            onChange={(quality) => video({ quality })}
          />
          {draft.video.quality === "custom" && (
            <NumberField
              value={Math.round(draft.video.bitrate_kbps / 1000)}
              min={1}
              max={200}
              unit={t("units.mbpsUnit")}
              label={t("settings.replay.bitrate")}
              onCommit={(mbps) => video({ bitrate_kbps: mbps * 1000 })}
            />
          )}
        </span>
      </SettingRow>
      <SettingRow label={t("settings.replay.fps")}>
        <span className="flex items-center gap-2">
          <Select
            label={t("settings.replay.fps")}
            value={FPS.includes(draft.video.fps) ? draft.video.fps : 0}
            options={[
              ...FPS.map((fps) => ({ value: fps, label: t("units.fps", { value: fps }) })),
              { value: 0, label: t("settings.replay.free") },
            ]}
            onChange={(fps) => video({ fps: fps === 0 ? 50 : fps })}
          />
          {!FPS.includes(draft.video.fps) && (
            <NumberField
              value={draft.video.fps}
              min={10}
              max={240}
              unit={t("units.fpsUnit")}
              label={t("settings.replay.fpsFree")}
              onCommit={(fps) => video({ fps })}
            />
          )}
        </span>
      </SettingRow>
      <SettingRow label={t("settings.replay.resolution")}>
        <span className="flex items-center gap-2">
          <Select
            label={t("settings.replay.resolution")}
            value={resolutionKey(draft.video.resolution)}
            options={[
              { value: "native", label: t("settings.replay.native") },
              ...heights.map((height) => ({ value: String(height), label: t("units.height", { value: height }) })),
              { value: "free", label: t("settings.replay.free") },
            ]}
            onChange={(key) =>
              video({
                resolution:
                  key === "native"
                    ? { kind: "native" }
                    : { kind: "height", height: key === "free" ? Math.min(900, sourceHeight) : Number(key) },
              })
            }
          />
          {resolutionKey(draft.video.resolution) === "free" && draft.video.resolution.kind === "height" && (
            <NumberField
              value={draft.video.resolution.height}
              min={Math.min(360, sourceHeight)}
              max={sourceHeight}
              unit={t("units.heightUnit")}
              label={t("settings.replay.heightFree")}
              onCommit={(height) => video({ resolution: { kind: "height", height } })}
            />
          )}
        </span>
      </SettingRow>
      <SettingRow label={t("settings.replay.keyframes")} help={t("settings.replay.keyframesHelp")}>
        <Select
          label={t("settings.replay.keyframes")}
          value={draft.video.keyframe_interval_secs}
          options={withCurrent(KEYFRAMES, draft.video.keyframe_interval_secs).map((seconds) => ({
            value: seconds,
            label: t("units.seconds", { value: number(seconds, 1) }),
          }))}
          onChange={(keyframe_interval_secs) => video({ keyframe_interval_secs })}
        />
      </SettingRow>
      <SettingRow label={t("settings.replay.gpuPriority")} help={t("settings.replay.gpuPriorityHelp")}>
        <Segmented<GpuPriority>
          label={t("settings.replay.gpuPriority")}
          value={draft.video.gpu_priority}
          options={[
            { value: "game", label: t("settings.replay.gpuPriorities.game") },
            { value: "capture", label: t("settings.replay.gpuPriorities.capture") },
          ]}
          onChange={(gpu_priority) => video({ gpu_priority })}
        />
      </SettingRow>
      <ConfigFooter config={config} focusAfter={estimateRef} />
    </>
  );
}
