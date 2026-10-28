import { useEffect, useRef, useState } from "react";
import { type LanguageSetting, currentLanguage, errorText, setLanguage, t as translate, useT } from "../../lib/i18n.ts";
import {
  type Feedback,
  type Theme,
  type UpdateInfo,
  checkUpdate,
  getUpdate,
  installUpdate,
  exportDiagnostic,
  openDiagnostic,
  getAutostart,
  getFeedback,
  getPlayer,
  getLanguage,
  getTheme,
  resetSettings,
  setAutostart,
  setFeedback,
  setPlayer,
  setLanguageSetting,
  setTheme,
} from "../../lib/ipc.ts";
import { saveConfig, useLiveConfig } from "../../lib/useConfig.ts";
import { applyTheme } from "../../lib/theme.ts";
import { Segmented, SettingRow, Switch } from "./Settings.tsx";

const THEMES: Theme[] = ["system", "dark", "light"];
const LANGUAGES: LanguageSetting[] = ["system", "fr", "en"];

function notesInLanguage(notes: string): string {
  const heading = translate(`settings.general.languages.${currentLanguage()}`);
  const block = notes.split(/^## /m).find((part) => part.split(/\r?\n/, 1)[0]?.trim() === heading);
  return block ? block.slice(block.indexOf("\n") + 1).trim() : notes;
}

export function DiagnosticRow({ onError }: { onError: (message: string) => void }) {
  const t = useT();
  const [exporting, setExporting] = useState(false);
  const [exported, setExported] = useState<string | null>(null);

  const exportReport = () => {
    if (exporting) return;
    setExporting(true);
    exportDiagnostic()
      .then((name) => {
        if (name) setExported(name);
      })
      .catch((failure) => onError(errorText(failure)))
      .finally(() => setExporting(false));
  };

  return (
    <SettingRow
      label={t("settings.general.diagnostic")}
      help={exported ? t("settings.general.diagnosticSaved", { name: exported }) : t("settings.general.diagnosticHelp")}
    >
      <span className="flex gap-2">
        {exported && (
          <button
            type="button"
            onClick={() => openDiagnostic().catch((failure) => onError(errorText(failure)))}
            className="pressable h-9 rounded-md border border-line-strong px-3.5 font-medium text-ink hover:bg-surface-hover"
          >
            {t("settings.general.diagnosticOpen")}
          </button>
        )}
        <button
          type="button"
          onClick={exportReport}
          aria-disabled={exporting}
          aria-busy={exporting}
          className="pressable h-9 rounded-md border border-line-strong px-3.5 font-medium text-ink hover:bg-surface-hover aria-disabled:cursor-progress aria-disabled:opacity-50"
        >
          {t("settings.general.diagnosticExport")}
        </button>
      </span>
    </SettingRow>
  );
}

const BUTTON =
  "pressable h-9 rounded-md border border-line-strong px-3.5 font-medium text-ink hover:bg-surface-hover aria-disabled:cursor-progress aria-disabled:opacity-50";

function UpdateRow({ onError }: { onError: (message: string) => void }) {
  const t = useT();
  const [info, setInfo] = useState<UpdateInfo | null>(null);
  const [checked, setChecked] = useState(false);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    getUpdate().then(setInfo, () => setInfo(null));
  }, []);

  const run = (action: () => Promise<void>) => {
    if (busy) return;
    setBusy(true);
    action()
      .catch((failure) => onError(errorText(failure)))
      .finally(() => setBusy(false));
  };

  const check = () =>
    run(() =>
      checkUpdate().then((next) => {
        setInfo(next);
        setChecked(true);
      }),
    );

  const available = info?.available ?? null;
  const help = !info
    ? undefined
    : available
      ? t("settings.general.updateAvailable", { version: available.version, current: info.current })
      : checked
        ? t("settings.general.upToDate", { current: info.current })
        : t("settings.general.version", { current: info.current });

  return (
    <>
      <SettingRow label={t("settings.general.updates")} help={help}>
        {available ? (
          <button
            type="button"
            onClick={() => run(installUpdate)}
            aria-disabled={busy}
            aria-busy={busy}
            className="pressable h-9 rounded-md bg-ink px-3.5 font-semibold text-ground hover:bg-ink/90 aria-disabled:cursor-progress aria-disabled:opacity-50"
          >
            {busy ? t("settings.general.updateInstalling") : t("settings.general.updateInstall")}
          </button>
        ) : (
          <button type="button" onClick={check} aria-disabled={busy} aria-busy={busy} className={BUTTON}>
            {t("settings.general.updateCheck")}
          </button>
        )}
      </SettingRow>
      {available?.notes && (
        <div className="border-b border-line px-5 py-3">
          <p className="text-[13px] font-semibold">{t("settings.general.updateNotes")}</p>
          <p
            tabIndex={0}
            aria-label={t("settings.general.updateNotes")}
            className="mt-1 max-h-48 max-w-[78ch] overflow-y-auto text-[13px] whitespace-pre-line text-ink-2"
          >
            {notesInLanguage(available.notes)}
          </p>
        </div>
      )}
    </>
  );
}

