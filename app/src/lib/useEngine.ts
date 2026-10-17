import { useCallback, useEffect, useState } from "react";
import {
  type AppStatus,
  type ClipSummary,
  type EngineConfig,
  type StorageStatus,
  getConfig,
  getHotkeys,
  getState,
  listRecentClips,
  onEngineEvent,
  replayArm,
  replayDisarm,
  replaySave,
  storageStatus,
} from "./ipc.ts";
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
  phase: ReplayPhase;
  actionError: string | null;
  arm: () => void;
  disarm: () => void;
  save: () => void;
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
  const [config, setConfig] = useState<EngineConfig | null>(null);
  const [clips, setClips] = useState<ClipSummary[]>([]);
  const [freshClip, setFreshClip] = useState<string | null>(null);
  const [storage, setStorage] = useState<StorageStatus | null>(null);
  const [savedAt, setSavedAt] = useState(0);
  const [pending, setPending] = useState<ReplayPhase | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const [saveKeys, setSaveKeys] = useState<string | null>(null);

  const refreshState = useCallback(() => {
    getState().then(setStatus, (error) => setActionError(errorText(error)));
  }, []);

  const refreshLibrary = useCallback(() => {
    listRecentClips(RECENT_CLIPS).then(setClips, () => setClips([]));
    storageStatus().then(setStorage, () => setStorage(null));
  }, []);

  useEffect(() => {
    refreshState();
    refreshLibrary();
    getConfig().then(setConfig, () => setConfig(null));
    getHotkeys().then(
      (view) => setSaveKeys(view.hotkeys.save_replay),
      () => setSaveKeys(null),
    );
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
        case "state_changed":
          setPending(null);
          if (event.state === "idle") clearLevels();
          refreshState();
          break;
        case "alert":
          if (event.code === "replay_arm_failed") setPending(null);
          refreshState();
          break;
        default:
          refreshState();
      }
    });
    return () => {
      unlisten.then((stop) => stop());
    };
  }, [refreshLibrary, refreshState]);

  const run = useCallback((action: () => Promise<void>, phase: ReplayPhase | null) => {
    setActionError(null);
    if (phase) setPending(phase);
    action().catch((error) => {
      setPending(null);
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
    phase: phaseOf(status, pending),
    actionError,
    arm: () => run(replayArm, "arming"),
    disarm: () => run(replayDisarm, "disarming"),
    save: () => run(replaySave, null),
  };
}
