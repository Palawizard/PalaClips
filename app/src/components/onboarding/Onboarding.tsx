import { CircleCheck, LoaderCircle, TriangleAlert } from "lucide-react";
import { type KeyboardEvent, type ReactNode, useEffect, useRef, useState } from "react";
import { bytes } from "../../lib/format.ts";
import { errorText, message, useT } from "../../lib/i18n.ts";
import {
  type AudioDevice,
  type EngineConfig,
  type EncoderInfo,
  type OnboardingChecks,
  type TargetInfo,
  estimateReplay,
  finishOnboarding,
  encodesOnGpu,
  getHotkeys,
  listAudioDevices,
  listCaptureTargets,
  listEncoders,
  onboardingChecks,
  pickClipsDir,
  setHotkeys,
} from "../../lib/ipc.ts";
import { trackTitle } from "../../lib/tracks.ts";
import type { Engine } from "../../lib/useEngine.ts";
import { Mark, type View } from "../Rail.tsx";
import { ClipCard } from "../RecentClips.tsx";
import { CaptureSection } from "../settings/CaptureSection.tsx";
import { DiagnosticRow } from "../settings/GeneralSection.tsx";
import { HotkeysSection, Keys } from "../settings/HotkeysSection.tsx";
import { cpuVideo, durationLabel } from "../settings/ReplaySection.tsx";
import { Select } from "../settings/Settings.tsx";
import { LoadError, useConfigDraft } from "../settings/configDraft.tsx";

type Question = "screen" | "mic" | "voice" | "mix" | "duration" | "storage" | "folder" | "hotkey";
type Screen = "loading" | "checks" | Question | "summary" | "test";

const DURATION_CHOICES = [30, 60, 120, 300];
const DEFAULT_DURATION_SECS = 60;
const DISK_ADVISED_AFTER_SECS = 120;
const DEFAULT_SAVE_KEYS = "Alt+F10";
const PICK_HOLD_MS = 150;
const MIN_CLIP_SECS = 3;
const SECONDARY =
  "pressable h-10 rounded-md border border-line-strong px-4 font-medium text-ink hover:bg-surface-hover";
const PRIMARY =
  "pressable h-10 rounded-md bg-ink px-4 font-semibold text-ground hover:bg-ink/90 disabled:cursor-not-allowed disabled:bg-surface-2 disabled:text-ink-3";

function questionsFor(config: EngineConfig): Question[] {
  return [
    "screen",
    "mic",
    ...(config.audio.record_microphone ? (["voice"] as const) : []),
    "mix",
    "duration",
    "storage",
    "folder",
    "hotkey",
  ];
}

function reducedMotion(): boolean {
  return window.matchMedia("(prefers-reduced-motion: reduce)").matches;
}

interface Choice {
  id: string;
  label: ReactNode;
  detail?: ReactNode;
  recommended?: boolean;
  open?: boolean;
}

