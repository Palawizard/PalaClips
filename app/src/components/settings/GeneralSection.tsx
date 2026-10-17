import { useEffect, useState } from "react";
import {
  type EngineConfig,
  type Feedback,
  type Theme,
  getAutostart,
  getConfig,
  getFeedback,
  getTheme,
  resetSettings,
  setAutostart,
  setConfig,
  setFeedback,
  setTheme,
} from "../../lib/ipc.ts";
import { applyTheme } from "../../lib/theme.ts";
import { Segmented, SettingRow, Switch } from "./Settings.tsx";

const THEMES: { value: Theme; label: string }[] = [
  { value: "system", label: "Système" },
  { value: "dark", label: "Sombre" },
  { value: "light", label: "Clair" },
];

export function GeneralSection() {
  const [autostart, setAutostartState] = useState<boolean | null>(null);
  const [config, setConfigState] = useState<EngineConfig | null>(null);
  const [feedback, setFeedbackState] = useState<Feedback | null>(null);
  const [theme, setThemeState] = useState<Theme>("system");
  const [error, setError] = useState<string | null>(null);
  const [confirmReset, setConfirmReset] = useState(false);

  const load = () => {
    getAutostart().then(setAutostartState, (failure) => setError(String(failure)));
    getConfig().then(setConfigState, (failure) => setError(String(failure)));
    getFeedback().then(setFeedbackState, (failure) => setError(String(failure)));
    getTheme().then(setThemeState, () => setThemeState("system"));
  };

  useEffect(load, []);

  const run = (action: Promise<void>, undo: () => void) => {
    setError(null);
    action.catch((failure) => {
      undo();
      setError(String(failure));
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

  const reset = () => {
    setConfirmReset(false);
    setError(null);
    resetSettings()
      .then(() => applyTheme("system"))
      .catch((failure) => setError(String(failure)))
      .finally(load);
  };

  return (
    <>
      {error && (
        <p role="alert" className="border-b border-line bg-surface-2 px-5 py-3 text-[13px] text-warn">
          {error}
        </p>
      )}
      <SettingRow label="Démarrer avec Windows" help="Dans la zone de notification, sans fenêtre.">
        {autostart !== null && (
          <Switch checked={autostart} label="Démarrer avec Windows" onChange={changeAutostart} />
        )}
      </SettingRow>
      <SettingRow label="Armer le replay au démarrage">
        {config && (
          <Switch
            checked={config.replay.arm_on_startup}
            label="Armer le replay au démarrage"
            onChange={changeArm}
          />
        )}
      </SettingRow>
      <SettingRow label="Son de confirmation" help="Joué à chaque clip sauvegardé, même en jeu.">
        {feedback && (
          <Switch
            checked={feedback.sound}
            label="Son de confirmation"
            onChange={(sound) => changeFeedback({ ...feedback, sound })}
          />
        )}
      </SettingRow>
      <SettingRow
        label="Notification de confirmation"
        help="Windows la retient pendant une partie en plein écran."
      >
        {feedback && (
          <Switch
            checked={feedback.toast}
            label="Notification de confirmation"
            onChange={(toast) => changeFeedback({ ...feedback, toast })}
          />
        )}
      </SettingRow>
      <SettingRow label="Thème">
        <Segmented value={theme} options={THEMES} label="Thème" onChange={changeTheme} />
      </SettingRow>
      <SettingRow
        label="Réinitialiser les réglages"
        help={
          confirmReset
            ? "Tous les réglages reviennent aux valeurs par défaut, raccourcis compris."
            : undefined
        }
      >
        {confirmReset ? (
          <span className="flex gap-2">
            <button
              type="button"
              onClick={reset}
              className="pressable h-9 rounded-md bg-ink px-3.5 font-semibold text-ground hover:bg-ink/90"
            >
              Réinitialiser
            </button>
            <button
              type="button"
              onClick={() => setConfirmReset(false)}
              className="pressable h-9 rounded-md border border-line-strong px-3.5 font-medium text-ink hover:bg-surface-hover"
            >
              Annuler
            </button>
          </span>
        ) : (
          <button
            type="button"
            onClick={() => setConfirmReset(true)}
            className="pressable h-9 rounded-md border border-line-strong px-3.5 font-medium text-ink hover:bg-surface-hover"
          >
            Réinitialiser…
          </button>
        )}
      </SettingRow>
    </>
  );
}
