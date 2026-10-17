import { useEffect, useRef, useState } from "react";
import { bytes } from "../../lib/format.ts";
import {
  type EngineConfig,
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
import { Segmented, Select, SettingRow } from "./Settings.tsx";
import { ConfigFooter, LoadError, useConfigDraft } from "./configDraft.tsx";

const RAM_SHARE_LIMIT = 0.4;

const DURATIONS = [5, 10, 15, 30, 45, 60, 90, 120, 180, 300, 600, 900, 1200, 1800, 2700, 3600];
const FPS = [30, 60, 120, 144, 165, 240];
const HEIGHTS = [720, 1080, 1440, 2160];
const KEYFRAMES = [0.5, 1, 2, 4];

const CODECS: { value: VideoCodec; label: string }[] = [
  { value: "h264", label: "H.264" },
  { value: "hevc", label: "HEVC" },
  { value: "av1", label: "AV1" },
];

const QUALITIES: { value: VideoQuality; label: string }[] = [
  { value: "quality", label: "Qualité" },
  { value: "balanced", label: "Équilibré" },
  { value: "light", label: "Léger" },
  { value: "custom", label: "Libre" },
];

function durationLabel(seconds: number): string {
  if (seconds < 60) return `${seconds} s`;
  const minutes = seconds / 60;
  return Number.isInteger(minutes) ? `${minutes} min` : `${Math.floor(minutes)} min ${seconds % 60} s`;
}

function withCurrent(presets: number[], current: number): number[] {
  return presets.includes(current) ? presets : [...presets, current].sort((a, b) => a - b);
}

function resolutionKey(resolution: OutputResolution): string {
  if (resolution.kind === "native") return "native";
  return HEIGHTS.includes(resolution.height) ? String(resolution.height) : "free";
}

function NumberField({
  value,
  min,
  max,
  unit,
  label,
  onCommit,
}: {
  value: number;
  min: number;
  max: number;
  unit: string;
  label: string;
  onCommit: (value: number) => void;
}) {
  const [text, setText] = useState(String(value));
  useEffect(() => setText(String(value)), [value]);
  const commit = () => {
    const parsed = Math.round(Number(text));
    const next = Number.isFinite(parsed) && text.trim() !== "" ? Math.min(max, Math.max(min, parsed)) : value;
    setText(String(next));
    if (next !== value) onCommit(next);
  };
  return (
    <label className="flex items-center gap-1.5 text-[13px] text-ink-2">
      <input
        type="number"
        min={min}
        max={max}
        value={text}
        onChange={(event) => setText(event.target.value)}
        onBlur={commit}
        onKeyDown={(event) => event.key === "Enter" && commit()}
        className="score h-9 w-20 rounded-md border border-line-strong bg-surface px-2 text-right text-[15px] text-ink"
        aria-label={label}
      />
      {unit}
    </label>
  );
}

export function ReplaySection() {
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
  const unavailableCodecs = CODECS.filter(
    (codec) => encoders.length > 0 && !available(codec.value),
  ).map((codec) => codec.label);
  const ramShare = estimate && memory ? estimate.bytes / memory.available_bytes : 0;
  const tooMuchRam = draft.replay.storage === "ram" && ramShare > RAM_SHARE_LIMIT;
  const sourceHeight = estimate?.source_height ?? 4320;
  const currentHeight = draft.video.resolution.kind === "height" ? draft.video.resolution.height : null;
  const heights = HEIGHTS.filter((height) => height < sourceHeight || height === currentHeight);

  return (
    <>
      <section ref={estimateRef} tabIndex={-1} aria-label="Estimation du tampon" className="border-b border-line px-5 py-4">
        <p className="flex flex-wrap items-baseline gap-x-2">
          <span className="score text-[28px] leading-none font-semibold">
            {estimate ? `≈ ${bytes(estimate.bytes)}` : "—"}
          </span>
          <span className="text-ink-2">{draft.replay.storage === "ram" ? "en RAM" : "sur le disque"}</span>
        </p>
        {estimate && (
          <p className="score mt-1 text-[15px] text-ink-2">
            {estimate.width}×{estimate.height} · {draft.video.fps} i/s · {(estimate.video_kbps / 1000).toLocaleString("fr-FR", { maximumFractionDigits: 1 })} Mb/s ·{" "}
            {estimate.audio_tracks} {estimate.audio_tracks > 1 ? "pistes audio" : "piste audio"}
          </p>
        )}
        {tooMuchRam && memory && (
          <div role="alert" className="mt-3 flex flex-wrap items-center gap-3">
            <p className="flex-1 text-[13px] text-warn">
              Ce tampon prendrait {Math.round(ramShare * 100)} % de la RAM libre ({bytes(memory.available_bytes)}).
            </p>
            <button
              type="button"
              onClick={() => replay({ storage: "disk" })}
              className="pressable h-9 rounded-md border border-line-strong px-3.5 font-medium text-ink hover:bg-surface-hover"
            >
              Passer le tampon sur le disque
            </button>
          </div>
        )}
      </section>

      <SettingRow label="Durée du replay">
        <Select
          label="Durée du replay"
          value={draft.replay.duration_secs}
          options={withCurrent(DURATIONS, draft.replay.duration_secs).map((seconds) => ({ value: seconds, label: durationLabel(seconds) }))}
          onChange={(duration_secs) => replay({ duration_secs })}
        />
      </SettingRow>
      <SettingRow label="Tampon" help="Sur le disque pour les longues durées, sans occuper la RAM.">
        <Segmented<ReplayStorage>
          label="Tampon"
          value={draft.replay.storage}
          options={[
            { value: "ram", label: "RAM" },
            { value: "disk", label: "Disque" },
          ]}
          onChange={(storage) => replay({ storage })}
        />
      </SettingRow>
      <SettingRow
        label="Codec"
        help={unavailableCodecs.length > 0 ? `Indisponible sur cette carte graphique : ${unavailableCodecs.join(", ")}.` : undefined}
      >
        <Segmented<VideoCodec>
          label="Codec"
          value={draft.video.codec}
          options={CODECS.map((codec) => ({ ...codec, disabled: encoders.length > 0 && !available(codec.value) }))}
          onChange={(codec) => video({ codec })}
        />
      </SettingRow>
      <SettingRow label="Qualité">
        <span className="flex items-center gap-2">
          <Segmented<VideoQuality>
            label="Qualité"
            value={draft.video.quality}
            options={QUALITIES}
            onChange={(quality) => video({ quality })}
          />
          {draft.video.quality === "custom" && (
            <NumberField
              value={Math.round(draft.video.bitrate_kbps / 1000)}
              min={1}
              max={200}
              unit="Mb/s"
              label="Débit en mégabits par seconde"
              onCommit={(mbps) => video({ bitrate_kbps: mbps * 1000 })}
            />
          )}
        </span>
      </SettingRow>
      <SettingRow label="Images par seconde">
        <span className="flex items-center gap-2">
          <Select
            label="Images par seconde"
            value={FPS.includes(draft.video.fps) ? draft.video.fps : 0}
            options={[...FPS.map((fps) => ({ value: fps, label: `${fps} i/s` })), { value: 0, label: "Libre" }]}
            onChange={(fps) => video({ fps: fps === 0 ? 50 : fps })}
          />
          {!FPS.includes(draft.video.fps) && (
            <NumberField
              value={draft.video.fps}
              min={10}
              max={240}
              unit="i/s"
              label="Images par seconde, valeur libre"
              onCommit={(fps) => video({ fps })}
            />
          )}
        </span>
      </SettingRow>
      <SettingRow label="Résolution de sortie">
        <span className="flex items-center gap-2">
          <Select
            label="Résolution de sortie"
            value={resolutionKey(draft.video.resolution)}
            options={[
              { value: "native", label: "Native" },
              ...heights.map((height) => ({ value: String(height), label: `${height}p` })),
              { value: "free", label: "Libre" },
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
              unit="p"
              label="Hauteur de sortie, valeur libre"
              onCommit={(height) => video({ resolution: { kind: "height", height } })}
            />
          )}
        </span>
      </SettingRow>
      <SettingRow label="Intervalle d'images clés" help="Plus court : le clip commence au plus près de l'instant voulu.">
        <Select
          label="Intervalle d'images clés"
          value={draft.video.keyframe_interval_secs}
          options={withCurrent(KEYFRAMES, draft.video.keyframe_interval_secs).map((seconds) => ({ value: seconds, label: `${String(seconds).replace(".", ",")} s` }))}
          onChange={(keyframe_interval_secs) => video({ keyframe_interval_secs })}
        />
      </SettingRow>
      <ConfigFooter config={config} focusAfter={estimateRef} />
    </>
  );
}
