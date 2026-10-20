import { useEffect, useRef, useState } from "react";
import { type LanguageSetting, errorText, setLanguage, useT } from "../../lib/i18n.ts";
import {
  type EngineConfig,
  type Feedback,
  type Theme,
  exportDiagnostic,
  openDiagnostic,
  getAutostart,
  getConfig,
  getFeedback,
  getLanguage,
  getTheme,
  resetSettings,
  setAutostart,
  setConfig,
  setFeedback,
  setLanguageSetting,
  setTheme,
} from "../../lib/ipc.ts";
import { applyTheme } from "../../lib/theme.ts";
import { Segmented, SettingRow, Switch } from "./Settings.tsx";

const THEMES: Theme[] = ["system", "dark", "light"];
const LANGUAGES: LanguageSetting[] = ["system", "fr", "en"];

export function GeneralSection() {
  const t = useT();
  const [autostart, setAutostartState] = useState<boolean | null>(null);
  const [config, setConfigState] = useState<EngineConfig | null>(null);
  const [feedback, setFeedbackState] = useState<Feedback | null>(null);
  const [theme, setThemeState] = useState<Theme>("system");
  const [language, setLanguageState] = useState<LanguageSetting>("system");
  const [error, setError] = useState<string | null>(null);
  const [confirmReset, setConfirmReset] = useState(false);
  const [exporting, setExporting] = useState(false);
  const [exported, setExported] = useState<string | null>(null);
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
    getConfig().then(setConfigState, (failure) => setError(errorText(failure)));
    getFeedback().then(setFeedbackState, (failure) => setError(errorText(failure)));
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
    if (!config) return;
    const previous = config;
    const next = { ...config, replay: { ...config.replay, arm_on_startup: enabled } };
    setConfigState(next);
    run(setConfig(next), () => setConfigState(previous));
  };

  const changeFeedback = (next: Feedback) => {
    if (!feedback) return;
    const previous = feedback;
    setFeedbackState(next);
    run(setFeedback(next), () => setFeedbackState(previous));
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

  const exportReport = () => {
    if (exporting) return;
    setError(null);
    setExporting(true);
    exportDiagnostic()
      .then((name) => {
        if (name) setExported(name);
      })
      .catch((failure) => setError(errorText(failure)))
      .finally(() => setExporting(false));
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
      <SettingRow
        label={t("settings.general.diagnostic")}
        help={exported ? t("settings.general.diagnosticSaved", { name: exported }) : t("settings.general.diagnosticHelp")}
      >
        <span className="flex gap-2">
          {exported && (
            <button
              type="button"
              onClick={() => openDiagnostic().catch((failure) => setError(errorText(failure)))}
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