function ChoiceCards({ label, choices, onPick }: { label: string; choices: Choice[]; onPick: (id: string) => void }) {
  const t = useT();
  const [picked, setPicked] = useState<string | null>(null);
  const cards = useRef<(HTMLButtonElement | null)[]>([]);
  const focusable = Math.max(
    0,
    choices.findIndex((choice) => choice.recommended),
  );

  useEffect(() => {
    cards.current[focusable]?.focus();
  }, []);

  const pick = (id: string) => {
    if (picked) return;
    setPicked(id);
    window.setTimeout(() => {
      setPicked(null);
      onPick(id);
    }, reducedMotion() ? 0 : PICK_HOLD_MS);
  };

  const move = (event: KeyboardEvent<HTMLButtonElement>, index: number) => {
    const step = { ArrowDown: 1, ArrowRight: 1, ArrowUp: -1, ArrowLeft: -1 }[event.key];
    if (!step) return;
    event.preventDefault();
    cards.current[(index + step + choices.length) % choices.length]?.focus();
  };

  return (
    <div role="group" aria-label={label} className="grid grid-cols-2 gap-3 max-[720px]:grid-cols-1">
      {choices.map((choice, index) => {
        const active = picked ? picked === choice.id : Boolean(choice.open);
        return (
          <button
            key={choice.id}
            ref={(element) => {
              cards.current[index] = element;
            }}
            type="button"
            aria-expanded={choice.open}
            tabIndex={index === focusable ? 0 : -1}
            data-picked={picked === choice.id}
            onClick={() => pick(choice.id)}
            onKeyDown={(event) => move(event, index)}
            className={`choice-card pressable flex min-h-24 flex-col items-start gap-1.5 rounded-md border bg-surface px-5 py-4 text-left ${
              active ? "border-ink" : "border-line-strong hover:border-ink-3 hover:bg-surface-hover"
            }`}
          >
            <span className="flex w-full items-baseline justify-between gap-3">
              <span className="text-[17px] font-semibold">{choice.label}</span>
              {choice.recommended && (
                <span className="shrink-0 rounded-[3px] border border-line-strong px-1.5 text-[12px] text-ink-2">
                  {t("onboarding.recommended")}
                </span>
              )}
            </span>
            {choice.detail && <span className="text-[13px] text-ink-2 [overflow-wrap:anywhere]">{choice.detail}</span>}
          </button>
        );
      })}
    </div>
  );
}

function CheckRow({ state, label, detail }: { state: "pending" | "ok" | "warn"; label: string; detail: string }) {
  const Icon = state === "pending" ? LoaderCircle : state === "ok" ? CircleCheck : TriangleAlert;
  return (
    <li className="grid grid-cols-[20px_minmax(0,1fr)] items-start gap-x-3 border-b border-line py-3">
      <Icon
        size={20}
        aria-hidden
        className={`mt-0.5 ${state === "pending" ? "check-spin text-ink-3" : state === "ok" ? "text-ink-2" : "text-warn"}`}
      />
      <div className="min-w-0">
        <p className="text-[15px] font-semibold">{label}</p>
        <p className={`mt-0.5 text-[13px] [overflow-wrap:anywhere] ${state === "warn" ? "text-warn" : "text-ink-2"}`}>
          {detail}
        </p>
      </div>
    </li>
  );
}

function Checks({
  checks,
  checksError,
  encoders,
  clipsDir,
  onError,
}: {
  checks: OnboardingChecks | null;
  checksError: string | null;
  encoders: EncoderInfo[];
  clipsDir: string;
  onError: (message: string) => void;
}) {
  const t = useT();
  const working = encoders.filter((encoder) => !encoder.failure);
  const onGpu = encodesOnGpu(encoders, "auto");
  const codecs = [
    ...new Set(working.filter((encoder) => encoder.hardware === onGpu).map((encoder) => t(`codecs.${encoder.codec}`))),
  ].join(" · ");
  const unknown = checksError ?? t("onboarding.checks.pending");
  return (
    <>
      <ul aria-live="polite" className="border-t border-line">
        <CheckRow
          state={!checks ? "warn" : checks.windows_supported ? "ok" : "warn"}
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
          state={working.length > 0 && onGpu ? "ok" : "warn"}
          label={t("onboarding.checks.encoder")}
          detail={
            working.length === 0
              ? t("onboarding.checks.encoderMissing")
              : t(onGpu ? "onboarding.checks.encoderOk" : "onboarding.checks.encoderCpu", { codecs })
          }
        />
        <CheckRow
          state={!checks ? "warn" : "ok"}
          label={t("onboarding.checks.webview")}
          detail={!checks ? unknown : t("onboarding.checks.webviewVersion", { version: checks.webview ?? "?" })}
        />
        <CheckRow
          state={!checks ? "warn" : checks.clips_dir_writable ? "ok" : "warn"}
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
      {working.length === 0 && (
        <div className="mt-4 overflow-hidden rounded-md border border-line">
          <DiagnosticRow onError={onError} />
        </div>
      )}
    </>
  );
}

