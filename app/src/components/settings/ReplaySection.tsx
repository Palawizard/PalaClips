import { useEffect, useMemo, useRef, useState } from "react";
import { bytes } from "../../lib/format.ts";
import { number, t, useT } from "../../lib/i18n.ts";
import {
  type EncoderChoice,
  type EngineConfig,
  type GpuPriority,
  type EncoderInfo,
  type MemoryStatus,
  type OutputResolution,
  type ReplayEstimate,
  type ReplayStorage,
  type VideoCodec,
  type VideoQuality,
  encodesOnGpu,
  listEncoders,
  memoryStatus,
} from "../../lib/ipc.ts";
import { recordVideo, recordVideoOf, recordingConfig } from "../../lib/config.ts";
import { useEstimate } from "../../lib/useEstimate.ts";
import { NumberField, Segmented, Select, SettingRow, Switch } from "./Settings.tsx";
import { ConfigFooter, LoadError, useConfigDraft } from "./configDraft.tsx";

const RAM_SHARE_LIMIT = 0.4;

const AFTER_PRESETS = [0, 10, 30, 60, 300, 600];
const MAX_AFTER_SECS = 600;

export const DURATIONS = [5, 10, 15, 30, 45, 60, 90, 120, 180, 300, 600, 900, 1200, 1800, 2700, 3600];
const FPS = [30, 60, 120, 144, 165, 240];
const HEIGHTS = [720, 1080, 1440, 2160];
const KEYFRAMES = [0.5, 1, 2, 4];

const CODECS: VideoCodec[] = ["h264", "hevc", "av1"];
const ENCODER_CHOICES: EncoderChoice[] = ["auto", "hardware", "cpu"];
const CPU_FPS = 30;
const MF_TWO_WORKERS_UP_TO = 1920 * 1080 * 60;
const CPU_ENCODER_MEMORY = {
  h264: { base: 25e6, perPixel: 70 },
  h264FourWorkers: { base: 40e6, perPixel: 84 },
  av1: { base: 25e6, perPixel: 57 },
};

export function cpuEncoderBytes(codec: VideoCodec, width: number, height: number, fps: number): number {
  const pixels = width * height;
  const memory =
    codec === "av1"
      ? CPU_ENCODER_MEMORY.av1
      : codec === "h264"
        ? pixels * fps <= MF_TWO_WORKERS_UP_TO
          ? CPU_ENCODER_MEMORY.h264
          : CPU_ENCODER_MEMORY.h264FourWorkers
        : null;
  return memory ? memory.base + memory.perPixel * pixels : 0;
}

const CPU_HEIGHT = 1080;

export function cpuVideo(
  video: EngineConfig["video"],
  sourceHeight: number,
  encoders: EncoderInfo[],
): EngineConfig["video"] {
  const cpuCodecs = encoders.filter((encoder) => !encoder.hardware && !encoder.failure).map((encoder) => encoder.codec);
  const tall = video.resolution.kind === "native" ? sourceHeight > CPU_HEIGHT : video.resolution.height > CPU_HEIGHT;
  const fallbackCodec = cpuCodecs.includes("h264") ? "h264" : cpuCodecs[0];
  return {
    ...video,
    fps: Math.min(video.fps, CPU_FPS),
    resolution: tall ? { kind: "height", height: CPU_HEIGHT } : video.resolution,
    codec: cpuCodecs.includes(video.codec) ? video.codec : (fallbackCodec ?? video.codec),
  };
}

const QUALITIES: VideoQuality[] = ["quality", "balanced", "light", "custom"];

export function durationLabel(seconds: number): string {
  if (seconds < 60) return t("units.seconds", { value: seconds });
  const minutes = seconds / 60;
  return Number.isInteger(minutes)
    ? t("units.minutes", { value: minutes })
    : t("units.minutesSeconds", { minutes: Math.floor(minutes), seconds: seconds % 60 });
}

