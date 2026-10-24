import { type RefObject, useEffect, useRef, useState } from "react";
import { type Key, errorText, useT } from "../../lib/i18n.ts";
import { type ConfigPatch, configPatch, restartsReplay, withPatch } from "../../lib/config.ts";
import { type EngineConfig, getState, onEngineEvent } from "../../lib/ipc.ts";
import { configReady, saveConfig, useLiveConfig } from "../../lib/useConfig.ts";
import { usePendingChange } from "../LeaveDialog.tsx";

const APPLIED_ON_DISARM: Key = "settings.applied";

export interface ConfigDraft {
  saved: EngineConfig;
  draft: EngineConfig;
  armed: boolean;
  pendingRestart: boolean;
  error: string | null;
  notice: Key | null;
  change: (next: EngineConfig) => void;
  commit: (next: EngineConfig) => void;
  apply: () => Promise<boolean>;
  cancel: () => void;
}

export function useConfigDraft(page?: Key): { config: ConfigDraft | null; loadError: string | null } {
  const saved = useLiveConfig();
  const [patch, setPatch] = useState<ConfigPatch>({});
  const [armed, setArmed] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<Key | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const saving = useRef(0);
  const latest = useRef({ saved, patch });
  latest.current = { saved, patch };

  const save = (next: ConfigPatch, appliedNotice?: Key): Promise<boolean> => {
    setError(null);
    saving.current += 1;
    return saveConfig(next)
      .then(
        (config) => {
          setPatch((current) => configPatch(config, withPatch(config, current)));
          if (appliedNotice) setNotice(appliedNotice);
          return true;
        },
        (failure) => {
          setPatch({});
          setError(errorText(failure));
          return false;
        },
      )
      .finally(() => {
        saving.current -= 1;
      });
  };

  useEffect(() => {
    configReady().catch((failure) => setLoadError(errorText(failure)));
    getState().then((status) => setArmed(status.state !== "idle"), () => setArmed(false));
    const unlisten = onEngineEvent((event) => {
      if (event.type === "state_changed") setArmed(event.state !== "idle");
    });
    return () => {
      unlisten.then((stop) => stop());
    };
  }, []);

  useEffect(() => {
    const { saved: current, patch: pending } = latest.current;
    if (!armed && saving.current === 0 && current && restartsReplay(current, withPatch(current, pending)))
      save(pending, APPLIED_ON_DISARM);
  }, [armed]);

  const draft = saved ? withPatch(saved, patch) : null;
  const pendingRestart = armed && saved !== null && draft !== null && restartsReplay(saved, draft);
  usePendingChange(
    page ?? "",
    page && pendingRestart
      ? { page, restarts: true, apply: () => save(latest.current.patch), discard: () => setPatch({}) }
      : null,
  );

  if (!saved || !draft) return { config: null, loadError };

  return {
    loadError,
    config: {
      saved,
      draft,
      armed,
      pendingRestart,
      error,
      notice,
      change: (next) => {
        setNotice(null);
        const nextPatch = configPatch(saved, next);
        setPatch(nextPatch);
        if (!armed || !restartsReplay(saved, next)) save(nextPatch);
      },
      commit: (next) => {
        setNotice(null);
        const nextPatch = configPatch(saved, next);
        setPatch(nextPatch);
        save(nextPatch);
      },
      apply: () => save(patch),
      cancel: () => setPatch({}),
    },
  };
}

export function ConfigFooter({ config, focusAfter }: { config: ConfigDraft; focusAfter: RefObject<HTMLElement | null> }) {
  const t = useT();
  return (
    <div className="sticky bottom-0 z-10">
      {config.error && (
        <p role="alert" className="border-t border-line bg-surface-2 px-5 py-3 text-[13px] text-warn">
          {config.error}
        </p>
      )}
      {config.pendingRestart && (
        <div role="alert" className="flex flex-wrap items-center gap-3 border-t border-line bg-surface-2 px-5 py-3">
          <p className="flex-1 text-[13px] text-ink">{t("settings.restartWarning")}</p>
          <button
            type="button"
            onClick={() => {
              config.cancel();
              focusAfter.current?.focus();
            }}
            className="pressable h-9 rounded-md border border-line-strong px-3.5 font-medium text-ink hover:bg-surface-hover"
          >
            {t("leave.discard")}
          </button>
          <button
            type="button"
            onClick={() => {
              focusAfter.current?.focus();
              void config.apply();
            }}
            className="pressable h-9 rounded-md bg-ink px-3.5 font-semibold text-ground hover:bg-ink/90"
          >
            {t("settings.apply")}
          </button>
        </div>
      )}
      {config.notice && !config.pendingRestart && (
        <p role="status" className="border-t border-line bg-surface-2 px-5 py-3 text-[13px] text-ink">
          {t(config.notice)}
        </p>
      )}
    </div>
  );
}

export function LoadError({ message }: { message: string | null }) {
  return message ? (
    <p role="alert" className="border-b border-line bg-surface-2 px-5 py-3 text-[13px] text-warn">
      {message}
    </p>
  ) : null;
}
