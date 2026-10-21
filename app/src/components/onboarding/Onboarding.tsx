import { Check, CircleCheck, LoaderCircle, TriangleAlert } from "lucide-react";
import { type KeyboardEvent, type ReactNode, useEffect, useRef, useState } from "react";
import { bytes } from "../../lib/format.ts";
import { errorText, message, useT } from "../../lib/i18n.ts";
import {
  type AudioDevice,
  type HardwareEncoder,
  type OnboardingChecks,
  type ReplayEstimate,
  type ReplayStorage,
  estimateReplay,
  finishOnboarding,
  listAudioDevices,
  listEncoders,
  onboardingChecks,
} from "../../lib/ipc.ts";
import { trackTitle } from "../../lib/tracks.ts";
import type { Engine } from "../../lib/useEngine.ts";
import { Mark, type View } from "../Rail.tsx";
import { ClipCard } from "../RecentClips.tsx";
import { Roster } from "../Roster.tsx";
import { CaptureSection } from "../settings/CaptureSection.tsx";
import { DiagnosticRow } from "../settings/GeneralSection.tsx";
import { HotkeysSection, Keys } from "../settings/HotkeysSection.tsx";
import { DURATIONS, durationLabel, withCurrent } from "../settings/ReplaySection.tsx";
import { Segmented, Select, SettingRow, Switch } from "../settings/Settings.tsx";
import { StorageSection } from "../settings/StorageSection.tsx";
import { LoadError, useConfigDraft } from "../settings/configDraft.tsx";

const STEPS = ["checks", "capture", "audio", "folder", "replay", "test"] as const;
type Step = (typeof STEPS)[number];

const MIN_CLIP_SECS = 3;
const SECONDARY =
  "pressable h-9 rounded-md border border-line-strong px-3.5 font-medium text-ink hover:bg-surface-hover";
const PRIMARY =
  "pressable h-9 rounded-md bg-ink px-3.5 font-semibold text-ground hover:bg-ink/90 disabled:cursor-not-allowed disabled:bg-surface-2 disabled:text-ink-3";

type CheckState = "pending" | "ok" | "warn";

function CheckRow({ state, label, detail }: { state: CheckState; label: string; detail: string }) {
  const Icon = state === "pending" ? LoaderCircle : state === "ok" ? CircleCheck : TriangleAlert;
  return (
    <li className="grid grid-cols-[20px_minmax(0,1fr)] items-start gap-x-3 border-b border-line px-5 py-3">
      <Icon
        size={20}
        aria-hidden
        className={`mt-0.5 ${state === "pending" ? "check-spin text-ink-3" : state === "ok" ? "text-ink-2" : "text-warn"}`}
      />
      <div className="min-w-0">
        <p className="text-[15px] font-semibold">{label}</p>
        <p className={`mt-0.5 max-w-[62ch] text-[13px] [overflow-wrap:anywhere] ${state === "warn" ? "text-warn" : "text-ink-2"}`}>
          {detail}
        </p>
      </div>
    </li>
  );
}