export function withCurrent(presets: number[], current: number): number[] {
  return presets.includes(current) ? presets : [...presets, current].sort((a, b) => a - b);
}

function resolutionKey(resolution: OutputResolution): string {
  if (resolution.kind === "native") return "native";
  return HEIGHTS.includes(resolution.height) ? String(resolution.height) : "free";
}

type VideoFields = Pick<EngineConfig["video"], "codec" | "quality" | "bitrate_kbps" | "fps" | "resolution">;

function PartTitle({ id, children }: { id: string; children: string }) {
  return (
    <h3 id={id} className="border-b border-line px-5 pt-7 pb-3 text-[17px] font-semibold">
      {children}
    </h3>
  );
}

function VideoRows({
  video,
  scope,
  codecs,
  codecHelp,
  sourceHeight,
  onChange,
}: {
  video: VideoFields;
  scope: "replay" | "recording";
  codecs: { value: VideoCodec; label: string; disabled?: boolean }[];
  codecHelp?: string;
  sourceHeight: number;
  onChange: (patch: Partial<VideoFields>) => void;
}) {
  const t = useT();
  const named = (setting: string) => (scope === "replay" ? setting : t("settings.replay.forRecording", { setting }));
  const currentHeight = video.resolution.kind === "height" ? video.resolution.height : null;
  const heights = HEIGHTS.filter((height) => height < sourceHeight || height === currentHeight);
  return (
    <>
      <SettingRow label={t("settings.replay.codec")} help={codecHelp}>
        <Segmented<VideoCodec>
          label={named(t("settings.replay.codec"))}
          value={video.codec}
          options={codecs}
          onChange={(codec) => onChange({ codec })}
        />
      </SettingRow>
      <SettingRow label={t("settings.replay.quality")}>
        <span className="flex items-center gap-2">
          <Segmented<VideoQuality>
            label={named(t("settings.replay.quality"))}
            value={video.quality}
            options={QUALITIES.map((value) => ({ value, label: t(`settings.replay.qualities.${value}`) }))}
            onChange={(quality) => onChange({ quality })}
          />
          {video.quality === "custom" && (
            <NumberField
              value={Math.round(video.bitrate_kbps / 1000)}
              min={1}
              max={200}
              unit={t("units.mbpsUnit")}
              label={named(t("settings.replay.bitrate"))}
              onCommit={(mbps) => onChange({ bitrate_kbps: mbps * 1000 })}
            />
          )}
        </span>
      </SettingRow>
      <SettingRow label={t("settings.replay.fps")}>
        <span className="flex items-center gap-2">
          <Select
            label={named(t("settings.replay.fps"))}
            value={FPS.includes(video.fps) ? video.fps : 0}
            options={[
              ...FPS.map((fps) => ({ value: fps, label: t("units.fps", { value: fps }) })),
              { value: 0, label: t("settings.replay.free") },
            ]}
            onChange={(fps) => onChange({ fps: fps === 0 ? 50 : fps })}
          />
          {!FPS.includes(video.fps) && (
            <NumberField
              value={video.fps}
              min={10}
              max={240}
              unit={t("units.fpsUnit")}
              label={named(t("settings.replay.fpsFree"))}
              onCommit={(fps) => onChange({ fps })}
            />
          )}
        </span>
      </SettingRow>
      <SettingRow label={t("settings.replay.resolution")}>
        <span className="flex items-center gap-2">
          <Select
            label={named(t("settings.replay.resolution"))}
            value={resolutionKey(video.resolution)}
            options={[
              { value: "native", label: t("settings.replay.native") },
              ...heights.map((height) => ({ value: String(height), label: t("units.height", { value: height }) })),
              { value: "free", label: t("settings.replay.free") },
            ]}
            onChange={(key) =>
              onChange({
                resolution:
                  key === "native"
                    ? { kind: "native" }
                    : { kind: "height", height: key === "free" ? Math.min(900, sourceHeight) : Number(key) },
              })
            }
          />
          {resolutionKey(video.resolution) === "free" && video.resolution.kind === "height" && (
            <NumberField
              value={video.resolution.height}
              min={Math.min(360, sourceHeight)}
              max={sourceHeight}
              unit={t("units.heightUnit")}
              label={named(t("settings.replay.heightFree"))}
              onCommit={(height) => onChange({ resolution: { kind: "height", height } })}
            />
          )}
        </span>
      </SettingRow>
    </>
  );
}

