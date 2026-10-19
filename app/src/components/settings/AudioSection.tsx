import { FolderOpen, Search, X } from "lucide-react";
import { type ReactNode, useEffect, useId, useRef, useState } from "react";
import { iconUrl } from "../../lib/appIcons.ts";
import { decibels } from "../../lib/format.ts";
import { errorText, localeCompare, useT } from "../../lib/i18n.ts";
import {
  type AppSession,
  type AppStatus,
  type AppTrack,
  type AudioDevice,
  type EngineConfig,
  type TargetInfo,
  type TrackKind,
  listAppSessions,
  listAudioDevices,
  listCaptureTargets,
  pickAppExe,
} from "../../lib/ipc.ts";
import { chosenName, trackColor, trackIcon, trackTitle } from "../../lib/tracks.ts";
import { MeterBar, useTrackMeter } from "../MeterBar.tsx";
import { CalibrationPanel } from "./CalibrationPanel.tsx";
import { NumberField, Select, SettingRow, Switch } from "./Settings.tsx";
import { ConfigFooter, LoadError, useConfigDraft } from "./configDraft.tsx";

const SESSION_POLL_MS = 100;
const SESSION_POLL_BACKGROUND_MS = 500;
const RECENT_SOUND_MS = 15_000;
const SOUND_FLOOR = 0.003;
const BUTTON =
  "pressable h-9 rounded-md border border-line-strong px-3 font-medium text-ink hover:bg-surface-hover disabled:cursor-not-allowed disabled:opacity-50";

type Audio = EngineConfig["audio"];

interface Row {
  key: string;
  kind: TrackKind;
  label: string;
  enabled: boolean;
  offset: number;
  exe: string | null;
}

interface Candidate {
  exe: string;
  name: string;
  processId: number;
  peak: number | null;
}

function rowsOf(audio: Audio): Row[] {
  return [
    { key: "mix", kind: "mix", label: trackTitle(audio, "mix"), enabled: audio.record_full_mix, offset: audio.full_mix_sync_offset_ms, exe: null },
    { key: "microphone", kind: "microphone", label: trackTitle(audio, "microphone"), enabled: audio.record_microphone, offset: audio.microphone_sync_offset_ms, exe: null },
    ...audio.app_tracks.map((track) => ({
      key: `app:${track.exe_name.toLowerCase()}`,
      kind: "application" as const,
      label: track.label,
      enabled: track.enabled,
      offset: track.sync_offset_ms,
      exe: track.exe_name,
    })),
    { key: "game", kind: "game", label: trackTitle(audio, "game"), enabled: audio.auto_game_track, offset: audio.game_sync_offset_ms, exe: null },
  ];
}

function rank(order: string[], exe: string): number {
  const index = order.indexOf(exe);
  return index < 0 ? order.length : index;
}

function exeStem(exe: string): string {
  return exe.replace(/\.exe$/i, "");
}

function meterPosition(peak: number): number {
  const db = decibels(peak);
  return Number.isFinite(db) ? Math.min(1, Math.max(0, (db + 60) / 60)) : 0;
}

function NameField({
  value,
  label,
  muted,
  errorId,
  isTaken,
  onCommit,
  onReject,
}: {
  value: string;
  label: string;
  muted: boolean;
  errorId: string | undefined;
  isTaken: (name: string) => boolean;
  onCommit: (name: string) => void;
  onReject: (message: string | null) => void;
}) {
  const t = useT();
  const [text, setText] = useState(value);
  useEffect(() => setText(value), [value]);
  const commit = () => {
    const next = text.trim().replace(/\s+/g, " ");
    if (next !== value && isTaken(next)) {
      setText(value);
      onReject(t("settings.audio.nameTaken", { name: next }));
      return;
    }
    onReject(null);
    if (next === "" || next === value) setText(value);
    else onCommit(next);
  };
  return (
    <input
      type="text"
      value={text}
      spellCheck={false}
      aria-label={label}
      aria-describedby={errorId}
      onChange={(event) => setText(event.target.value)}
      onBlur={commit}
      onKeyDown={(event) => {
        if (event.key === "Enter") commit();
        if (event.key === "Escape") setText(value);
      }}
      className={`h-9 w-full min-w-0 rounded-md border border-transparent bg-transparent px-2 text-[15px] font-semibold hover:border-ink-3 focus:border-ink-3 focus:bg-surface ${muted ? "text-ink-2" : "text-ink"}`}
    />
  );
}

