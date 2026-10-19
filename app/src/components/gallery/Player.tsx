import { convertFileSrc } from "@tauri-apps/api/core";
import { ArrowLeft, Copy, FolderOpen, Pause, Play, Star, Trash2 } from "lucide-react";
import { type KeyboardEvent, useCallback, useEffect, useRef, useState } from "react";
import { bytes, clock } from "../../lib/format.ts";
import { autoNamed, clipTitle, dayLabel, preciseTime, timeOfDay } from "../../lib/gallery.ts";
import { errorText, t as translate, useT } from "../../lib/i18n.ts";
import {
  type Clip,
  MEDIA_SCHEME,
  type TrackFile,
  type TrackKind,
  copyClip,
  preparePlayer,
  revealClip,
  setClipFavorite,
} from "../../lib/ipc.ts";
import { PlayerAudio, type TrackMix } from "../../lib/playerAudio.ts";
import { type TrackLabels, kindOfTitle, trackColor } from "../../lib/tracks.ts";
import { ExportBar } from "./ExportBar.tsx";
import { type Shortcut, Shortcuts } from "./Shortcuts.tsx";
import { MIN_RANGE_SECS, type Range, Timeline } from "./Timeline.tsx";
import { TrackLane } from "./TrackLane.tsx";

const STEP_SECS = 1;
const FULL_MIX: TrackMix = { volume: 1, mute: false, solo: false };
const BUTTON =
  "pressable inline-flex h-9 items-center gap-2 rounded-md border border-line-strong px-3 font-medium text-ink hover:bg-surface-hover";
const HEADER_LABEL = "max-[1100px]:sr-only";

function playerShortcuts(): Shortcut[] {
  return [
    { keys: [translate("keys.space")], label: translate("shortcuts.playPause") },
    { keys: ["←", "→"], label: translate("shortcuts.seek") },
    { keys: [",", "."], label: translate("shortcuts.frame") },
    { keys: ["I", "O"], label: translate("shortcuts.cutPoints") },
    { keys: [translate("keys.home"), translate("keys.end")], label: translate("shortcuts.jumpCut") },
    { keys: [translate("keys.escape")], label: translate("shortcuts.back") },
  ];
}

export function audibleTracks(mix: TrackMix[]): boolean[] {
  const solo = mix.some((track) => track.solo);
  return mix.map((track) => !track.mute && track.volume > 0 && (!solo || track.solo));
}