function ChecksStep({
  checks,
  checksError,
  encoders,
  clipsDir,
  onError,
}: {
  checks: OnboardingChecks | null;
  checksError: string | null;
  encoders: HardwareEncoder[] | null;
  clipsDir: string;
  onError: (message: string) => void;
}) {
  const t = useT();
  const pending = t("onboarding.checks.pending");
  const working = encoders?.filter((encoder) => !encoder.failure) ?? [];
  const codecs = [...new Set(working.map((encoder) => t(`codecs.${encoder.codec}`)))].join(" · ");
  const unknown = checksError ?? pending;
  const unknownState: CheckState = checksError ? "warn" : "pending";

  return (
    <>
      <ul aria-live="polite">
        <CheckRow
          state={!checks ? unknownState : checks.windows_supported ? "ok" : "warn"}
          label={t("onboarding.checks.windows")}
          detail={
            !checks
              ? unknown
              : checks.windows_build === null
                ? t("onboarding.checks.windowsUnknown")
                : t(checks.windows_supported ? "onboarding.checks.windowsOk" : "onboarding.checks.windowsOld", {
                    build: checks.windows_build,
                  })
          }
        />
        <CheckRow
          state={!encoders ? "pending" : working.length > 0 ? "ok" : "warn"}
          label={t("onboarding.checks.encoder")}
          detail={
            !encoders
              ? pending
              : working.length > 0
                ? t("onboarding.checks.encoderOk", { codecs })
                : t("onboarding.checks.encoderMissing")
          }
        />
        <CheckRow
          state={!checks ? unknownState : "ok"}
          label={t("onboarding.checks.webview")}
          detail={!checks ? unknown : t("onboarding.checks.webviewVersion", { version: checks.webview ?? "?" })}
        />
        <CheckRow
          state={!checks ? unknownState : checks.clips_dir_writable ? "ok" : "warn"}
          label={t("onboarding.checks.folder")}
          detail={
            !checks
              ? unknown
              : t(checks.clips_dir_writable ? "onboarding.checks.folderOk" : "onboarding.checks.folderReadOnly", {
                  path: clipsDir,
                })
          }
        />
      </ul>
      {encoders && working.length === 0 && <DiagnosticRow onError={onError} />}
    </>
  );
}

function AudioStep({ engine }: { engine: Engine }) {
  const t = useT();
  const { config, loadError } = useConfigDraft();
  const [inputs, setInputs] = useState<AudioDevice[]>([]);
  const choices = useRef<(HTMLButtonElement | null)[]>([]);

  useEffect(() => {
    listAudioDevices(true).then(setInputs, () => setInputs([]));
  }, []);

  if (!config) return <LoadError message={loadError} />;
  const { draft } = config;
  const audio = draft.audio;
  const setAudio = (patch: Partial<typeof audio>) => config.commit({ ...draft, audio: { ...audio, ...patch } });
  const mix = trackTitle(audio, "mix");
  const fallback = inputs.find((device) => device.default);
  const deviceOptions = [
    {
      value: "",
      label: fallback
        ? t("settings.audio.defaultDeviceNamed", { name: fallback.name })
        : t("settings.audio.defaultDevice"),
    },
    ...inputs.map((device) => ({ value: device.id, label: device.name })),
  ];

  const pickMix = (desktop: boolean) => {
    if (desktop !== audio.desktop_in_full_mix) setAudio({ desktop_in_full_mix: desktop });
    choices.current[desktop ? 1 : 0]?.focus();
  };
  const moveChoice = (event: KeyboardEvent<HTMLButtonElement>) => {
    if (["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown"].includes(event.key)) {
      event.preventDefault();
      pickMix(!audio.desktop_in_full_mix);
    }
  };

  return (
    <>
      <SettingRow label={t("onboarding.audio.microphone")}>
        <Switch
          checked={audio.record_microphone}
          label={t("onboarding.audio.microphone")}
          onChange={(record_microphone) => setAudio({ record_microphone })}
        />
      </SettingRow>
      {audio.record_microphone && (
        <>
          <SettingRow label={t("onboarding.audio.device")}>
            <Select
              label={t("onboarding.audio.device")}
              className="w-80 max-w-full min-w-0"
              value={audio.microphone_device_id ?? ""}
              options={deviceOptions}
              onChange={(id) => setAudio({ microphone_device_id: id || null })}
            />
          </SettingRow>
          <SettingRow
            label={t("onboarding.audio.voiceInMix")}
            help={t("onboarding.audio.voiceInMixHelp", { name: mix })}
          >
            <Switch
              checked={audio.microphone_in_full_mix}
              label={t("onboarding.audio.voiceInMix")}
              onChange={(microphone_in_full_mix) => setAudio({ microphone_in_full_mix })}
            />
          </SettingRow>
        </>
      )}
      <div className="border-b border-line px-5 py-4">
        <p id="mix-choice" className="text-[15px] font-semibold">
          {t("onboarding.audio.mixTitle", { name: mix })}
        </p>
        <div role="radiogroup" aria-labelledby="mix-choice" className="mt-3 grid grid-cols-2 gap-3">
          {[false, true].map((desktop) => {
            const checked = audio.desktop_in_full_mix === desktop;
            return (
              <button
                key={String(desktop)}
                ref={(element) => {
                  choices.current[desktop ? 1 : 0] = element;
                }}
                type="button"
                role="radio"
                aria-checked={checked}
                tabIndex={checked ? 0 : -1}
                onClick={() => pickMix(desktop)}
                onKeyDown={moveChoice}
                className={`mix-choice pressable rounded-md border bg-surface px-4 py-3 text-left ${
                  checked ? "border-ink outline outline-ink" : "border-line-strong hover:bg-surface-hover"
                }`}
              >
                <span className="block font-semibold">
                  {t(desktop ? "onboarding.audio.desktop" : "onboarding.audio.chosen")}
                </span>
                <span className="mt-1 block text-[13px] text-ink-2">
                  {t(desktop ? "onboarding.audio.desktopHelp" : "onboarding.audio.chosenHelp")}
                </span>
              </button>
            );
          })}
        </div>
      </div>
      <Roster tracks={engine.status?.tracks ?? []} appOrder={[]} flashAt={0} />
      <p className="max-w-[78ch] px-5 pb-5 text-[13px] text-ink-2">{t("onboarding.audio.apps")}</p>
    </>
  );
}

