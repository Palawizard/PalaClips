import { convertFileSrc } from "@tauri-apps/api/core";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { Check, Copy, Film, FolderOpen, RotateCcw, Upload, X } from "lucide-react";
import { memo, useEffect, useRef, useState } from "react";
import {
  type Draft,
  type Preset,
  PRESET_MB,
  addDrafts,
  clearDrafts,
  editPlan,
  planInput,
  removeDraft,
  setOptions,
  targetBytes,
  thumbnailOf,
  useCompressDrafts,
} from "../../lib/compressDrafts.ts";
import { bytes, clock } from "../../lib/format.ts";
import { errorText, message, number, useT } from "../../lib/i18n.ts";
import {
  type CompressCodec,
  type CompressJob,
  type CompressQuality,
  compressAdd,
  compressCancel,
  compressClear,
  compressCopy,
  compressJobs,
  compressPickFiles,
  compressRemove,
  compressRetry,
  compressReveal,
  onCompressChanged,
} from "../../lib/ipc.ts";
import { NumberField, Segmented, Select, SettingRow, Switch } from "../settings/Settings.tsx";

const SECONDARY =
  "pressable inline-flex h-9 items-center gap-2 rounded-md border border-line-strong px-3.5 font-medium text-ink hover:bg-surface-hover";
const ICON_BUTTON = "pressable -m-1 rounded-md p-1.5 text-ink-3 hover:bg-surface-hover hover:text-ink";
const PRESETS: Preset[] = ["discord", "nitroBasic", "nitro", "other"];
const HEIGHTS = [2160, 1440, 1080, 720, 480];
const MAX_FPS = 60;
const LOW_FPS = 30;
const MIN_KBPS = 100;
const MAX_KBPS = 200_000;
const MIN_FREE_MB = 1;
const TARGET_MB = 1_000_000;
const VIDEO_EXTENSIONS = ["mp4", "mkv", "mov", "webm", "avi", "m4v", "wmv", "flv", "ts"];
const MAX_FREE_MB = 10_000;
const QUALITY_TONE: Record<CompressQuality, string> = { good: "text-ink", fair: "text-ink-2", low: "text-warn" };


function isVideo(path: string): boolean {
  return VIDEO_EXTENSIONS.includes(path.split(".").pop()?.toLowerCase() ?? "");
}

function fileName(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}

function even(value: number): number {
  return Math.max(2, Math.round(value / 2) * 2);
}

function Thumbnail({ path }: { path: string | null }) {
  return (
    <span className="flex h-[54px] w-24 shrink-0 items-center justify-center overflow-hidden rounded-[4px] bg-meter-ground">
      {path ? (
        <img src={convertFileSrc(path)} alt="" className="size-full object-cover" draggable={false} />
      ) : (
        <Film size={20} className="text-ink-3" aria-hidden />
      )}
    </span>
  );
}

