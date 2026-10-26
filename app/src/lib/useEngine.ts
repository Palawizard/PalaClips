import { useCallback, useEffect, useState } from "react";
import {
  type AppStatus,
  type ClipSummary,
  type EngineConfig,
  type StorageStatus,
  getHotkeys,
  onHotkeysChanged,
  getState,
  listRecentClips,
  onEngineEvent,
  recordStart,
  recordStop,
  replayArm,
  replayDisarm,
  replayFinishClip,
  replaySave,
  storageStatus,
} from "./ipc.ts";
import { useLibraryChanges } from "./library.ts";
import { useLiveConfig } from "./useConfig.ts";
import { errorText } from "./i18n.ts";
import { clearLevels, pushLevels } from "./levels.ts";

const RECENT_CLIPS = 5;

export type ReplayPhase = "armed" | "idle" | "arming" | "disarming" | "error";

export interface Engine {
  status: AppStatus | null;
  config: EngineConfig | null;
  clips: ClipSummary[];
  freshClip: string | null;
  storage: StorageStatus | null;
  savedAt: number;
  saveKeys: string | null;
  recordKeys: string | null;
  phase: ReplayPhase;
  recording: boolean;
  recordPending: boolean;
  actionError: string | null;
  arm: () => void;
  disarm: () => void;
  save: () => void;
  finishClip: () => void;
  record: () => void;
  stopRecording: () => void;
}

function phaseOf(status: AppStatus | null, pending: ReplayPhase | null): ReplayPhase {
  if (pending) return pending;
  if (!status) return "idle";
  if (status.state !== "idle") return "armed";
  const failed = status.alerts.some(
    (alert) => alert.code === "replay_arm_failed" || alert.code === "replay_stopped",
  );
  return failed ? "error" : "idle";
}

export function useEngine(): Engine {
  const [status, setStatus] = useState<AppStatus | null>(null);
  const config = useLiveConfig();
  const [clips, setClips] = useState<ClipSummary[]>([]);
  const [freshClip, setFreshClip] = useState<string | null>(null);
  const [storage, setStorage] = useState<StorageStatus | null>(null);
  const [savedAt, setSavedAt] = useState(0);
  const [pending, setPending] = useState<ReplayPhase | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const [saveKeys, setSaveKeys] = useState<string | null>(null);
  const [recordKeys, setRecordKeys] = useState<string | null>(null);
  const [recordPending, setRecordPending] = useState(false);

  const refreshState = useCallback(() => {
    getState().then(setStatus, (error) => setActionError(errorText(error)));
  }, []);

  const refreshLibrary = useCallback(() => {
    listRecentClips(RECENT_CLIPS).then(setClips, () => setClips([]));
    storageStatus().then(setStorage, () => setStorage(null));
  }, []);

  useLibraryChanges(refreshLibrary);

  useEffect(() => {
    refreshState();
    refreshLibrary();
    const refreshHotkeys = () =>
      getHotkeys().then(
        (view) => {
          setSaveKeys(view.hotkeys.save_replay);
          setRecordKeys(view.hotkeys.toggle_recording);
        },
        () => setSaveKeys(null),
      );
    refreshHotkeys();
    const unlistenHotkeys = onHotkeysChanged(refreshHotkeys);
    const unlisten = onEngineEvent((event) => {
      switch (event.type) {
        case "audio_levels":
          pushLevels(event.levels);
          break;
        case "replay_stats":
          setStatus((current) =>
            current
              ? { ...current, buffered_secs: event.buffered_secs, buffer_bytes: event.bytes }
              : current,
          );
          break;
        case "clip_saved":
          setFreshClip(event.path);
          setSavedAt(Date.now());
          refreshLibrary();
          break;
        case "recording_stats":
          setStatus((current) =>
            current ? { ...current, recording_secs: event.elapsed_secs, recording_bytes: event.bytes } : current,
          );
          break;
        case "state_changed":
          setPending(null);
          setRecordPending(false);
          if (event.state === "idle") clearLevels();
          refreshState();
          break;
        case "alert":
          if (event.code === "replay_arm_failed") setPending(null);
          if (event.code === "recording_failed" || event.code === "replay_arm_failed") setRecordPending(false);
          if (event.code === "recording_recovered") refreshLibrary();
          refreshState();
          break;
        default:
          refreshState();
      }
    });
    return () => {
      unlisten.then((stop) => stop());
      unlistenHotkeys.then((stop) => stop());
    };
  }, [refreshLibrary, refreshState]);

  const run = useCallback((action: () => Promise<void>, phase: ReplayPhase | null) => {
    setActionError(null);
    if (phase) setPending(phase);
    action().catch((error) => {
      setPending(null);
      setRecordPending(false);
      setActionError(errorText(error));
    });
  }, []);

  return {
    status,
    config,
    clips,
    freshClip,
    storage,
    savedAt,
    saveKeys,
    recordKeys,
    phase: phaseOf(status, pending),
    recording: status?.state === "recording",
    recordPending,
    actionError,
    arm: () => run(replayArm, "arming"),
    disarm: () => run(replayDisarm, "disarming"),
    save: () => run(replaySave, null),
    finishClip: () => run(replayFinishClip, null),
    record: () => {
      setRecordPending(true);
      run(recordStart, null);
    },
    stopRecording: () => {
      setRecordPending(true);
      run(recordStop, null);
    },
  };
}