function ReplayStep() {
  const t = useT();
  const { config, loadError } = useConfigDraft();
  const [estimate, setEstimate] = useState<ReplayEstimate | null>(null);
  const draft = config?.draft ?? null;

  useEffect(() => {
    if (!draft) return;
    let current = true;
    estimateReplay(draft).then(
      (value) => current && setEstimate(value),
      () => current && setEstimate(null),
    );
    return () => {
      current = false;
    };
  }, [draft]);

  if (!config || !draft) return <LoadError message={loadError} />;
  const replay = (patch: Partial<typeof draft.replay>) => config.commit({ ...draft, replay: { ...draft.replay, ...patch } });

  const size = estimate ? bytes(estimate.bytes) : "—";

  return (
    <>
      <SettingRow label={t("settings.replay.duration")}>
        <Select
          label={t("settings.replay.duration")}
          value={draft.replay.duration_secs}
          options={withCurrent(DURATIONS, draft.replay.duration_secs).map((seconds) => ({
            value: seconds,
            label: durationLabel(seconds),
          }))}
          onChange={(duration_secs) => replay({ duration_secs })}
        />
      </SettingRow>
      <SettingRow
        label={t("onboarding.replay.storage")}
        help={t(draft.replay.storage === "ram" ? "onboarding.replay.ramHelp" : "onboarding.replay.diskHelp", { size })}
      >
        <Segmented<ReplayStorage>
          value={draft.replay.storage}
          options={[
            { value: "ram", label: t("onboarding.replay.ram") },
            { value: "disk", label: t("onboarding.replay.disk") },
          ]}
          label={t("onboarding.replay.storage")}
          onChange={(storage) => replay({ storage })}
        />
      </SettingRow>
      <SettingRow label={t("settings.general.armOnStartup")}>
        <Switch
          checked={draft.replay.arm_on_startup}
          label={t("settings.general.armOnStartup")}
          onChange={(arm_on_startup) => replay({ arm_on_startup })}
        />
      </SettingRow>
      <HotkeysSection only="save_replay" />
    </>
  );
}