const DraftRow = memo(function DraftRow({ draft, target }: { draft: Draft; target: number }) {
  const t = useT();
  const source = draft.source;
  const view = draft.view;
  const heights = source
    ? [...new Set([source.height, ...HEIGHTS].filter((height) => height <= source.height))].sort((a, b) => b - a)
    : [];
  const sourceFps = source ? Math.min(Math.round(source.fps), MAX_FPS) : MAX_FPS;
  const rates = [...new Set([sourceFps, LOW_FPS].filter((fps) => fps <= sourceFps))];
  const plan = view?.plan;
  const range = draft.endSecs !== null || draft.startSecs > 0;
  return (
    <li className="compress-row flex flex-wrap items-center gap-x-4 gap-y-2 border-b border-line px-5 py-3">
      <Thumbnail path={draft.thumbnail} />
      <div className="min-w-0 flex-1 basis-60">
        <p className="truncate text-[15px] font-semibold" title={draft.path}>
          {draft.name}
        </p>
        {source ? (
          <p className="truncate text-[13px] text-ink-2">
            {t("compress.source", {
              width: source.width,
              height: source.height,
              fps: t("units.fps", { value: number(source.fps) }),
              duration: clock(source.duration_secs),
              size: bytes(source.size_bytes),
            })}
            {source.hdr && <span className="ml-2 rounded-[3px] border border-line-strong px-1 text-[12px]">{t("compress.hdr")}</span>}
            {range && (
              <span className="ml-2">
                {t("compress.range", { start: clock(draft.startSecs), end: clock(draft.endSecs ?? source.duration_secs) })}
              </span>
            )}
          </p>
        ) : (
          !draft.error && <p className="text-[13px] text-ink-3">{t("compress.probing")}</p>
        )}
        {draft.error && <p className="text-[13px] text-warn">{draft.error}</p>}
      </div>
      {source && plan && plan.copy_bytes !== null && (
        <p className="score text-right text-[15px]">
          <span className="mr-2 font-sans text-[13px] text-ink-2">{t("compress.copyPlan")}</span>
          {t("compress.estimate", { size: bytes(plan.copy_bytes, 2) })}
        </p>
      )}
      {source && plan && plan.copy_bytes === null && (
        <div className="flex flex-wrap items-center gap-2">
          <Select
            label={t("compress.resolution")}
            value={plan.height}
            options={heights.map((height) => ({ value: height, label: t("units.height", { value: height }) }))}
            onChange={(height) =>
              editPlan(draft.key, { ...plan, height, width: even((height * source.width) / source.height) })
            }
            className="min-w-24"
          />
          <Select
            label={t("compress.fps")}
            value={plan.fps}
            options={rates.map((fps) => ({ value: fps, label: t("units.fps", { value: fps }) }))}
            onChange={(fps) => editPlan(draft.key, { ...plan, fps })}
            className="min-w-24"
          />
          <NumberField
            value={plan.video_kbps}
            min={MIN_KBPS}
            max={MAX_KBPS}
            unit={t("compress.kbps")}
            label={t("compress.bitrate")}
            onCommit={(video_kbps) => editPlan(draft.key, { ...plan, video_kbps })}
          />
          <p className="score w-40 text-right text-[15px]">
            {view.estimated_bytes > target ? (
              <span className="text-warn">{t("compress.overTarget", { size: t("units.mb", { value: number(target / TARGET_MB) }) })}</span>
            ) : (
              <>
                {t("compress.estimate", { size: bytes(view.estimated_bytes, 2) })}
                <span className={`ml-2 text-[13px] font-medium ${QUALITY_TONE[view.quality]}`}>
                  <span className="sr-only">{t("compress.qualityLabel", { quality: "" })}</span>
                  {t(`compress.quality.${view.quality}`)}
                </span>
              </>
            )}
          </p>
        </div>
      )}
      <button
        type="button"
        onClick={() => removeDraft(draft.key)}
        aria-label={t("compress.remove", { name: draft.name })}
        className={`${ICON_BUTTON} ml-auto`}
      >
        <X size={16} aria-hidden />
      </button>
    </li>
  );
});

const JobRow = memo(function JobRow({
  job,
  onNotice,
  onError,
  onRemoved,
}: {
  job: CompressJob;
  onNotice: (text: string) => void;
  onError: (text: string) => void;
  onRemoved: (id: number) => void;
}) {
  const t = useT();
  const name = fileName(job.source);
  const fail = (failure: unknown) => onError(errorText(failure));
  return (
    <li className="compress-row flex flex-wrap items-center gap-x-4 gap-y-2 border-b border-line px-5 py-3">
      <Thumbnail path={thumbnailOf(job.source)} />
      <div className="min-w-0 flex-1 basis-60">
        <p className="truncate text-[15px] font-semibold" title={job.source}>
          {name}
        </p>
        <p className="text-[13px] text-ink-2">
          {t("compress.jobPlan", {
            size: t("units.mb", { value: number(job.target_bytes / TARGET_MB) }),
            height: t("units.height", { value: job.plan.height }),
            fps: t("units.fps", { value: job.plan.fps }),
          })}
        </p>
      </div>
      <div className="ml-auto flex flex-wrap items-center justify-end gap-3">
        {job.state === "queued" && (
          <span className="flex items-center gap-3">
            <span className="text-[13px] text-ink-2">{t("compress.waiting")}</span>
            <button type="button" onClick={() => compressCancel(job.id).catch(fail)} className={SECONDARY}>
              {t("compress.cancel")}
            </button>
          </span>
        )}
        {job.state === "running" && (
          <span className="flex items-center gap-3">
            {job.attempt > 1 && <span className="text-[13px] text-ink-2">{t("compress.attempt", { attempt: job.attempt })}</span>}
            <span
              role="progressbar"
              aria-label={name}
              aria-valuetext={t("compress.progress", {
                percent: t("units.percent", { value: number(job.fraction * 100) }),
                attempt: job.attempt,
              })}
              aria-valuemin={0}
              aria-valuemax={100}
              aria-valuenow={Math.round(job.fraction * 100)}
              className="relative h-2 w-40 overflow-hidden rounded-[2px] bg-meter-ground"
            >
              <span className="export-fill absolute inset-0 origin-left bg-ink" style={{ transform: `scaleX(${job.fraction})` }} />
            </span>
            <span className="score w-12 text-right text-[15px] text-ink-2">
              {t("units.percent", { value: number(job.fraction * 100) })}
            </span>
            <button type="button" onClick={() => compressCancel(job.id).catch(fail)} className={SECONDARY}>
              {t("compress.cancel")}
            </button>
          </span>
        )}
        {job.state === "done" && (
          <span className="flex flex-wrap items-center gap-3">
            <span className="flex items-center gap-1.5 text-[13px] text-ink-2">
              <Check size={15} strokeWidth={2.4} className="text-ink" aria-hidden />
              {t("compress.done")}
            </span>
            <span className="score text-[17px] font-semibold">{bytes(job.bytes, 2)}</span>
            <button
              type="button"
              onClick={() => compressCopy(job.id).then(() => onNotice(t("compress.copied")), fail)}
              className={SECONDARY}
            >
              <Copy size={16} aria-hidden />
              {t("compress.copy")}
            </button>
            <button type="button" onClick={() => compressReveal(job.id).catch(fail)} className={SECONDARY}>
              <FolderOpen size={16} aria-hidden />
              {t("compress.show")}
            </button>
          </span>
        )}
        {job.state === "failed" && <p className="max-w-[60ch] text-[13px] text-warn">{message(job.error)}</p>}
        {job.state === "cancelled" && <span className="text-[13px] text-ink-3">{t("compress.cancelled")}</span>}
        {(job.state === "failed" || job.state === "cancelled") && (
          <button type="button" onClick={() => compressRetry(job.id).catch(fail)} className={SECONDARY}>
            <RotateCcw size={16} aria-hidden />
            {t("compress.retry")}
          </button>
        )}
        {(job.state === "done" || job.state === "failed" || job.state === "cancelled") && (
          <button
            type="button"
            onClick={() => compressRemove(job.id).then(() => onRemoved(job.id), fail)}
            aria-label={t("compress.remove", { name })}
            className={ICON_BUTTON}
          >
            <X size={16} aria-hidden />
          </button>
        )}
      </div>
    </li>
  );
});

