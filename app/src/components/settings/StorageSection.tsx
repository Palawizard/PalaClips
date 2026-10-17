import { useEffect, useId, useState } from "react";
import { bytes } from "../../lib/format.ts";
import { errorText, localeCompare, useT } from "../../lib/i18n.ts";
import {
  type EngineConfig,
  type StorageStatus,
  getConfig,
  openClipsDir,
  pickClipsDir,
  setConfig,
  storageStatus,
} from "../../lib/ipc.ts";
import { SettingRow } from "./Settings.tsx";

const BUTTON =
  "pressable h-9 rounded-md border border-line-strong px-3.5 font-medium text-ink hover:bg-surface-hover";

function GameNameRow({
  exe,
  name,
  error,
  onCommit,
  onCancel,
}: {
  exe: string;
  name: string;
  error: string | null;
  onCommit: (name: string) => void;
  onCancel: () => void;
}) {
  const t = useT();
  const [text, setText] = useState(name);
  const errorId = useId();
  useEffect(() => setText(name), [name]);
  const commit = () => {
    const next = text.trim().replace(/\s+/g, " ");
    if (next === name) setText(name);
    else onCommit(next);
  };
  return (
    <li className="grid grid-cols-[minmax(0,16rem)_minmax(0,1fr)] items-center gap-x-6 gap-y-1 border-b border-line px-5 py-2.5">
      <input
        type="text"
        value={text}
        spellCheck={false}
        onChange={(event) => setText(event.target.value)}
        onBlur={commit}
        onKeyDown={(event) => {
          if (event.key === "Enter") commit();
          if (event.key === "Escape") {
            setText(name);
            onCancel();
          }
        }}
        aria-label={t("settings.storage.folderFor", { exe })}
        aria-invalid={error ? true : undefined}
        aria-describedby={error ? errorId : undefined}
        className="h-9 w-full rounded-md border border-ink-3 bg-surface px-2.5 text-[14px] text-ink hover:border-ink-2 aria-invalid:border-warn"
      />
      <span className="truncate text-[13px] text-ink-3" title={exe}>
        {exe}
      </span>
      {error && (
        <p id={errorId} role="alert" className="col-span-2 text-[13px] text-warn">
          {error}
        </p>
      )}
    </li>
  );
}

export function StorageSection() {
  const t = useT();
  const [config, setConfigState] = useState<EngineConfig | null>(null);
  const [status, setStatus] = useState<StorageStatus | null>(null);
  const [folderError, setFolderError] = useState<string | null>(null);
  const [nameError, setNameError] = useState<{ exe: string; message: string } | null>(null);

  const refreshStatus = () => storageStatus().then(setStatus, () => setStatus(null));

  useEffect(() => {
    getConfig().then(setConfigState, (failure) => setFolderError(errorText(failure)));
    refreshStatus();
  }, []);

  if (!config) {
    return folderError ? (
      <p role="alert" className="border-b border-line bg-surface-2 px-5 py-3 text-[13px] text-warn">
        {folderError}
      </p>
    ) : null;
  }

  const save = (next: EngineConfig) => setConfig(next).then(() => setConfigState(next));

  const changeFolder = () => {
    setFolderError(null);
    pickClipsDir()
      .then((dir) => {
        if (!dir) return;
        return save({ ...config, storage: { ...config.storage, clips_dir: dir } }).then(refreshStatus);
      })
      .catch((failure) => setFolderError(errorText(failure)));
  };

  const openFolder = () => {
    setFolderError(null);
    openClipsDir().catch((failure) => setFolderError(errorText(failure)));
  };

  const rename = (exe: string, name: string) => {
    setNameError(null);
    const game_names = { ...config.storage.game_names, [exe]: name };
    save({ ...config, storage: { ...config.storage, game_names } }).catch((failure) =>
      setNameError({ exe, message: errorText(failure) }),
    );
  };

  const games = Object.entries(config.storage.game_names).sort(([, a], [, b]) => localeCompare(a, b));

  return (
    <>
      <SettingRow
        label={t("settings.storage.folder")}
        error={folderError}
        help={
          <>
            <span className="block break-all">{status?.clips_dir ?? "—"}</span>
            {status && (
              <span className="score mt-0.5 block text-[14px]">
                {t("settings.storage.freeOf", { free: bytes(status.free_bytes), total: bytes(status.total_bytes) })}
              </span>
            )}
          </>
        }
      >
        <span className="flex gap-2">
          <button type="button" onClick={changeFolder} className={BUTTON}>
            {t("settings.storage.change")}
          </button>
          <button type="button" onClick={openFolder} className={BUTTON}>
            {t("settings.storage.open")}
          </button>
        </span>
      </SettingRow>

      <section aria-labelledby="game-names-title">
        <div className="border-b border-line px-5 pt-5 pb-3">
          <h3 id="game-names-title" className="text-[15px] font-semibold">
            {t("settings.storage.gameNames")}
          </h3>
          <p className="mt-0.5 text-[13px] text-ink-2">{t("settings.storage.gameNamesHelp")}</p>
        </div>
        {games.length === 0 ? (
          <p className="border-b border-line px-5 py-3 text-[13px] text-ink-2">{t("settings.storage.noGames")}</p>
        ) : (
          <ul>
            <li
              aria-hidden="true"
              className="grid grid-cols-[minmax(0,16rem)_minmax(0,1fr)] gap-x-6 border-b border-line px-5 py-2"
            >
              <span className="label">{t("settings.storage.folderColumn")}</span>
              <span className="label">{t("settings.storage.exeColumn")}</span>
            </li>
            {games.map(([exe, name]) => (
              <GameNameRow
                key={exe}
                exe={exe}
                name={name}
                error={nameError?.exe === exe ? nameError.message : null}
                onCommit={(next) => rename(exe, next)}
                onCancel={() => setNameError(null)}
              />
            ))}
          </ul>
        )}
      </section>
    </>
  );
}