function TestStep({ engine, startedAt, onGallery }: { engine: Engine; startedAt: number; onGallery: () => void }) {
  const t = useT();
  const armOnce = useRef(false);

  useEffect(() => {
    if (!armOnce.current && engine.phase === "idle") {
      armOnce.current = true;
      engine.arm();
    }
  }, [engine]);

  const savedNow = engine.savedAt > startedAt;
  const clip = savedNow ? engine.clips.find((entry) => entry.path === engine.freshClip) : undefined;
  const failure = engine.status?.alerts.find(
    (alert) => alert.code === "replay_arm_failed" || alert.code === "replay_stopped",
  );
  const buffered = engine.status?.buffered_secs ?? 0;
  const [beforeKeys, afterKeys] = t("onboarding.test.pressKeys").split("{keys}");

  let body: ReactNode;
  if (savedNow) {
    body = (
      <>
        <p className="text-[22px] font-semibold">{clip ? t("onboarding.test.saved") : t("onboarding.test.saving")}</p>
        {clip && (
          <ul className="mt-4 grid max-w-sm">
            <ClipCard clip={clip} appOrder={[]} labels={engine.config?.audio ?? null} fresh />
          </ul>
        )}
        <button type="button" onClick={onGallery} className={`${SECONDARY} mt-4`}>
          {t("onboarding.test.gallery")}
        </button>
      </>
    );
  } else if (engine.phase === "error") {
    body = (
      <>
        <p className="text-[22px] font-semibold">{t("onboarding.test.failed")}</p>
        {failure && <p className="mt-2 max-w-[78ch] text-warn">{message(failure)}</p>}
        <button type="button" onClick={engine.arm} className={`${SECONDARY} mt-4`}>
          {t("onboarding.test.retry")}
        </button>
      </>
    );
  } else if (engine.phase !== "armed" || buffered < MIN_CLIP_SECS) {
    body =
      engine.phase === "armed" ? (
        <p className="flex items-baseline gap-3 text-[22px] font-semibold text-ink-2">
          {t("onboarding.test.filling")}
          <span className="score text-[28px] leading-none text-ink">
            {t("onboarding.test.buffered", { value: Math.floor(buffered), total: MIN_CLIP_SECS })}
          </span>
        </p>
      ) : (
        <p className="text-[22px] font-semibold text-ink-2">{t("onboarding.test.arming")}</p>
      );
  } else {
    body = (
      <>
        <p className="flex flex-wrap items-center gap-x-2 gap-y-1 text-[22px] font-semibold">
          {engine.saveKeys ? (
            <>
              {beforeKeys}
              <Keys keys={engine.saveKeys} />
              {afterKeys}
            </>
          ) : (
            t("onboarding.test.noKeys")
          )}
        </p>
        <button type="button" onClick={engine.save} className={`${SECONDARY} mt-4`}>
          {t("onboarding.test.saveNow")}
        </button>
      </>
    );
  }

  return (
    <div className="px-5 py-6" aria-live="polite">
      {body}
      {engine.actionError && (
        <p role="alert" className="mt-3 max-w-[78ch] text-[13px] text-warn">
          {engine.actionError}
        </p>
      )}
    </div>
  );
}

