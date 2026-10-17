import { type RefObject, useEffect, useRef, useState } from "react";
import { type Key, errorText, useT } from "../../lib/i18n.ts";
import { type EngineConfig, getConfig, getState, onEngineEvent, setConfig } from "../../lib/ipc.ts";

const APPLIED_ON_DISARM: Key = "settings.applied";

export function restartsReplay(before: EngineConfig, after: EngineConfig): boolean {
  return (
    JSON.stringify(before.capture) !== JSON.stringify(after.capture) ||
    JSON.stringify(before.video) !== JSON.stringify(after.video) ||
    JSON.stringify(before.audio) !== JSON.stringify(after.audio) ||
    before.replay.storage !== after.replay.storage
  );
}

export interface ConfigDraft {
  saved: EngineConfig;
  draft: EngineConfig;
  armed: boolean;
  pendingRestart: boolean;
  error: string | null;
  notice: Key | null;
  change: (next: EngineConfig) => void;
  commit: (next: EngineConfig) => void;
  apply: () => void;
  cancel: () => void;
}

export function useConfigDraft(): { config: ConfigDraft | null; loadError: string | null } {
  const [saved, setSaved] = useState<EngineConfig | null>(null);
  const [draft, setDraft] = useState<EngineConfig | null>(null);
  const [armed, setArmed] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<Key | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const latest = useRef({ saved, draft });
  latest.current = { saved, draft };

  const save = (next: EngineConfig, appliedNotice?: Key) => {
    setError(null);
    setConfig(next).then(
      () => {
        setSaved(next);
        setDraft(next);
        if (appliedNotice) setNotice(appliedNotice);
      },
      (failure) => {
        setDraft(latest.current.saved);
        setError(errorText(failure));
      },
    );
  };

  useEffect(() => {
    getConfig().then((config) => {
      setSaved(config);
      setDraft(config);
    }, (failure) => setLoadError(errorText(failure)));
    getState().then((status) => setArmed(status.state !== "idle"), () => setArmed(false));
    const unlisten = onEngineEvent((event) => {
      if (event.type === "state_changed") setArmed(event.state !== "idle");
    });
    return () => {
      unlisten.then((stop) => stop());
    };
  }, []);

  useEffect(() => {
    const { saved: current, draft: pending } = latest.current;
    if (!armed && current && pending && restartsReplay(current, pending)) save(pending, APPLIED_ON_DISARM);
  }, [armed]);

  if (!saved || !draft) return { config: null, loadError };

  return {
    loadError,
    config: {
      saved,
      draft,
      armed,
      pendingRestart: armed && restartsReplay(saved, draft),
      error,
      notice,
      change: (next) => {
        setNotice(null);
        setDraft(next);
        if (!armed || !restartsReplay(saved, next)) save(next);
      },
      commit: (next) => {
        setNotice(null);
        setDraft(next);
        save(next);
      },
      apply: () => save(draft),
      cancel: () => setDraft(saved),
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
              focusAfter.current?.focus();
              config.apply();
            }}
            className="pressable h-9 rounded-md bg-ink px-3.5 font-semibold text-ground hover:bg-ink/90"
          >
            {t("settings.apply")}
          </button>
          <button
            type="button"
            onClick={() => {
              config.cancel();
              focusAfter.current?.focus();
            }}
            className="pressable h-9 rounded-md border border-line-strong px-3.5 font-medium text-ink hover:bg-surface-hover"
          >
            {t("settings.cancel")}
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