function AppGlyph({ exe, processId, kind, color }: { exe: string | null; processId: number | null; kind: TrackKind; color: string }) {
  const [icon, setIcon] = useState<string | null>(null);
  useEffect(() => {
    if (!exe || processId === null) return;
    let current = true;
    iconUrl(exe, processId).then((url) => current && setIcon(url));
    return () => {
      current = false;
    };
  }, [exe, processId]);
  if (icon) return <img src={icon} alt="" className="size-5 shrink-0" />;
  const Icon = trackIcon(kind);
  return <Icon size={18} aria-hidden className="shrink-0" style={{ color }} />;
}

const ROW_COLUMNS = "grid grid-cols-[44px_minmax(0,1fr)_minmax(80px,150px)_10.5rem] items-center gap-x-4";

function TrackRow({
  row,
  index,
  meterIndex,
  color,
  processId,
  detail,
  isTaken,
  onRename,
  onOffset,
  action,
}: {
  row: Row;
  index: number | null;
  meterIndex: number | null;
  color: string;
  processId: number | null;
  detail: ReactNode;
  isTaken: (name: string) => boolean;
  onRename: (label: string) => void;
  onOffset: (offset: number) => void;
  action: ReactNode;
}) {
  const t = useT();
  const { fill, hold, peak } = useTrackMeter(meterIndex);
  const [nameError, setNameError] = useState<string | null>(null);
  const errorId = useId();
  return (
    <li className={`${ROW_COLUMNS} gap-y-1 border-b border-line px-5 py-3`}>
      <span
        className="score row-span-2 flex h-8 w-11 items-center justify-center rounded-[4px] text-[18px] font-semibold"
        style={index === null ? undefined : { background: color, color: "var(--ground)" }}
      >
        {index === null ? <span className="text-ink-3">—</span> : String(index + 1).padStart(2, "0")}
      </span>
      <div className="flex min-w-0 items-center gap-1">
        <AppGlyph exe={row.exe} processId={processId} kind={row.kind} color={color} />
        <NameField
          value={row.label}
          label={t("settings.audio.trackName", { name: row.label })}
          muted={!row.enabled}
          errorId={nameError ? errorId : undefined}
          isTaken={isTaken}
          onCommit={onRename}
          onReject={setNameError}
        />
      </div>
      <div className="flex min-w-0 items-center gap-2">
        <MeterBar color={color} fill={fill} hold={hold} className="h-2 flex-1" />
        <span ref={peak} aria-hidden className="score w-14 shrink-0 text-right text-[14px] whitespace-nowrap text-ink-2">
          −∞
        </span>
      </div>
      <div className="row-span-2 flex items-center justify-end gap-3">
        <NumberField
          value={row.offset}
          min={-1000}
          max={1000}
          unit={t("units.msUnit")}
          label={t("settings.audio.offsetOf", { name: row.label })}
          onCommit={onOffset}
        />
        <span className="flex w-11 justify-center">{action}</span>
      </div>
      <div
        className="col-start-2 col-end-4 flex min-w-0 flex-wrap items-center gap-2 pl-7 text-[12px] text-ink-2"
      >
        {detail}
      </div>
      {nameError && (
        <p id={errorId} role="alert" className="col-start-2 col-end-4 pl-7 text-[12px] text-warn">
          {nameError}
        </p>
      )}
    </li>
  );
}