export function Onboarding({ engine, onDone }: { engine: Engine; onDone: (view: View) => void }) {
  const t = useT();
  const [step, setStep] = useState<Step>("checks");
  const [reached, setReached] = useState(0);
  const [checks, setChecks] = useState<OnboardingChecks | null>(null);
  const [checksError, setChecksError] = useState<string | null>(null);
  const [encoders, setEncoders] = useState<HardwareEncoder[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const testStartedAt = useRef(0);
  const heading = useRef<HTMLHeadingElement>(null);
  const first = useRef(true);
  const index = STEPS.indexOf(step);
  const previous = STEPS[index - 1];
  const following = STEPS[index + 1];
  const blocked = encoders !== null && !encoders.some((encoder) => !encoder.failure);

  useEffect(() => {
    onboardingChecks().then(setChecks, (failure) => setChecksError(errorText(failure)));
    listEncoders().then(setEncoders, () => setEncoders([]));
  }, []);

  useEffect(() => {
    if (first.current) {
      first.current = false;
      return;
    }
    heading.current?.focus();
  }, [step]);

  const go = (next: Step) => {
    setError(null);
    if (next === "test" && step !== "test") testStartedAt.current = Date.now();
    setStep(next);
    setReached((current) => Math.max(current, STEPS.indexOf(next)));
  };

  const finish = (view: View) => {
    setError(null);
    finishOnboarding().then(
      () => onDone(view),
      (failure) => setError(errorText(failure)),
    );
  };

  return (
    <div className="grid h-full grid-cols-[232px_minmax(0,1fr)]">
      <nav aria-label={t("onboarding.stepsLabel")} className="flex flex-col border-r border-line bg-rail px-3 py-4">
        <div className="flex items-center gap-2.5 px-2 pb-5">
          <Mark className="size-8" />
          <p className="text-[17px] font-semibold">{t("onboarding.title")}</p>
        </div>
        <ol className="flex flex-col gap-0.5">
          {STEPS.map((id, position) => {
            const current = id === step;
            const done = position < reached && !current;
            const open = position <= reached;
            return (
              <li key={id}>
                <button
                  type="button"
                  disabled={!open}
                  onClick={() => go(id)}
                  aria-current={current ? "step" : undefined}
                  className={`pressable flex w-full items-center gap-3 rounded-md px-2 py-2 text-left text-[14px] ${
                    current
                      ? "bg-surface-2 font-semibold text-ink"
                      : open
                        ? "text-ink-2 hover:bg-surface-hover hover:text-ink"
                        : "cursor-default text-ink-3"
                  }`}
                >
                  <span
                    className={`score flex h-6 w-8 shrink-0 items-center justify-center rounded-[4px] text-[14px] font-semibold ${
                      current ? "bg-ink text-ground" : "bg-surface-2"
                    }`}
                  >
                    {done ? <Check size={15} strokeWidth={2.4} aria-hidden /> : String(position + 1).padStart(2, "0")}
                  </span>
                  <span className="min-w-0 flex-1">
                    {t(`onboarding.steps.${id}`)}
                    {done && <span className="sr-only">, {t("onboarding.done")}</span>}
                  </span>
                </button>
              </li>
            );
          })}
        </ol>
        <button type="button" onClick={() => finish("status")} className={`${SECONDARY} mx-2 mt-auto`}>
          {t("onboarding.skip")}
        </button>
      </nav>
      <main className="flex min-h-0 min-w-0 flex-col">
        <div key={step} className="onboarding-step min-h-0 flex-1 overflow-y-auto">
          <h1
            ref={heading}
            tabIndex={-1}
            className="border-b border-line px-5 pt-6 pb-4 text-[22px] font-semibold outline-none"
          >
            {t(`onboarding.steps.${step}`)}
          </h1>
          {step === "checks" && (
            <ChecksStep
              checks={checks}
              checksError={checksError}
              encoders={encoders}
              clipsDir={engine.storage?.clips_dir ?? "—"}
              onError={setError}
            />
          )}
          {step === "capture" && (
            <>
              <CaptureSection screensOnly />
              <p className="max-w-[78ch] px-5 pb-5 text-[13px] text-ink-2">{t("onboarding.capture.help")}</p>
            </>
          )}
          {step === "audio" && <AudioStep engine={engine} />}
          {step === "folder" && <StorageSection folderOnly />}
          {step === "replay" && <ReplayStep />}
          {step === "test" && (
            <TestStep engine={engine} startedAt={testStartedAt.current} onGallery={() => finish("gallery")} />
          )}
        </div>
        {error && (
          <p role="alert" className="border-t border-line bg-surface-2 px-5 py-3 text-[13px] text-warn">
            {error}
          </p>
        )}
        <footer className="flex justify-end gap-2 border-t border-line bg-surface px-5 py-3">
          {previous && (
            <button type="button" onClick={() => go(previous)} className={SECONDARY}>
              {t("onboarding.back")}
            </button>
          )}
          {following ? (
            <button
              type="button"
              disabled={step === "checks" && (encoders === null || blocked)}
              onClick={() => go(following)}
              className={PRIMARY}
            >
              {t("onboarding.next")}
            </button>
          ) : (
            <button type="button" onClick={() => finish("status")} className={PRIMARY}>
              {t("onboarding.finish")}
            </button>
          )}
        </footer>
      </main>
    </div>
  );
}