function technical(estimate: ReplayEstimate, fps: number): string {
  return [
    `${estimate.width}×${estimate.height}`,
    t("units.fps", { value: fps }),
    t("units.mbps", { value: number(estimate.video_kbps / 1000, 1) }),
    t("settings.replay.audioTracks", { count: estimate.audio_tracks }),
  ].join(" · ");
}

let handledFocus = 0;
const ARRIVAL_FLASH_MS = 80;

export function ReplaySection({ focusKey = 0 }: { focusKey?: number }) {
  const t = useT();
  const { config, loadError } = useConfigDraft("settings.sections.replay");
  const [encoders, setEncoders] = useState<EncoderInfo[]>([]);
  const [memory, setMemory] = useState<MemoryStatus | null>(null);
  const estimateRef = useRef<HTMLElement>(null);
  const [arrived, setArrived] = useState(false);
  const beforeCpu = useRef<Pick<EngineConfig["video"], "fps" | "resolution" | "codec"> | null>(null);
  const draft = config?.draft ?? null;
  const recordDraft = useMemo(() => (draft ? recordingConfig(draft) : null), [draft]);
  const estimate = useEstimate(draft);
  const recordEstimate = useEstimate(recordDraft);
  const loaded = config !== null;

  useEffect(() => {
    listEncoders().then(setEncoders, () => setEncoders([]));
    memoryStatus().then(setMemory, () => setMemory(null));
  }, []);

  useEffect(() => {
    if (focusKey <= handledFocus || !loaded) return;
    handledFocus = focusKey;
    estimateRef.current?.scrollIntoView({ block: "start" });
    estimateRef.current?.focus({ preventScroll: true });
    setArrived(true);
    window.setTimeout(() => setArrived(false), ARRIVAL_FLASH_MS);
  }, [focusKey, loaded]);

  if (!config || !draft) return <LoadError message={loadError} />;
  const change = config.change;

  const separate = draft.record.video !== null;
  const recordFields = recordVideo(draft);
  const video = (patch: Partial<EngineConfig["video"]>) => change({ ...draft, video: { ...draft.video, ...patch } });
  const replay = (patch: Partial<EngineConfig["replay"]>) => change({ ...draft, replay: { ...draft.replay, ...patch } });
  const record = (patch: Partial<VideoFields>) =>
    change({ ...draft, record: { ...draft.record, video: { ...recordVideoOf(recordFields), ...patch } } });

  const works = (hardware: boolean) => encoders.some((encoder) => encoder.hardware === hardware && !encoder.failure);
  const known = encoders.length > 0;
  const onGpu = encodesOnGpu(encoders, draft.video.encoder);
  const onCpu = known && !onGpu;
  const available = (codec: VideoCodec) =>
    encoders.some((encoder) => encoder.codec === codec && encoder.hardware === onGpu && !encoder.failure);
  const sourceHeight = estimate?.source_height ?? 4320;
  const chooseEncoder = (encoder: EncoderChoice) => {
    if (!encodesOnGpu(encoders, encoder)) {
      if (!onCpu) beforeCpu.current = { fps: draft.video.fps, resolution: draft.video.resolution, codec: draft.video.codec };
      change({
        ...draft,
        video: { ...cpuVideo(draft.video, sourceHeight, encoders), encoder },
        record: {
          ...draft.record,
          video: separate ? recordVideoOf(cpuVideo(recordFields, sourceHeight, encoders)) : null,
        },
      });
      return;
    }
    const restored = onCpu ? beforeCpu.current : null;
    beforeCpu.current = null;
    video({ ...(restored ?? {}), encoder });
  };
  const encoderHelp = !known
    ? undefined
    : onGpu
      ? draft.video.encoder === "auto"
        ? t("settings.replay.encoderGpu")
        : undefined
      : (
          <span className="text-warn">
            {t(works(true) ? "settings.replay.encoderCpu" : "settings.replay.encoderCpuNoGpu")}
          </span>
        );
  const cpuFallback = CODECS.find(
    (codec) => codec !== "hevc" && encoders.some((encoder) => encoder.codec === codec && !encoder.hardware && !encoder.failure),
  );
  const unavailableCodecs = CODECS.filter((codec) => known && !available(codec)).map((codec) => t(`codecs.${codec}`));
  const codecHelp = (codec: VideoCodec) =>
    onCpu && cpuFallback && !available(codec)
      ? t("settings.replay.codecFallbackCpu", { codec: t(`codecs.${codec}`), fallback: t(`codecs.${cpuFallback}`) })
      : unavailableCodecs.length > 0
        ? t(onCpu ? "settings.replay.codecUnavailableCpu" : "settings.replay.codecUnavailable", {
            codecs: unavailableCodecs.join(", "),
          })
        : undefined;
  const codecOptions = CODECS.map((codec) => ({
    value: codec,
    label: t(`codecs.${codec}`),
    disabled: known && !available(codec),
  }));

  const encoderBytes = estimate && onCpu ? cpuEncoderBytes(draft.video.codec, estimate.width, estimate.height, draft.video.fps) : 0;
  const ramShare = estimate && memory ? (estimate.bytes + encoderBytes) / memory.available_bytes : 0;
  const tooMuchRam = draft.replay.storage === "ram" && ramShare > RAM_SHARE_LIMIT;
  const perMinute = estimate?.file_bytes_per_minute ?? 0;
  const clipBytes = (seconds: number) => (perMinute * seconds) / 60;
  const recordPerMinute = recordEstimate?.file_bytes_per_minute ?? 0;

  return (
    <>
      <section
        ref={estimateRef}
        id="replay-quality"
        tabIndex={-1}
        data-arrived={arrived}
        aria-label={t("settings.replay.estimate")}
        className="quality-estimate scroll-mt-4 border-b border-line px-5 py-4"
      >
        <p className="flex flex-wrap items-baseline gap-x-2">
          <span className="score text-[28px] leading-none font-semibold">
            {estimate ? t("settings.replay.approx", { size: bytes(clipBytes(draft.replay.duration_secs)) }) : "—"}
          </span>
          <span className="text-ink-2">
            {t("settings.replay.perClip", { duration: durationLabel(draft.replay.duration_secs) })}
          </span>
          {estimate && (
            <span className="text-ink-2">
              {"· "}
              <span className="score text-[17px] font-medium text-ink">{t("settings.replay.approx", { size: bytes(perMinute) })}</span>{" "}
              {t("settings.replay.perMinute")}
            </span>
          )}
        </p>
        {estimate && (
          <>
            <p className="score mt-1.5 text-[15px] text-ink-2">
              {technical(estimate, draft.video.fps)}
              {encoderBytes > 0 && <> · {t("settings.replay.encoderMemory", { size: bytes(encoderBytes) })}</>}
            </p>
            {draft.replay.after_secs > 0 && (
              <p className="mt-1 text-ink-2">
                {t("settings.replay.withAfter", { after: durationLabel(draft.replay.after_secs) })}{" "}
                <span className="score text-[17px] font-medium text-ink">
                  {t("settings.replay.approx", { size: bytes(clipBytes(draft.replay.duration_secs + draft.replay.after_secs)) })}
                </span>
              </p>
            )}
            <p className="mt-1 text-ink-2">
              {t("settings.replay.bufferSize")}{" "}
              <span className="score text-[17px] font-medium text-ink">{bytes(estimate.bytes)}</span>{" "}
              {draft.replay.storage === "ram" ? t("settings.replay.inRam") : t("settings.replay.onDisk")}
            </p>
          </>
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

      <section aria-labelledby="replay-part">
        <PartTitle id="replay-part">{t("settings.replay.replayPart")}</PartTitle>
        <SettingRow label={t("settings.replay.duration")}>
          <Select
            label={t("settings.replay.duration")}
            value={draft.replay.duration_secs}
            options={withCurrent(DURATIONS, draft.replay.duration_secs).map((seconds) => ({ value: seconds, label: durationLabel(seconds) }))}
            onChange={(duration_secs) => replay({ duration_secs })}
          />
        </SettingRow>
        <SettingRow label={t("settings.replay.after")} help={t("settings.replay.afterHelp")}>
          <div className="flex items-center gap-2">
            <Select
              label={t("settings.replay.after")}
              value={draft.replay.after_secs}
              options={withCurrent(AFTER_PRESETS, draft.replay.after_secs).map((seconds) => ({
                value: seconds,
                label: seconds === 0 ? t("settings.replay.afterNone") : durationLabel(seconds),
              }))}
              onChange={(after_secs) => replay({ after_secs })}
              className="min-w-28"
            />
            <NumberField
              value={draft.replay.after_secs}
              min={0}
              max={MAX_AFTER_SECS}
              unit={t("units.secUnit")}
              label={t("settings.replay.afterFree")}
              onCommit={(after_secs) => replay({ after_secs })}
            />
          </div>
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
        <VideoRows
          video={draft.video}
          scope="replay"
          codecs={codecOptions}
          codecHelp={codecHelp(draft.video.codec)}
          sourceHeight={sourceHeight}
          onChange={video}
        />
      </section>

      <section aria-labelledby="recording-part">
        <PartTitle id="recording-part">{t("settings.replay.recordingPart")}</PartTitle>
        <p className="border-b border-line px-5 py-3 text-ink-2">
          <span className="score text-[20px] leading-none font-semibold text-ink">
            {recordEstimate ? t("settings.replay.approx", { size: bytes(recordPerMinute) }) : "—"}
          </span>{" "}
          {t("settings.replay.perMinute")}
          {recordEstimate && (
            <>
              {" "}
              ·{" "}
              <span className="score text-[17px] font-medium text-ink">
                {t("settings.replay.approx", { size: bytes(recordPerMinute * 60) })}
              </span>{" "}
              {t("settings.replay.perHour")}
            </>
          )}
          {separate && recordEstimate && (
            <span className="score mt-1 block text-[15px]">{technical(recordEstimate, recordFields.fps)}</span>
          )}
        </p>
        <SettingRow label={t("settings.replay.sameAsReplay")}>
          <Switch
            checked={!separate}
            label={t("settings.replay.sameAsReplay")}
            onChange={(same) =>
              change({ ...draft, record: { ...draft.record, video: same ? null : recordVideoOf(draft.video) } })
            }
          />
        </SettingRow>
        {separate && (
          <VideoRows
            video={recordFields}
            scope="recording"
            codecs={codecOptions}
            codecHelp={codecHelp(recordFields.codec)}
            sourceHeight={sourceHeight}
            onChange={record}
          />
        )}
      </section>

      <section aria-labelledby="common-part">
        <PartTitle id="common-part">{t("settings.replay.commonPart")}</PartTitle>
        <SettingRow label={t("settings.replay.encoder")} help={encoderHelp}>
          <Segmented<EncoderChoice>
            label={t("settings.replay.encoder")}
            value={draft.video.encoder}
            options={ENCODER_CHOICES.map((choice) => ({
              value: choice,
              label: t(`settings.replay.encoders.${choice}`),
              disabled: known && choice !== "auto" && !works(choice === "hardware"),
            }))}
            onChange={chooseEncoder}
          />
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
      </section>
      <ConfigFooter config={config} focusAfter={estimateRef} />
    </>
  );
}