export function AudioSection({ status }: { status: AppStatus | null }) {
  const t = useT();
  const { config, loadError } = useConfigDraft();
  const [outputs, setOutputs] = useState<AudioDevice[]>([]);
  const [inputs, setInputs] = useState<AudioDevice[]>([]);
  const [sessions, setSessions] = useState<AppSession[]>([]);
  const [windows, setWindows] = useState<TargetInfo[]>([]);
  const [visible, setVisible] = useState(document.visibilityState === "visible");
  const [focused, setFocused] = useState(document.hasFocus());
  const peaks = useRef(new Map<string, number>());
  const meters = useRef(new Map<string, HTMLSpanElement>());
  const membership = useRef("");
  const candidateOrder = useRef<string[]>([]);
  const [focusInList, setFocusInList] = useState(false);
  const [query, setQuery] = useState("");
  const [calibrating, setCalibrating] = useState(false);
  const [pickError, setPickError] = useState<string | null>(null);
  const lastSound = useRef(new Map<string, number>());
  const listRef = useRef<HTMLUListElement>(null);

  useEffect(() => {
    listAudioDevices(false).then(setOutputs, () => setOutputs([]));
    listAudioDevices(true).then(setInputs, () => setInputs([]));
    const loadWindows = () => listCaptureTargets().then(setWindows, () => setWindows([]));
    loadWindows();
    const onVisibility = () => setVisible(document.visibilityState === "visible");
    const onFocus = () => {
      setFocused(true);
      loadWindows();
    };
    const onBlur = () => setFocused(false);
    window.addEventListener("focus", onFocus);
    window.addEventListener("blur", onBlur);
    document.addEventListener("visibilitychange", onVisibility);
    return () => {
      window.removeEventListener("focus", onFocus);
      window.removeEventListener("blur", onBlur);
      document.removeEventListener("visibilitychange", onVisibility);
    };
  }, []);

  useEffect(() => {
    if (!visible) return;
    let timer = 0;
    let active = true;
    const poll = () => {
      listAppSessions()
        .then((list) => {
          if (!active) return;
          const now = Date.now();
          for (const session of list) {
            const key = session.exe_name.toLowerCase();
            if (session.peak > SOUND_FLOOR) lastSound.current.set(key, now);
            peaks.current.set(key, session.peak);
            const meter = meters.current.get(key);
            if (meter) meter.style.transform = `scaleX(${meterPosition(session.peak)})`;
          }
          const signature = list
            .map((session) => {
              const key = session.exe_name.toLowerCase();
              const sounding = now - (lastSound.current.get(key) ?? 0) < RECENT_SOUND_MS;
              return `${key}:${session.process_id}:${sounding ? 1 : 0}`;
            })
            .sort()
            .join("|");
          if (signature !== membership.current) {
            membership.current = signature;
            setSessions(list);
          }
        }, () => undefined)
        .finally(() => {
          if (active) timer = window.setTimeout(poll, focused ? SESSION_POLL_MS : SESSION_POLL_BACKGROUND_MS);
        });
    };
    poll();
    return () => {
      active = false;
      window.clearTimeout(timer);
    };
  }, [visible, focused]);

  if (!config) return <LoadError message={loadError} />;
  const { draft } = config;
  const audio = draft.audio;
  const setAudio = (patch: Partial<Audio>) => config.change({ ...draft, audio: { ...audio, ...patch } });
  const setApp = (exe: string, patch: Partial<AppTrack>) =>
    setAudio({ app_tracks: audio.app_tracks.map((track) => (track.exe_name === exe ? { ...track, ...patch } : track)) });

  const rows = rowsOf(audio);
  let next = 0;
  const indexes = rows.map((row) => (row.enabled ? next++ : null));
  const running = config.armed && !calibrating ? rowsOf(config.saved.audio).filter((row) => row.enabled).map((row) => row.key) : [];
  const meterIndexOf = (key: string) => (running.includes(key) ? running.indexOf(key) : null);
  const isTakenBy = (row: Row) => (name: string) =>
    rows.some((other) => other.key !== row.key && other.label.toLowerCase() === name.toLowerCase());
  const appOrder = audio.app_tracks.filter((track) => track.enabled).map((track) => track.label);
  const runningPid = new Map<string, number>();
  for (const target of windows) if (target.kind === "window") runningPid.set(target.exe_name.toLowerCase(), target.process_id);
  for (const session of sessions) runningPid.set(session.exe_name.toLowerCase(), session.process_id);
  const gameIndex = meterIndexOf("game");
  const gameSource = gameIndex === null ? null : (status?.tracks[gameIndex]?.source ?? null);

  const deviceOptions = (devices: AudioDevice[]) => {
    const fallback = devices.find((device) => device.default);
    return [
      {
        value: "",
        label: fallback
          ? t("settings.audio.defaultDeviceNamed", { name: fallback.name })
          : t("settings.audio.defaultDevice"),
      },
      ...devices.map((device) => ({ value: device.id, label: device.name })),
    ];
  };

  const detailOf = (row: Row): ReactNode => {
    if (row.kind === "mix")
      return (
        <Select
          label={t("settings.audio.output")}
          className="w-full max-w-80 min-w-0"
          value={audio.output_device_id ?? ""}
          options={deviceOptions(outputs)}
          onChange={(id) => setAudio({ output_device_id: id || null })}
        />
      );
    if (row.kind === "microphone")
      return (
        <>
          <Select
            label={t("settings.audio.microphone")}
            className="w-full max-w-80 min-w-0"
            value={audio.microphone_device_id ?? ""}
            options={deviceOptions(inputs)}
            onChange={(id) => setAudio({ microphone_device_id: id || null })}
          />
          <span className="flex items-center gap-1.5">
            {t("settings.audio.gain")}
            <NumberField
              value={audio.microphone_gain_db}
              min={-20}
              max={20}
              unit={t("units.dbUnit")}
              label={t("settings.audio.gainLabel")}
              onCommit={(microphone_gain_db) => setAudio({ microphone_gain_db })}
            />
          </span>
          <span className="flex items-center gap-2">
            {t("settings.audio.inFullMix", { name: trackTitle(audio, "mix") })}
            <Switch
              checked={audio.microphone_in_full_mix}
              label={t("settings.audio.inFullMixLabel", { name: trackTitle(audio, "mix") })}
              onChange={(microphone_in_full_mix) => setAudio({ microphone_in_full_mix })}
            />
          </span>
        </>
      );
    if (row.kind === "game") {
      if (gameSource)
        return (
          <span className="truncate">
            {t("settings.audio.follows", {
              name: draft.storage.game_names[gameSource.toLowerCase()] ?? exeStem(gameSource),
            })}
          </span>
        );
      return (
        <span className="truncate">
          {draft.capture.kind === "window" ? t("settings.audio.followsWindow") : t("settings.audio.followsForeground")}
        </span>
      );
    }
    const running = row.exe ? runningPid.has(row.exe.toLowerCase()) : false;
    return (
      <span className="truncate text-ink-3">
        {running ? row.exe : t("settings.audio.notLaunched", { exe: row.exe })}
      </span>
    );
  };

  const actionOf = (row: Row): ReactNode => {
    if (row.kind === "application" && row.exe) {
      const exe = row.exe;
      return (
        <button
          type="button"
          aria-label={t("settings.audio.remove", { name: row.label })}
          onClick={() => {
            setAudio({ app_tracks: audio.app_tracks.filter((track) => track.exe_name !== exe) });
            listRef.current?.focus();
          }}
          className="pressable flex size-9 items-center justify-center rounded-md text-ink-2 hover:bg-surface-hover hover:text-ink"
        >
          <X size={18} aria-hidden />
        </button>
      );
    }
    const toggle = (enabled: boolean) =>
      setAudio(
        row.kind === "mix"
          ? { record_full_mix: enabled }
          : row.kind === "microphone"
            ? { record_microphone: enabled }
            : { auto_game_track: enabled },
      );
    return <Switch checked={row.enabled} label={t("settings.audio.record", { name: row.label })} onChange={toggle} />;
  };

  const rename = (row: Row, label: string) => {
    if (row.kind === "mix") setAudio({ full_mix_label: chosenName(label, "mix") });
    else if (row.kind === "microphone") setAudio({ microphone_label: chosenName(label, "microphone") });
    else if (row.kind === "game") setAudio({ game_label: chosenName(label, "game") });
    else if (row.exe) setApp(row.exe, { label });
  };

  const setOffset = (row: Row, offset: number) => {
    if (row.kind === "mix") setAudio({ full_mix_sync_offset_ms: offset });
    else if (row.kind === "microphone") setAudio({ microphone_sync_offset_ms: offset });
    else if (row.kind === "game") setAudio({ game_sync_offset_ms: offset });
    else if (row.exe) setApp(row.exe, { sync_offset_ms: offset });
  };

  const tracked = new Set(audio.app_tracks.map((track) => track.exe_name.toLowerCase()));
  const full = audio.app_tracks.length >= audio.max_app_tracks;
  const candidates = new Map<string, Candidate>();
  for (const session of sessions) {
    candidates.set(session.exe_name.toLowerCase(), {
      exe: session.exe_name,
      name: draft.storage.game_names[session.exe_name.toLowerCase()] ?? session.app_name,
      processId: session.process_id,
      peak: session.peak,
    });
  }
  for (const target of windows) {
    if (target.kind !== "window" || candidates.has(target.exe_name.toLowerCase())) continue;
    candidates.set(target.exe_name.toLowerCase(), {
      exe: target.exe_name,
      name: draft.storage.game_names[target.exe_name.toLowerCase()] ?? target.app_name,
      processId: target.process_id,
      peak: null,
    });
  }
  const needle = query.trim().toLowerCase();
  const now = Date.now();
  const available = [...candidates.values()]
    .filter((candidate) => !tracked.has(candidate.exe.toLowerCase()))
    .filter((candidate) => !needle || candidate.name.toLowerCase().includes(needle) || candidate.exe.toLowerCase().includes(needle))
    .sort((a, b) => localeCompare(a.name, b.name));
  const sounding = available.filter((candidate) => now - (lastSound.current.get(candidate.exe.toLowerCase()) ?? 0) < RECENT_SOUND_MS);
  const fresh = [...sounding, ...available.filter((candidate) => !sounding.includes(candidate))];
  const previous = candidateOrder.current;
  const ordered = focusInList
    ? [...fresh].sort((a, b) => rank(previous, a.exe) - rank(previous, b.exe))
    : fresh;
  candidateOrder.current = ordered.map((candidate) => candidate.exe);

  const add = (exe: string, label: string) => {
    setAudio({ app_tracks: [...audio.app_tracks, { exe_name: exe, label, enabled: true, sync_offset_ms: 0 }] });
    listRef.current?.focus();
  };

  const browse = () => {
    setPickError(null);
    pickAppExe().then((picked) => {
      if (!picked) return;
      if (tracked.has(picked.exe_name.toLowerCase()))
        setPickError(t("settings.audio.alreadyTracked", { name: picked.app_name }));
      else add(picked.exe_name, picked.app_name);
    }, (failure) => setPickError(errorText(failure)));
  };

  const candidateRow = (candidate: Candidate) => (
    <li key={candidate.exe} className="grid grid-cols-[minmax(0,1fr)_minmax(80px,150px)_auto] items-center gap-x-4 border-b border-line px-5 py-2">
      <span className="flex min-w-0 items-center gap-2.5">
        <AppGlyph exe={candidate.exe} processId={candidate.processId} kind="application" color="var(--ink-2)" />
        <span className="min-w-0">
          <span className="block truncate font-semibold">{candidate.name}</span>
          <span className="block truncate text-[12px] text-ink-3">{candidate.exe}</span>
        </span>
      </span>
      <span aria-hidden className="relative block h-2 overflow-hidden rounded-[2px] bg-meter-ground">
        {candidate.peak !== null && (
          <span
            ref={(element) => {
              const key = candidate.exe.toLowerCase();
              if (element) {
                meters.current.set(key, element);
                element.style.transform = `scaleX(${meterPosition(peaks.current.get(key) ?? 0)})`;
              } else meters.current.delete(key);
            }}
            className="session-meter absolute inset-0 origin-left bg-ink-2"
          />
        )}
      </span>
      <button
        type="button"
        disabled={full}
        onClick={() => add(candidate.exe, candidate.name)}
        aria-label={t("settings.audio.addNamed", { name: candidate.name })}
        className={BUTTON}
      >
        {t("settings.audio.add")}
      </button>
    </li>
  );

  return (
    <>
      <CalibrationPanel config={config} onRunning={setCalibrating} />
      <section aria-labelledby="tracks-title">
        <div className={`${ROW_COLUMNS} px-5 pt-5 pb-3`}>
          <div className="col-span-3 flex items-baseline gap-3">
            <h3 id="tracks-title" className="text-[15px] font-semibold">
              {t("settings.audio.tracks")}
            </h3>
            <span className="score text-[15px] text-ink-2">
              {t("settings.audio.appCount", { count: audio.app_tracks.length, max: audio.max_app_tracks })}
            </span>
          </div>
          <span className="pr-[3.75rem] text-right text-[13px] text-ink-2">{t("settings.audio.offset")}</span>
        </div>
        <ul ref={listRef} tabIndex={-1} className="border-t border-line outline-none">
          {rows.map((row, position) => (
            <TrackRow
              key={row.key}
              row={row}
              index={indexes[position] ?? null}
              meterIndex={meterIndexOf(row.key)}
              isTaken={isTakenBy(row)}
              color={trackColor(row.kind, row.label, appOrder)}
              processId={row.exe ? (runningPid.get(row.exe.toLowerCase()) ?? null) : null}
              detail={detailOf(row)}
              onRename={(label) => rename(row, label)}
              onOffset={(offset) => setOffset(row, offset)}
              action={actionOf(row)}
            />
          ))}
        </ul>
      </section>

      <SettingRow label={t("settings.audio.maxApps")}>
        <Select
          label={t("settings.audio.maxApps")}
          value={audio.max_app_tracks}
          options={Array.from({ length: 12 }, (_, slot) => slot + 1)
            .filter((count) => count >= audio.app_tracks.length)
            .map((count) => ({ value: count, label: String(count) }))}
          onChange={(max_app_tracks) => setAudio({ max_app_tracks })}
        />
      </SettingRow>

      <section aria-labelledby="add-app-title" className="pb-5">
        <div className="flex flex-wrap items-center gap-3 px-5 pt-5 pb-3">
          <h3 id="add-app-title" className="mr-auto text-[15px] font-semibold">
            {t("settings.audio.addApp")}
          </h3>
          <label className="relative flex items-center">
            <Search size={16} aria-hidden className="pointer-events-none absolute left-2.5 text-ink-3" />
            <input
              type="search"
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              placeholder={t("settings.audio.search")}
              aria-label={t("settings.audio.searchLabel")}
              className="h-9 w-52 rounded-md border border-ink-3 bg-surface pr-2.5 pl-8 text-[14px] text-ink placeholder:text-ink-3 hover:border-ink-2"
            />
          </label>
          <button type="button" onClick={browse} disabled={full} className={`${BUTTON} flex items-center gap-2`}>
            <FolderOpen size={16} aria-hidden />
            {t("settings.audio.browse")}
          </button>
        </div>
        {full && (
          <p className="border-t border-line px-5 py-3 text-[13px] text-ink-2">
            {t("settings.audio.full", { max: audio.max_app_tracks })}
          </p>
        )}
        {pickError && (
          <p role="alert" className="border-t border-line px-5 py-3 text-[13px] text-warn">
            {pickError}
          </p>
        )}
        {ordered.length > 0 && (
          <ul
            className="border-t border-line"
            onFocus={() => setFocusInList(true)}
            onBlur={(event) => {
              if (!event.currentTarget.contains(event.relatedTarget as Node | null)) setFocusInList(false);
            }}
          >
            {ordered.map(candidateRow)}
          </ul>
        )}
        {available.length === 0 && (
          <p className="border-t border-line px-5 py-3 text-[13px] text-ink-2">
            {needle ? t("settings.audio.noMatch") : t("settings.audio.noOther")}
          </p>
        )}
      </section>
      <ConfigFooter config={config} focusAfter={listRef} />
    </>
  );
}