export function CompressPage() {
  const t = useT();
  const { options, drafts } = useCompressDrafts();
  const [jobs, setJobs] = useState<CompressJob[]>([]);
  const [dragging, setDragging] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const queueTitle = useRef<HTMLHeadingElement>(null);
  const [announce, setAnnounce] = useState("");

  useEffect(() => {
    compressJobs().then(
      (loaded) =>
        setJobs((current) => [...loaded.filter((job) => !current.some((item) => item.id === job.id)), ...current]),
      (failure) => setError(errorText(failure)),
    );
    const unlisten = onCompressChanged((job) => {
      if (job.state === "done") setAnnounce(t("compress.finished", { name: fileName(job.source), size: bytes(job.bytes, 2) }));
      setJobs((current) =>
        current.some((item) => item.id === job.id) ? current.map((item) => (item.id === job.id ? job : item)) : [...current, job],
      );
    });
    return () => {
      unlisten.then((stop) => stop()).catch(() => undefined);
    };
  }, [t]);

  useEffect(() => {
    const unlisten = getCurrentWebview().onDragDropEvent((event) => {
      const payload = event.payload;
      if (payload.type === "enter" || payload.type === "over") setDragging(true);
      else {
        setDragging(false);
        const videos = payload.type === "drop" ? payload.paths.filter(isVideo) : [];
        if (videos.length > 0) addDrafts(videos.map((path) => ({ path })));
      }
    });
    return () => {
      unlisten.then((stop) => stop()).catch(() => undefined);
    };
  }, []);

  const removed = (id: number) => setJobs((current) => current.filter((job) => job.id !== id));
  const target = targetBytes(options);
  const busy = drafts.length + jobs.length > 0;
  const ready = drafts.filter((draft) => draft.view !== null && draft.error === null);
  const finished = jobs.some((job) => job.state === "done" || job.state === "failed" || job.state === "cancelled");
  const start = () => {
    setError(null);
    setNotice(null);
    compressAdd(ready.map((draft) => ({ ...planInput(draft, options), plan: draft.view!.plan }))).then(
      () => {
        clearDrafts(ready.map((draft) => draft.key));
        queueTitle.current?.focus();
      },
      (failure) => setError(errorText(failure)),
    );
  };
  const pick = () =>
    compressPickFiles().then(
      (paths) => paths.length > 0 && addDrafts(paths.map((path) => ({ path }))),
      (failure) => setError(errorText(failure)),
    );

  return (
    <div className="flex min-h-full flex-col">
      <h1 className="border-b border-line px-5 pt-6 pb-4 text-[22px] font-semibold">{t("nav.compress")}</h1>
      <SettingRow label={t("compress.target")}>
        <div className="flex flex-wrap items-center justify-end gap-2">
          <Segmented<Preset>
            label={t("compress.target")}
            value={options.preset}
            options={PRESETS.map((preset) => ({
              value: preset,
              label:
                preset === "other"
                  ? t("compress.presets.other")
                  : t("compress.presetLabel", { size: PRESET_MB[preset], name: t(`compress.presets.${preset}`) }),
            }))}
            onChange={(preset) => setOptions({ preset })}
          />
          {options.preset === "other" && (
            <NumberField
              value={options.freeMb}
              min={MIN_FREE_MB}
              max={MAX_FREE_MB}
              unit={t("units.mbUnit")}
              label={t("compress.targetFree")}
              onCommit={(freeMb) => setOptions({ freeMb })}
            />
          )}
        </div>
      </SettingRow>
      <SettingRow label={t("compress.codec")} help={options.codec === "av1" ? t("compress.av1Warning") : undefined}>
        <Segmented<CompressCodec>
          label={t("compress.codec")}
          value={options.codec}
          options={[
            { value: "h264", label: t("codecs.h264") },
            { value: "av1", label: t("codecs.av1") },
          ]}
          onChange={(codec) => setOptions({ codec })}
        />
      </SettingRow>
      <SettingRow label={t("compress.allTracks")} help={t("compress.allTracksHelp")}>
        <Switch checked={options.allTracks} label={t("compress.allTracks")} onChange={(allTracks) => setOptions({ allTracks })} />
      </SettingRow>

      <div className="px-5 py-4">
        <div
          data-dragging={dragging}
          className={`drop-zone flex items-center gap-3 rounded-md border border-dashed border-line-strong data-[dragging=true]:border-ink data-[dragging=true]:bg-surface-2 ${
            busy ? "flex-row px-4 py-2.5" : "flex-col px-5 py-6 text-center"
          }`}
        >
          {!busy && <Upload size={22} className="text-ink-3" aria-hidden />}
          <p className={`text-[15px] font-semibold ${busy ? "flex-1" : ""}`}>
            {dragging ? t("compress.dropActive") : t("compress.drop")}
          </p>
          <button type="button" onClick={pick} className={SECONDARY}>
            {t("compress.pick")}
          </button>
          {!busy && <p className="text-[13px] text-ink-3">{t("compress.formats")}</p>}
        </div>
      </div>

      {busy && (
        <section aria-labelledby="compress-queue" className="flex-1">
          <div className="flex items-center justify-between border-b border-line px-5 pb-3">
            <h2 id="compress-queue" ref={queueTitle} tabIndex={-1} className="text-[17px] font-semibold outline-none">
              {t("compress.queue")}
            </h2>
            {finished && (
              <button
                type="button"
                onClick={() =>
                  compressClear().then(
                    () => setJobs((current) => current.filter((job) => job.state === "queued" || job.state === "running")),
                    (failure) => setError(errorText(failure)),
                  )
                }
                className="pressable -my-1 h-8 rounded-md px-2.5 text-[13px] font-medium text-ink-2 hover:bg-surface-hover hover:text-ink"
              >
                {t("compress.clear")}
              </button>
            )}
          </div>
          <ul>
            {drafts.map((draft) => (
              <DraftRow key={`draft-${draft.key}`} draft={draft} target={target} />
            ))}
            {jobs.map((job) => (
              <JobRow key={`job-${job.id}`} job={job} onNotice={setNotice} onError={setError} onRemoved={removed} />
            ))}
          </ul>
        </section>
      )}

      <p role="status" className="sr-only">
        {announce}
      </p>
      <div className="sticky bottom-0 z-10 mt-auto flex flex-wrap items-center justify-end gap-3 border-t border-line bg-surface-2 px-5 py-3">
        {error && (
          <p role="alert" className="mr-auto text-[13px] text-warn">
            {error}
          </p>
        )}
        {notice && !error && (
          <p role="status" className="mr-auto text-[13px] text-ink">
            {notice}
          </p>
        )}
        <button
          type="button"
          disabled={ready.length === 0}
          onClick={start}
          className="pressable h-9 rounded-md bg-ink px-3.5 font-semibold text-ground hover:bg-ink/90 disabled:cursor-not-allowed disabled:border disabled:border-line disabled:bg-transparent disabled:text-ink-3"
        >
          {t("compress.start", { count: ready.length })}
        </button>
      </div>
    </div>
  );
}