export function GeneralSection() {
  const t = useT();
  const [autostart, setAutostartState] = useState<boolean | null>(null);
  const config = useLiveConfig();
  const [feedback, setFeedbackState] = useState<Feedback | null>(null);
  const [muteFullMix, setMuteFullMix] = useState<boolean | null>(null);
  const [theme, setThemeState] = useState<Theme>("system");
  const [language, setLanguageState] = useState<LanguageSetting>("system");
  const [error, setError] = useState<string | null>(null);
  const [confirmReset, setConfirmReset] = useState(false);
  const resetTrigger = useRef<HTMLButtonElement>(null);
  const resetCancel = useRef<HTMLButtonElement>(null);
  const resetOpened = useRef(false);

  useEffect(() => {
    if (confirmReset) {
      resetOpened.current = true;
      resetCancel.current?.focus();
    } else if (resetOpened.current) {
      resetTrigger.current?.focus();
    }
  }, [confirmReset]);

  const load = () => {
    getAutostart().then(setAutostartState, (failure) => setError(errorText(failure)));
    getFeedback().then(setFeedbackState, (failure) => setError(errorText(failure)));
    getPlayer().then((player) => setMuteFullMix(player.mute_full_mix), (failure) => setError(errorText(failure)));
    getTheme().then(setThemeState, () => setThemeState("system"));
    getLanguage().then(
      (view) => {
        setLanguageState(view.setting);
        setLanguage(view.resolved);
      },
      () => setLanguageState("system"),
    );
  };

  useEffect(load, []);

  const run = (action: Promise<void>, undo: () => void) => {
    setError(null);
    action.catch((failure) => {
      undo();
      setError(errorText(failure));
    });
  };

  const changeAutostart = (enabled: boolean) => {
    setAutostartState(enabled);
    run(setAutostart(enabled), () => setAutostartState(!enabled));
  };

  const changeArm = (enabled: boolean) => {
    run(saveConfig({ replay: { arm_on_startup: enabled } }).then(() => {}), () => {});
  };

  const changeFeedback = (next: Feedback) => {
    if (!feedback) return;
    const previous = feedback;
    setFeedbackState(next);
    run(setFeedback(next), () => setFeedbackState(previous));
  };

  const changeMuteFullMix = (mute_full_mix: boolean) => {
    setMuteFullMix(mute_full_mix);
    run(setPlayer({ mute_full_mix }), () => setMuteFullMix(!mute_full_mix));
  };

  const changeTheme = (next: Theme) => {
    const previous = theme;
    setThemeState(next);
    applyTheme(next);
    run(setTheme(next), () => {
      setThemeState(previous);
      applyTheme(previous);
    });
  };

  const changeLanguage = (next: LanguageSetting) => {
    const previous = language;
    setLanguageState(next);
    setError(null);
    setLanguageSetting(next).then(setLanguage, (failure) => {
      setLanguageState(previous);
      setError(errorText(failure));
    });
  };

  const reset = () => {
    setConfirmReset(false);
    setError(null);
    resetSettings()
      .then(() => applyTheme("system"))
      .catch((failure) => setError(errorText(failure)))
      .finally(load);
  };

  return (
    <>
      {error && (
        <p role="alert" className="border-b border-line bg-surface-2 px-5 py-3 text-[13px] text-warn">
          {error}
        </p>
      )}
      <SettingRow label={t("settings.general.autostart")} help={t("settings.general.autostartHelp")}>
        {autostart !== null && (
          <Switch checked={autostart} label={t("settings.general.autostart")} onChange={changeAutostart} />
        )}
      </SettingRow>
      <SettingRow label={t("settings.general.armOnStartup")}>
        {config && (
          <Switch
            checked={config.replay.arm_on_startup}
            label={t("settings.general.armOnStartup")}
            onChange={changeArm}
          />
        )}
      </SettingRow>
      <SettingRow label={t("settings.general.sound")} help={t("settings.general.soundHelp")}>
        {feedback && (
          <Switch
            checked={feedback.sound}
            label={t("settings.general.sound")}
            onChange={(sound) => changeFeedback({ ...feedback, sound })}
          />
        )}
      </SettingRow>
      <SettingRow label={t("settings.general.toast")} help={t("settings.general.toastHelp")}>
        {feedback && (
          <Switch
            checked={feedback.toast}
            label={t("settings.general.toast")}
            onChange={(toast) => changeFeedback({ ...feedback, toast })}
          />
        )}
      </SettingRow>
      <SettingRow label={t("settings.general.muteFullMix")} help={t("settings.general.muteFullMixHelp")}>
        {muteFullMix !== null && (
          <Switch checked={muteFullMix} label={t("settings.general.muteFullMix")} onChange={changeMuteFullMix} />
        )}
      </SettingRow>
      <SettingRow label={t("settings.general.theme")}>
        <Segmented
          value={theme}
          options={THEMES.map((value) => ({ value, label: t(`settings.general.themes.${value}`) }))}
          label={t("settings.general.theme")}
          onChange={changeTheme}
        />
      </SettingRow>
      <SettingRow label={t("settings.general.language")}>
        <Segmented
          value={language}
          options={LANGUAGES.map((value) => ({ value, label: t(`settings.general.languages.${value}`) }))}
          label={t("settings.general.language")}
          onChange={changeLanguage}
        />
      </SettingRow>
      <UpdateRow onError={setError} />
      <DiagnosticRow onError={setError} />
      <SettingRow
        label={t("settings.general.reset")}
        help={confirmReset ? t("settings.general.resetHelp") : undefined}
      >
        {confirmReset ? (
          <span className="flex gap-2">
            <button
              type="button"
              onClick={reset}
              className="pressable h-9 rounded-md bg-ink px-3.5 font-semibold text-ground hover:bg-ink/90"
            >
              {t("settings.general.resetConfirm")}
            </button>
            <button
              ref={resetCancel}
              type="button"
              onClick={() => setConfirmReset(false)}
              className="pressable h-9 rounded-md border border-line-strong px-3.5 font-medium text-ink hover:bg-surface-hover"
            >
              {t("settings.cancel")}
            </button>
          </span>
        ) : (
          <button
            ref={resetTrigger}
            type="button"
            onClick={() => setConfirmReset(true)}
            className="pressable h-9 rounded-md border border-line-strong px-3.5 font-medium text-ink hover:bg-surface-hover"
          >
            {t("settings.general.resetOpen")}
          </button>
        )}
      </SettingRow>
    </>
  );
}