export function Player({
  clip,
  labels,
  appOrder,
  onBack,
  onTrash,
  onChange,
}: {
  clip: Clip;
  labels: TrackLabels | null;
  appOrder: string[];
  onBack: () => void;
  onTrash: (clip: Clip) => void;
  onChange: (clip: Clip) => void;
}) {
  const t = useT();
  const root = useRef<HTMLDivElement>(null);
  const heading = useRef<HTMLHeadingElement>(null);
  const video = useRef<HTMLVideoElement>(null);
  const audio = useRef<PlayerAudio | null>(null);
  const timeText = useRef<HTMLSpanElement>(null);
  const position = useRef<HTMLDivElement>(null);
  const [duration, setDuration] = useState(clip.duration_secs);
  const [playing, setPlaying] = useState(false);
  const [range, setRange] = useState<Range>({ start: 0, end: clip.duration_secs });
  const [tracks, setTracks] = useState<TrackFile[] | null>(null);
  const [tracksFailed, setTracksFailed] = useState<string | null>(null);
  const [mix, setMix] = useState<TrackMix[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const fps = clip.fps;
  const frame = 1 / (fps > 0 ? fps : 60);

  const prepare = useCallback(() => {
    setTracksFailed(null);
    setTracks(null);
    preparePlayer(clip.path).then(
      (files) => {
        setTracks(files);
        setMix(files.map(() => FULL_MIX));
        audio.current?.load(files.map((file) => file.path));
      },
      (failure) => setTracksFailed(errorText(failure)),
    );
  }, [clip.path]);

  useEffect(() => {
    heading.current?.focus();
    const element = video.current;
    if (!element) return;
    const player = new PlayerAudio(element);
    audio.current = player;
    prepare();
    return () => {
      player.dispose();
      audio.current = null;
    };
  }, [prepare]);

  useEffect(() => {
    audio.current?.mix(mix);
  }, [mix]);

  useEffect(() => {
    if (!notice) return;
    const timer = window.setTimeout(() => setNotice(null), 6_000);
    return () => window.clearTimeout(timer);
  }, [notice]);

  useEffect(() => {
    const element = video.current;
    const container = root.current;
    if (!element || !container) return;
    let frameRequest = 0;
    const paint = () => {
      const span = element.duration || duration || 1;
      const now = element.currentTime;
      const played = `${Math.min(1, now / span) * 100}%`;
      for (const head of container.querySelectorAll<HTMLElement>("[data-playhead]")) head.style.left = played;
      for (const bar of container.querySelectorAll<HTMLElement>("[data-played]")) bar.style.width = played;
      const shown = element.paused ? preciseTime(now, fps) : clock(now);
      if (timeText.current) timeText.current.textContent = shown;
      position.current?.setAttribute("aria-valuenow", String(Math.round(now)));
      position.current?.setAttribute("aria-valuetext", preciseTime(now, fps));
    };
    const loop = () => {
      paint();
      frameRequest = requestAnimationFrame(loop);
    };
    const start = () => {
      setPlaying(true);
      cancelAnimationFrame(frameRequest);
      frameRequest = requestAnimationFrame(loop);
    };
    const stop = () => {
      setPlaying(false);
      cancelAnimationFrame(frameRequest);
      paint();
    };
    const metadata = () => {
      if (Number.isFinite(element.duration)) {
        setDuration(element.duration);
        setRange((current) => (current.end > element.duration ? { ...current, end: element.duration } : current));
      }
      paint();
    };
    element.addEventListener("play", start);
    element.addEventListener("pause", stop);
    element.addEventListener("ended", stop);
    element.addEventListener("seeked", paint);
    element.addEventListener("loadedmetadata", metadata);
    paint();
    return () => {
      cancelAnimationFrame(frameRequest);
      element.removeEventListener("play", start);
      element.removeEventListener("pause", stop);
      element.removeEventListener("ended", stop);
      element.removeEventListener("seeked", paint);
      element.removeEventListener("loadedmetadata", metadata);
    };
  }, [duration, fps, tracks]);

  const seek = (seconds: number) => {
    const element = video.current;
    if (!element) return;
    element.currentTime = Math.min(Math.max(0, seconds), element.duration || duration);
  };

  const toggle = () => {
    const element = video.current;
    if (!element) return;
    if (element.paused) element.play().catch((failure) => setError(errorText(failure)));
    else element.pause();
  };

  const keys = (event: KeyboardEvent<HTMLDivElement>) => {
    const target = event.target as HTMLElement;
    if (event.ctrlKey || event.altKey || event.metaKey) return;
    if (target.closest("input, select, textarea, .cut-handle, [role='menu'], [role='dialog']")) return;
    const now = video.current?.currentTime ?? 0;
    const key = event.key.toLowerCase();
    const handled = (() => {
      if (event.key === " " && !target.closest("button")) toggle();
      else if (event.key === "ArrowLeft") seek(now - STEP_SECS);
      else if (event.key === "ArrowRight") seek(now + STEP_SECS);
      else if (event.key === ",") seek(now - frame);
      else if (event.key === ".") seek(now + frame);
      else if (event.key === "Home") seek(range.start);
      else if (event.key === "End") seek(range.end);
      else if (key === "i") setRange((current) => ({ ...current, start: Math.min(now, current.end - MIN_RANGE_SECS) }));
      else if (key === "o") setRange((current) => ({ ...current, end: Math.max(now, current.start + MIN_RANGE_SECS) }));
      else if (event.key === "Escape") onBack();
      else return false;
      return true;
    })();
    if (handled) event.preventDefault();
  };

  const act = (action: "favorite" | "copy" | "reveal") => {
    setError(null);
    setNotice(null);
    const fail = (failure: unknown) => setError(errorText(failure));
    if (action === "favorite") setClipFavorite(clip.path, !clip.favorite).then(() => onChange({ ...clip, favorite: !clip.favorite }), fail);
    else if (action === "copy") copyClip(clip.path).then(() => setNotice(t("gallery.copied")), fail);
    else revealClip(clip.path).catch(fail);
  };

  const titles = (tracks ?? []).map((track, index) => track.title || t("player.untitled", { number: index + 1 }));
  const kinds: TrackKind[] = titles.map((title) => kindOfTitle(title, labels));
  const colors = titles.map((title, index) => trackColor(kinds[index] ?? "application", title, appOrder));
  const audible = audibleTracks(mix);

  return (
    <div ref={root} onKeyDown={keys} className="player flex min-h-0 flex-1 flex-col">
      <header className="flex items-center gap-x-4 border-b border-line px-5 py-3">
        <button type="button" onClick={onBack} className={BUTTON} aria-label={t("player.back")}>
          <ArrowLeft size={16} aria-hidden />
          {t("nav.gallery")}
        </button>
        <div className="min-w-0 flex-1">
          <h1 ref={heading} tabIndex={-1} className="truncate text-[17px] font-semibold outline-none" title={clip.name}>
            {clipTitle(clip)}
          </h1>
          <p className="flex min-w-0 items-baseline gap-1.5 text-[13px] text-ink-2">
            <span className="score shrink-0 text-[14px]">{`${dayLabel(clip.modified_ms)} ${timeOfDay(clip.modified_ms)}`}</span>
            <span aria-hidden>·</span>
            <span className="score shrink-0 text-[14px]">{bytes(clip.size_bytes)}</span>
            {!autoNamed(clip) && (
              <>
                <span aria-hidden>·</span>
                <span className="truncate">{clip.game || t("clips.fallbackName")}</span>
              </>
            )}
          </p>
        </div>
        <span className="flex shrink-0 items-center gap-2">
          <button
            type="button"
            aria-pressed={clip.favorite}
            onClick={() => act("favorite")}
            title={t("gallery.favorites")}
            className={BUTTON}
          >
            <Star size={16} fill={clip.favorite ? "currentColor" : "none"} aria-hidden />
            <span className={HEADER_LABEL}>{t("gallery.favorites")}</span>
          </button>
          <button type="button" onClick={() => act("copy")} title={t("gallery.copy")} className={BUTTON}>
            <Copy size={16} aria-hidden />
            <span className={HEADER_LABEL}>{t("gallery.copy")}</span>
          </button>
          <button type="button" onClick={() => act("reveal")} title={t("gallery.reveal")} className={BUTTON}>
            <FolderOpen size={16} aria-hidden />
            <span className={HEADER_LABEL}>{t("gallery.reveal")}</span>
          </button>
          <Shortcuts shortcuts={playerShortcuts()} />
          <span aria-hidden className="mx-1 h-6 w-px bg-line-strong" />
          <button
            type="button"
            onClick={() => onTrash(clip)}
            aria-label={t("gallery.trash")}
            title={t("gallery.trash")}
            className="pressable flex size-9 items-center justify-center rounded-md text-ink-3 hover:bg-surface-hover hover:text-ink"
          >
            <Trash2 size={17} aria-hidden />
          </button>
        </span>
      </header>
      {error && (
        <p role="alert" className="border-b border-line bg-surface-2 px-5 py-3 text-[13px] text-warn">
          {error}
        </p>
      )}
      <p role="status" className={notice ? "border-b border-line bg-surface-2 px-5 py-3 text-[13px] text-ink" : "sr-only"}>
        {notice}
      </p>
      <div className="flex justify-center bg-meter-ground">
        <video
          ref={video}
          src={convertFileSrc(clip.path, MEDIA_SCHEME)}
          muted
          preload="metadata"
          playsInline
          onClick={toggle}
          className="aspect-video max-h-[min(40vh,calc(100vh-380px))] min-h-28 w-full max-w-full bg-black object-contain"
        />
      </div>
      <div className="flex items-center gap-4 border-b border-line px-5 py-2">
        <button
          type="button"
          onClick={toggle}
          aria-label={playing ? t("player.pause") : t("player.play")}
          aria-keyshortcuts="Space"
          className="pressable flex size-10 shrink-0 items-center justify-center rounded-md bg-ink text-ground hover:bg-ink/90"
        >
          {playing ? <Pause size={18} fill="currentColor" aria-hidden /> : <Play size={18} fill="currentColor" aria-hidden />}
        </button>
        <span className="score w-28 shrink-0 text-[20px] font-semibold">
          <span ref={timeText}>{preciseTime(0, fps)}</span>
        </span>
        <Timeline duration={duration} fps={fps} range={range} position={position} onSeek={seek} onRange={setRange} />
        <span className="score w-16 shrink-0 text-right text-[17px] font-medium text-ink-2">{clock(duration)}</span>
      </div>
      <div className="min-h-0 flex-1 overflow-y-auto">
        {tracksFailed ? (
          <div role="alert" className="flex flex-wrap items-center gap-3 border-b border-line px-5 py-3">
            <p className="flex-1 text-[13px] text-warn">{tracksFailed}</p>
            <button type="button" onClick={prepare} className={BUTTON}>
              {t("player.retry")}
            </button>
          </div>
        ) : tracks === null ? (
          <p role="status" className="border-b border-line px-5 py-4 text-[14px] text-ink-2">
            {t("player.preparing")}
          </p>
        ) : tracks.length === 0 ? (
          <p className="border-b border-line px-5 py-4 text-[14px] text-ink-2">{t("player.noAudio")}</p>
        ) : (
          <ul aria-label={t("player.tracks")}>
            {tracks.map((track, index) => (
              <TrackLane
                key={track.path}
                index={index}
                title={titles[index] ?? ""}
                path={track.path}
                color={colors[index] ?? "var(--ink)"}
                duration={duration}
                range={range}
                mix={mix[index] ?? FULL_MIX}
                audible={audible[index] ?? true}
                onMix={(next) => setMix((current) => current.map((item, place) => (place === index ? next : item)))}
              />
            ))}
          </ul>
        )}
      </div>
      <ExportBar
        clip={clip}
        tracks={tracks}
        titles={titles}
        kinds={kinds}
        colors={colors}
        audible={audible}
        volumes={mix.map((track) => track.volume)}
        range={range}
      />
    </div>
  );
}