function TestClip({ engine, saveKeys, startedAt, onGallery }: { engine: Engine; saveKeys: string | null; startedAt: number; onGallery: () => void }) {
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
        {failure && <p className="mt-2 text-warn">{message(failure)}</p>}
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
          {saveKeys ? (
            <>
              {beforeKeys}
              <Keys keys={saveKeys} />
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
    <div aria-live="polite">
      {body}
      {engine.actionError && (
        <p role="alert" className="mt-3 text-[13px] text-warn">
          {engine.actionError}
        </p>
      )}
    </div>
  );
}

export function Onboarding({ engine, onDone }: { engine: Engine; onDone: (view: View) => void }) {
  const t = useT();
  const { config, loadError } = useConfigDraft();
  const [screen, setScreen] = useState<Screen>("loading");
  const [direction, setDirection] = useState<"forward" | "back">("forward");
  const [editing, setEditing] = useState(false);
  const [checks, setChecks] = useState<OnboardingChecks | null>(null);
  const [checksError, setChecksError] = useState<string | null>(null);
  const [encoders, setEncoders] = useState<EncoderInfo[] | null>(null);
  const [inputs, setInputs] = useState<AudioDevice[]>([]);
  const [targets, setTargets] = useState<TargetInfo[]>([]);
  const [estimates, setEstimates] = useState<Record<number, number>>({});
  const [storageSize, setStorageSize] = useState<number | null>(null);
  const [saveKeys, setSaveKeys] = useState<string | null>(null);
  const [customKeys, setCustomKeys] = useState(false);
  const [chooseMic, setChooseMic] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const testStartedAt = useRef(0);
  const heading = useRef<HTMLHeadingElement>(null);
  const draft = config?.draft ?? null;
  const main = useRef<HTMLElement>(null);

  const refreshKeys = () =>
    getHotkeys().then(
      (view) => setSaveKeys(view.hotkeys.save_replay),
      () => setSaveKeys(null),
    );

  useEffect(() => {
    onboardingChecks().then(setChecks, (failure) => setChecksError(errorText(failure)));
    listEncoders().then(setEncoders, () => setEncoders([]));
    listAudioDevices(true).then(setInputs, () => setInputs([]));
    listCaptureTargets().then(setTargets, () => setTargets([]));
    refreshKeys();
  }, []);

  useEffect(() => {
    if (screen !== "loading" || encoders === null || (checks === null && checksError === null)) return;
    const blocked = !encoders.some((encoder) => !encoder.failure);
    const trouble = blocked || !encodesOnGpu(encoders, "auto") || checksError !== null || !checks?.windows_supported || !checks?.clips_dir_writable;
    setScreen(trouble ? "checks" : "screen");
  }, [screen, encoders, checks, checksError]);

  useEffect(() => {
    if (screen === "duration" && draft) {
      for (const seconds of DURATION_CHOICES) {
        estimateReplay({ ...draft, replay: { ...draft.replay, duration_secs: seconds } }).then(
          (estimate) => setEstimates((current) => ({ ...current, [seconds]: estimate.bytes })),
          () => undefined,
        );
      }
    }
    if (screen === "storage" && draft) {
      estimateReplay(draft).then(
        (estimate) => setStorageSize(estimate.bytes),
        () => setStorageSize(null),
      );
    }
    if (screen !== "loading" && !main.current?.contains(document.activeElement)) heading.current?.focus({ preventScroll: true });
  }, [screen]);

  if (!config || !draft) {
    return (
      <div className="grid h-full place-items-center">
        {loadError ? <LoadError message={loadError} /> : <LoaderCircle aria-hidden className="check-spin text-ink-3" />}
      </div>
    );
  }

  const questions = questionsFor(draft);
  const clipsDir = draft.storage.clips_dir ?? engine.storage?.clips_dir ?? "—";
  const shownError = error ?? config.error;
  const position = questions.indexOf(screen as Question);
  const audio = draft.audio;
  const mix = trackTitle(audio, "mix");
  const fallbackInput = inputs.find((device) => device.default);
  const micName =
    inputs.find((device) => device.id === audio.microphone_device_id)?.name ?? fallbackInput?.name ?? t("settings.audio.defaultDevice");

  const go = (next: Screen, back = false) => {
    setError(null);
    setChooseMic(false);
    setCustomKeys(false);
    setDirection(back ? "back" : "forward");
    if (next === "test" && screen !== "test") testStartedAt.current = Date.now();
    setScreen(next);
  };

  const advance = (next: EngineConfig = draft) => {
    if (editing) {
      setEditing(false);
      go("summary");
      return;
    }
    const flow = questionsFor(next);
    const index = flow.indexOf(screen as Question);
    go(flow[index + 1] ?? "summary");
  };

  const goBack = () => {
    if (editing) {
      setEditing(false);
      go("summary", true);
      return;
    }
    if (screen === "summary") return go(questions[questions.length - 1] ?? "screen", true);
    if (screen === "test") return go("summary", true);
    const previous = questions[position - 1];
    if (previous) go(previous, true);
  };

  const commit = (next: EngineConfig) => {
    config.commit(next);
    return next;
  };
  const setAudio = (patch: Partial<EngineConfig["audio"]>) => commit({ ...draft, audio: { ...audio, ...patch } });
  const setReplay = (patch: Partial<EngineConfig["replay"]>) => commit({ ...draft, replay: { ...draft.replay, ...patch } });

  const edit = (question: Question) => {
    setEditing(true);
    go(question, true);
  };

  const finish = (view: View) => {
    setError(null);
    finishOnboarding().then(
      () => onDone(view),
      (failure) => setError(errorText(failure)),
    );
  };

  const saveDefaultKeys = () =>
    getHotkeys()
      .then((view) => setHotkeys({ ...view.hotkeys, save_replay: DEFAULT_SAVE_KEYS }))
      .then(getHotkeys)
      .then((view) => {
        setSaveKeys(view.hotkeys.save_replay);
        if (view.unavailable.includes("save_replay")) {
          setCustomKeys(true);
          setError(t("onboarding.hotkey.taken", { keys: DEFAULT_SAVE_KEYS }));
        } else advance();
      })
      .catch((failure) => setError(errorText(failure)));

  const screenName = (() => {
    const monitors = targets.filter((target): target is Extract<TargetInfo, { kind: "monitor" }> => target.kind === "monitor");
    const capture = draft.capture;
    if (capture.kind === "monitor") {
      const found = monitors.find((monitor) =>
        capture.monitor_path ? monitor.monitor_path === capture.monitor_path : monitor.device_name === capture.device_name,
      );
      return found?.name ?? capture.device_name;
    }
    if (capture.kind === "window") return capture.exe_name;
    const primary = monitors.find((monitor) => monitor.primary);
    return primary ? t("onboarding.summary.primaryNamed", { name: primary.name }) : t("onboarding.summary.primary");
  })();

  const yesNo = (value: boolean) => (value ? t("onboarding.summary.yes") : t("onboarding.summary.no"));
  const summaryRows: { question: Question; label: string; value: ReactNode }[] = [
    { question: "screen", label: t("onboarding.summary.screen"), value: screenName },
    {
      question: "mic",
      label: t("onboarding.summary.mic"),
      value: audio.record_microphone ? micName : t("onboarding.summary.no"),
    },
    ...(audio.record_microphone
      ? [{ question: "voice" as const, label: t("onboarding.summary.voice", { name: mix }), value: yesNo(audio.microphone_in_full_mix) }]
      : []),
    {
      question: "mix",
      label: t("onboarding.summary.mix", { name: mix }),
      value: audio.desktop_in_full_mix ? t("onboarding.mix.desktop") : t("onboarding.mix.chosen"),
    },
    { question: "duration", label: t("onboarding.summary.duration"), value: durationLabel(draft.replay.duration_secs) },
    {
      question: "storage",
      label: t("onboarding.summary.storage"),
      value: draft.replay.storage === "ram" ? t("onboarding.storage.ram") : t("onboarding.storage.disk"),
    },
    { question: "folder", label: t("onboarding.summary.folder"), value: clipsDir },
    {
      question: "hotkey",
      label: t("onboarding.summary.hotkey"),
      value: saveKeys ? <Keys keys={saveKeys} /> : t("settings.hotkeys.none"),
    },
  ];

  let title = "";
  let hint: ReactNode = null;
  let body: ReactNode = null;
  let footer: ReactNode = null;

  switch (screen) {
    case "loading":
      body = <LoaderCircle aria-hidden className="check-spin mx-auto text-ink-3" />;
      break;
    case "checks": {
      const blocked = encoders !== null && !encoders.some((encoder) => !encoder.failure);
      title = t("onboarding.checks.title");
      body = (
        <Checks
          checks={checks}
          checksError={checksError}
          encoders={encoders ?? []}
          clipsDir={clipsDir}
          onError={setError}
        />
      );
      const continueOnCpu = () =>
        estimateReplay(draft)
          .then(
            (estimate) => estimate.source_height,
            () => Number.POSITIVE_INFINITY,
          )
          .then((sourceHeight) => {
            commit({ ...draft, video: cpuVideo(draft.video, sourceHeight, encoders ?? []) });
            go("screen");
          });
      footer = !blocked && (
        <button
          type="button"
          onClick={encodesOnGpu(encoders ?? [], "auto") ? () => go("screen") : continueOnCpu}
          className={PRIMARY}
        >
          {t("onboarding.checks.continueAnyway")}
        </button>
      );
      break;
    }
    case "screen":
      title = t("onboarding.screen.question");
      hint = t("onboarding.screen.hint");
      body = (
        <div className="-mx-5">
          <CaptureSection screensOnly />
        </div>
      );
      footer = (
        <button
          type="button"
          onClick={() => advance()}
          className={PRIMARY}
        >
          {t("onboarding.next")}
        </button>
      );
      break;
    case "mic":
      title = t("onboarding.mic.question");
      body = (
        <>
          <ChoiceCards
            label={title}
            choices={[
              { id: "yes", label: t("onboarding.mic.yes"), detail: t("onboarding.mic.yesDetail", { device: micName }), recommended: true, open: chooseMic },
              { id: "no", label: t("onboarding.mic.no"), detail: t("onboarding.mic.noDetail") },
            ]}
            onPick={(id) => {
              const next = setAudio({ record_microphone: id === "yes" });
              if (id === "yes" && inputs.length > 1) setChooseMic(true);
              else advance(next);
            }}
          />
          {chooseMic && (
            <div className="choice-extra mt-5 flex flex-wrap items-center gap-3">
              <span className="text-[15px] font-semibold">{t("onboarding.mic.device")}</span>
              <Select
                label={t("onboarding.mic.device")}
                className="w-80 max-w-full min-w-0"
                value={audio.microphone_device_id ?? ""}
                options={[
                  {
                    value: "",
                    label: fallbackInput
                      ? t("settings.audio.defaultDeviceNamed", { name: fallbackInput.name })
                      : t("settings.audio.defaultDevice"),
                  },
                  ...inputs.map((device) => ({ value: device.id, label: device.name })),
                ]}
                onChange={(id) => setAudio({ microphone_device_id: id || null })}
              />
            </div>
          )}
        </>
      );
      footer = chooseMic && (
        <button type="button" onClick={() => advance()} className={PRIMARY}>
          {t("onboarding.next")}
        </button>
      );
      break;
    case "voice":
      title = t("onboarding.voice.question", { name: mix });
      body = (
        <ChoiceCards
          label={title}
          choices={[
            { id: "yes", label: t("onboarding.voice.yes"), detail: t("onboarding.voice.yesDetail"), recommended: true },
            { id: "no", label: t("onboarding.voice.no"), detail: t("onboarding.voice.noDetail") },
          ]}
          onPick={(id) => advance(setAudio({ microphone_in_full_mix: id === "yes" }))}
        />
      );
      break;
    case "mix":
      title = t("onboarding.mix.question", { name: mix });
      body = (
        <ChoiceCards
          label={title}
          choices={[
            { id: "chosen", label: t("onboarding.mix.chosen"), detail: t("onboarding.mix.chosenDetail"), recommended: true },
            { id: "desktop", label: t("onboarding.mix.desktop"), detail: t("onboarding.mix.desktopDetail") },
          ]}
          onPick={(id) => advance(setAudio({ desktop_in_full_mix: id === "desktop" }))}
        />
      );
      break;
    case "duration":
      title = t("onboarding.duration.question");
      body = (
        <ChoiceCards
          label={title}
          choices={DURATION_CHOICES.map((seconds) => ({
            id: String(seconds),
            label: <span className="score text-[22px]">{durationLabel(seconds)}</span>,
            detail:
              estimates[seconds] === undefined
                ? "—"
                : t("onboarding.duration.detail", { size: bytes(estimates[seconds]) }),
            recommended: seconds === DEFAULT_DURATION_SECS,
          }))}
          onPick={(id) => {
            const seconds = Number(id);
            advance(
              setReplay({
                duration_secs: seconds,
                storage: seconds > DISK_ADVISED_AFTER_SECS && draft.replay.storage === "ram" ? "disk" : draft.replay.storage,
              }),
            );
          }}
        />
      );
      break;
    case "storage": {
      const size = storageSize === null ? "—" : bytes(storageSize);
      const diskAdvised = draft.replay.duration_secs > DISK_ADVISED_AFTER_SECS;
      title = t("onboarding.storage.question");
      body = (
        <ChoiceCards
          label={title}
          choices={[
            { id: "ram", label: t("onboarding.storage.ram"), detail: t("onboarding.storage.ramDetail", { size }), recommended: !diskAdvised },
            { id: "disk", label: t("onboarding.storage.disk"), detail: t("onboarding.storage.diskDetail", { size }), recommended: diskAdvised },
          ]}
          onPick={(id) => advance(setReplay({ storage: id === "disk" ? "disk" : "ram" }))}
        />
      );
      break;
    }
    case "folder":
      title = t("onboarding.folder.question");
      body = (
        <ChoiceCards
          label={title}
          choices={[
            { id: "keep", label: t("onboarding.folder.keep"), detail: clipsDir, recommended: true },
            { id: "pick", label: t("onboarding.folder.pick"), detail: t("onboarding.folder.pickDetail") },
          ]}
          onPick={(id) => {
            if (id === "keep") return advance();
            pickClipsDir()
              .then((dir) => {
                if (dir) advance(commit({ ...draft, storage: { ...draft.storage, clips_dir: dir } }));
                else go("folder");
              })
              .catch((failure) => setError(errorText(failure)));
          }}
        />
      );
      break;
    case "hotkey":
      title = t("onboarding.hotkey.question");
      body = (
        <>
          <ChoiceCards
            label={title}
            choices={[
              {
                id: "default",
                label: <Keys keys={DEFAULT_SAVE_KEYS} />,
                detail: t("onboarding.hotkey.defaultDetail"),
                recommended: true,
              },
              {
                id: "custom",
                label: t("onboarding.hotkey.custom"),
                detail: t("onboarding.hotkey.customDetail"),
                open: customKeys,
              },
            ]}
            onPick={(id) => {
              if (id === "default") saveDefaultKeys();
              else setCustomKeys(true);
            }}
          />
          {customKeys && (
            <div className="choice-extra -mx-5 mt-5 border-t border-line">
              <HotkeysSection only="save_replay" />
            </div>
          )}
        </>
      );
      footer = customKeys && (
        <button
          type="button"
          onClick={() => {
            refreshKeys();
            advance();
          }}
          className={PRIMARY}
        >
          {t("onboarding.next")}
        </button>
      );
      break;
    case "summary":
      title = t("onboarding.summary.title");
      body = (
        <dl className="border-t border-line">
          {summaryRows.map((row) => (
            <div key={row.question} className="grid grid-cols-[minmax(0,14rem)_minmax(0,1fr)_auto] items-center gap-x-5 border-b border-line py-3">
              <dt className="text-[13px] text-ink-2">{row.label}</dt>
              <dd className="min-w-0 font-semibold [overflow-wrap:anywhere]">{row.value}</dd>
              <dd>
                <button
                  type="button"
                  onClick={() => edit(row.question)}
                  aria-label={t("onboarding.summary.editLabel", { name: row.label })}
                  className="pressable h-8 rounded-md px-2.5 text-[13px] font-medium text-ink-2 hover:bg-surface-hover hover:text-ink"
                >
                  {t("onboarding.summary.edit")}
                </button>
              </dd>
            </div>
          ))}
        </dl>
      );
      footer = (
        <>
          <button type="button" onClick={() => finish("capture")} className={SECONDARY}>
            {t("onboarding.summary.manual")}
          </button>
          <button type="button" onClick={() => go("test")} className={PRIMARY}>
            {t("onboarding.summary.confirm")}
          </button>
        </>
      );
      break;
    case "test":
      title = t("onboarding.test.title");
      body = (
        <TestClip engine={engine} saveKeys={saveKeys} startedAt={testStartedAt.current} onGallery={() => finish("gallery")} />
      );
      footer = (
        <button type="button" onClick={() => finish("status")} className={PRIMARY}>
          {t("onboarding.finish")}
        </button>
      );
      break;
  }

  const canGoBack = editing || screen === "summary" || screen === "test" || position > 0;

  return (
    <div
      className="flex h-full flex-col"
      onKeyDown={(event) => {
        if (event.key === "Escape" && canGoBack && !event.defaultPrevented) goBack();
      }}
    >
      <header className="flex items-center gap-4 border-b border-line px-5 py-3">
        <Mark className="size-7" />
        <p className="text-[15px] font-semibold">{t("onboarding.title")}</p>
        {position >= 0 && (
          <p className="score ml-auto text-[14px] text-ink-2" aria-live="polite">
            {t("onboarding.progress", { step: position + 1, total: questions.length })}
          </p>
        )}
        <button
          type="button"
          onClick={() => finish("status")}
          className={`pressable h-9 rounded-md px-3 text-[13px] font-medium text-ink-2 hover:bg-surface-hover hover:text-ink ${position >= 0 ? "" : "ml-auto"}`}
        >
          {t("onboarding.skip")}
        </button>
      </header>
      <main ref={main} className="min-h-0 flex-1 overflow-y-auto">
        <div key={screen} data-direction={direction} className="onboarding-question mx-auto w-full max-w-[760px] px-5 pt-14 pb-10">
          {title && (
            <h1 ref={heading} tabIndex={-1} className="text-[28px] leading-tight font-semibold outline-none">
              {title}
            </h1>
          )}
          {hint && <p className="mt-2 text-[14px] text-ink-2">{hint}</p>}
          <div className="mt-8">{body}</div>
        </div>
      </main>
      {shownError && (
        <p role="alert" className="border-t border-line bg-surface-2 px-5 py-3 text-[13px] text-warn">
          {shownError}
        </p>
      )}
      <footer className="flex items-center gap-2 border-t border-line px-5 py-3">
        {canGoBack && (
          <button type="button" onClick={goBack} className={SECONDARY}>
            {editing ? t("onboarding.backToSummary") : t("onboarding.back")}
          </button>
        )}
        <span className="ml-auto flex gap-2">{footer}</span>
      </footer>
    </div>
  );
}
