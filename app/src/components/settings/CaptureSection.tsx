import { AppWindow, Monitor } from "lucide-react";
import { type KeyboardEvent, useCallback, useEffect, useRef, useState } from "react";
import {
  type CaptureTarget,
  type EngineConfig,
  type TargetInfo,
  listCaptureTargets,
  startPreview,
  stopPreview,
  stopPreviews,
} from "../../lib/ipc.ts";
import { iconUrl } from "../../lib/appIcons.ts";
import { errorText, t, useT } from "../../lib/i18n.ts";
import { SettingRow, Switch } from "./Settings.tsx";
import { ConfigFooter, LoadError, useConfigDraft } from "./configDraft.tsx";

const LIVE_WINDOWS = 6;

interface Card {
  key: string;
  target: CaptureTarget;
  kind: "monitor" | "window";
  name: string;
  detail: string;
  processId: number | null;
  exe: string | null;
  state: "live" | "minimized" | "closed";
}

function exeStem(exe: string): string {
  return exe.replace(/\.exe$/i, "");
}

function gameName(config: EngineConfig, exe: string): string {
  return config.storage.game_names[exe.toLowerCase()] ?? exeStem(exe);
}

function toCards(targets: TargetInfo[], config: EngineConfig): Card[] {
  return targets.map((target) =>
    target.kind === "monitor"
      ? {
          key: `monitor:${target.device_name}`,
          target: { kind: "monitor", device_name: target.device_name },
          kind: "monitor",
          name: target.name,
          detail: [
            `${target.width}×${target.height}`,
            t("units.hz", { value: target.refresh_rate }),
            ...(target.primary ? [t("settings.capture.primary")] : []),
          ].join(" · "),
          processId: null,
          exe: null,
          state: "live",
        }
      : {
          key: `window:${target.process_id}:${target.title}`,
          target: { kind: "window", exe_name: target.exe_name, title_hint: target.title },
          kind: "window",
          name: config.storage.game_names[target.exe_name.toLowerCase()] ?? target.app_name,
          detail: target.title,
          processId: target.process_id,
          exe: target.exe_name,
          state: target.minimized ? "minimized" : "live",
        },
  );
}

function selectedKey(cards: Card[], capture: CaptureTarget, targets: TargetInfo[]): string | null {
  if (capture.kind === "primary_monitor") {
    const primary = targets.find((target) => target.kind === "monitor" && target.primary);
    return primary?.kind === "monitor" ? `monitor:${primary.device_name}` : null;
  }
  if (capture.kind === "monitor") {
    const wanted = capture.device_name.toLowerCase();
    return cards.find((card) => card.target.kind === "monitor" && card.target.device_name.toLowerCase() === wanted)?.key ?? null;
  }
  const exe = capture.exe_name.toLowerCase();
  const windows = cards.filter((card) => card.exe?.toLowerCase() === exe);
  const hint = capture.title_hint;
  return (
    (hint ? windows.find((card) => card.detail.includes(hint)) : undefined) ??
    windows.find((card) => card.detail !== "") ??
    windows[0]
  )?.key ?? null;
}

function moveFocus(event: KeyboardEvent<HTMLElement>) {
  if (!["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown"].includes(event.key)) return;
  const cards = [...event.currentTarget.querySelectorAll<HTMLElement>("[data-card]")];
  const index = cards.indexOf(document.activeElement as HTMLElement);
  const current = cards[index];
  if (!current) return;
  event.preventDefault();
  let next: HTMLElement | undefined;
  if (event.key === "ArrowLeft") next = cards[index - 1];
  else if (event.key === "ArrowRight") next = cards[index + 1];
  else {
    const from = current.getBoundingClientRect();
    const down = event.key === "ArrowDown";
    const rows = cards
      .map((card) => ({ card, rect: card.getBoundingClientRect() }))
      .filter(({ rect }) => (down ? rect.top > from.top + 1 : rect.top < from.top - 1));
    const rowTop = down
      ? Math.min(...rows.map(({ rect }) => rect.top))
      : Math.max(...rows.map(({ rect }) => rect.top));
    const center = from.left + from.width / 2;
    next = rows
      .filter(({ rect }) => Math.abs(rect.top - rowTop) < 1)
      .sort(
        (a, b) =>
          Math.abs(a.rect.left + a.rect.width / 2 - center) - Math.abs(b.rect.left + b.rect.width / 2 - center),
      )[0]?.card;
  }
  next?.focus();
  next?.scrollIntoView({ block: "nearest" });
}

function initialLive(cards: Card[], previous: string[]): string[] {
  const monitors = cards.filter((card) => card.kind === "monitor").map((card) => card.key);
  const liveWindows = new Set(
    cards.filter((card) => card.kind === "window" && card.state === "live").map((card) => card.key),
  );
  const windows = previous.filter((key) => liveWindows.has(key));
  for (const card of cards) {
    if (windows.length >= LIVE_WINDOWS) break;
    if (card.kind === "window" && card.state === "live" && !windows.includes(card.key)) windows.push(card.key);
  }
  return [...monitors, ...windows];
}

function CaptureCard({
  card,
  selected,
  armed,
  focusable,
  frame,
  live,
  failed,
  onSelect,
  onPreview,
  onFocus,
}: {
  card: Card;
  selected: boolean;
  armed: boolean;
  focusable: boolean;
  frame: string | undefined;
  live: boolean;
  failed: boolean;
  onSelect: () => void;
  onPreview: () => void;
  onFocus: () => void;
}) {
  const t = useT();
  const [icon, setIcon] = useState<string | null>(null);
  useEffect(() => {
    if (!card.exe || card.processId === null) return;
    let current = true;
    iconUrl(card.exe, card.processId).then((url) => current && setIcon(url));
    return () => {
      current = false;
    };
  }, [card.exe, card.processId]);

  const glyph = icon ? (
    <img src={icon} alt="" className="size-5 shrink-0" />
  ) : card.kind === "monitor" ? (
    <Monitor aria-hidden className="size-5 shrink-0 text-ink-2" strokeWidth={1.75} />
  ) : (
    <AppWindow aria-hidden className="size-5 shrink-0 text-ink-2" strokeWidth={1.75} />
  );
  const placeholder =
    card.state === "closed"
      ? t("settings.capture.notLaunched")
      : card.state === "minimized"
        ? t("settings.capture.minimized")
        : failed
          ? t("settings.capture.previewUnavailable")
          : null;

  return (
    <li
      className={`capture-card relative flex flex-col overflow-hidden rounded-md border bg-surface ${
        selected ? "border-ink outline outline-ink" : "border-line-strong hover:border-ink-3 hover:bg-surface-hover"
      }`}
    >
      <div className="relative aspect-video overflow-hidden bg-meter-ground">
        {frame && live && card.state === "live" ? (
          <img src={frame} alt="" className="capture-frame size-full object-contain" />
        ) : (
          <div className="flex size-full flex-col items-center justify-center gap-2 text-[13px] text-ink-2">
            {icon ? (
              <img src={icon} alt="" className="size-10" />
            ) : card.kind === "monitor" ? (
              <Monitor aria-hidden className="size-10 text-ink-3" strokeWidth={1.5} />
            ) : (
              <AppWindow aria-hidden className="size-10 text-ink-3" strokeWidth={1.5} />
            )}
            {placeholder}
          </div>
        )}
        {armed && (
          <span className="absolute top-2 left-2 flex items-center gap-1.5 rounded bg-ground/85 px-1.5 py-0.5 text-[12px] font-medium text-ink">
            <span aria-hidden className="size-2 rounded-full bg-rec" />
            {t("settings.capture.capturing")}
          </span>
        )}
      </div>
      <div className="flex min-w-0 items-center gap-2 px-3 py-2">
        {glyph}
        <span className="min-w-0 flex-1">
          <span className="block truncate text-[14px] font-semibold">{card.name}</span>
          <span className="block truncate text-[12px] text-ink-2">
            {card.detail}
          </span>
        </span>
      </div>
      <button
        type="button"
        data-card
        tabIndex={focusable ? 0 : -1}
        onFocus={onFocus}
        aria-pressed={selected}
        aria-label={t("settings.capture.card", {
          name: card.name,
          detail: card.state === "closed" ? t("settings.capture.notLaunchedLower") : card.detail,
        })}
        onClick={onSelect}
        className="absolute inset-0 rounded-md"
      />
      {!live && card.state === "live" && card.kind === "window" && (
        <button
          type="button"
          onClick={onPreview}
          aria-label={t("settings.capture.previewOf", { name: card.name })}
          className="pressable absolute top-2 right-2 z-10 h-8 rounded-md border border-line-strong bg-surface px-2.5 text-[13px] font-medium text-ink hover:bg-surface-hover"
        >
          {t("settings.capture.preview")}
        </button>
      )}
    </li>
  );
}

export function CaptureSection({ screensOnly = false }: { screensOnly?: boolean }) {
  const t = useT();
  const { config, loadError } = useConfigDraft();
  const [targets, setTargets] = useState<TargetInfo[] | null>(null);
  const [listError, setListError] = useState<string | null>(null);
  const [live, setLive] = useState<string[]>([]);
  const [frames, setFrames] = useState<Record<string, string>>({});
  const [failed, setFailed] = useState<Record<string, boolean>>({});
  const [visible, setVisible] = useState(document.visibilityState === "visible");
  const [focused, setFocused] = useState(document.hasFocus());
  const [focusedCard, setFocusedCard] = useState<string | null>(null);
  const running = useRef(new Map<string, Promise<number | null>>());
  const urls = useRef(new Map<string, string>());
  const mounted = useRef(true);
  const gridRef = useRef<HTMLDivElement>(null);
  const draft = config?.draft ?? null;
  const cards = draft && targets ? toCards(targets, draft) : [];
  const byKey = useRef(new Map<string, Card>());
  byKey.current = new Map(cards.map((card) => [card.key, card]));

  const refresh = useCallback(() => {
    listCaptureTargets().then(
      (list) => {
        setListError(null);
        setTargets(list);
      },
      (failure) => setListError(errorText(failure)),
    );
  }, []);

  useEffect(() => {
    mounted.current = true;
    refresh();
    const onVisibility = () => setVisible(document.visibilityState === "visible");
    const onFocus = () => {
      setFocused(true);
      refresh();
    };
    const onBlur = () => setFocused(false);
    window.addEventListener("focus", onFocus);
    window.addEventListener("blur", onBlur);
    document.addEventListener("visibilitychange", onVisibility);
    const previews = running.current;
    const frameUrls = urls.current;
    return () => {
      mounted.current = false;
      window.removeEventListener("focus", onFocus);
      window.removeEventListener("blur", onBlur);
      document.removeEventListener("visibilitychange", onVisibility);
      for (const id of previews.values()) {
        id.then((value) => {
          if (value !== null) stopPreview(value).catch(() => undefined);
        });
      }
      previews.clear();
      stopPreviews().catch(() => undefined);
      for (const url of frameUrls.values()) URL.revokeObjectURL(url);
      frameUrls.clear();
    };
  }, [refresh]);

  useEffect(() => {
    if (draft && targets) setLive((previous) => initialLive(toCards(targets, draft), previous));
  }, [targets, draft === null]);

  useEffect(() => {
    const wanted = new Set(visible && focused ? live : []);
    for (const [key, id] of running.current) {
      if (wanted.has(key)) continue;
      running.current.delete(key);
      id.then((value) => {
        if (value !== null) stopPreview(value).catch(() => undefined);
      });
    }
    for (const key of wanted) {
      const card = byKey.current.get(key);
      if (!card || running.current.has(key)) continue;
      const started = startPreview(card.target, (jpeg) => {
        if (!mounted.current || !running.current.has(key)) return;
        const url = URL.createObjectURL(new Blob([jpeg], { type: "image/jpeg" }));
        const old = urls.current.get(key);
        if (old) URL.revokeObjectURL(old);
        urls.current.set(key, url);
        setFrames((current) => ({ ...current, [key]: url }));
      }).catch(() => {
        setFailed((current) => ({ ...current, [key]: true }));
        return null;
      });
      running.current.set(key, started);
    }
  }, [live, visible, focused]);

  if (!config || !draft) return <LoadError message={loadError} />;

  const list = targets ?? [];
  const selected = selectedKey(cards, draft.capture, list);
  const capturing = config.armed ? selectedKey(cards, config.saved.capture, list) : null;
  const closed: Card | null =
    targets !== null && draft.capture.kind === "window" && selected === null
      ? {
          key: `closed:${draft.capture.exe_name}`,
          target: draft.capture,
          kind: "window",
          name: gameName(draft, draft.capture.exe_name),
          detail: draft.capture.exe_name,
          processId: null,
          exe: null,
          state: "closed",
        }
      : null;
  const monitors = cards.filter((card) => card.kind === "monitor");
  const windows = [...(closed ? [closed] : []), ...cards.filter((card) => card.kind === "window")];
  const focusKey =
    (focusedCard && (byKey.current.has(focusedCard) || focusedCard === closed?.key) ? focusedCard : null) ??
    closed?.key ??
    selected ??
    cards[0]?.key;

  const choose = (card: Card) => (screensOnly ? config.commit : config.change)({ ...draft, capture: card.target });
  const preview = (key: string) =>
    setLive((current) => {
      const windowKeys = current.filter((entry) => entry.startsWith("window:"));
      const dropped = windowKeys.length >= LIVE_WINDOWS ? windowKeys[0] : undefined;
      return [...current.filter((entry) => entry !== dropped), key];
    });
  const video = (patch: Partial<EngineConfig["video"]>) => config.change({ ...draft, video: { ...draft.video, ...patch } });

  const grid = (title: string, group: Card[], empty: string) => (
    <section aria-label={title} className="px-5 pt-5">
      <h3 className="mb-3 text-[15px] font-semibold">{title}</h3>
      {group.length === 0 ? (
        <p className="text-[13px] text-ink-2">{empty}</p>
      ) : (
        <ul className="grid grid-cols-2 gap-3 @3xl:grid-cols-3">
          {group.map((card) => (
            <CaptureCard
              key={card.key}
              card={card}
              selected={card.key === selected || card.key === closed?.key}
              armed={card.key === capturing}
              focusable={card.key === focusKey}
              frame={frames[card.key]}
              live={live.includes(card.key) && visible}
              failed={Boolean(failed[card.key])}
              onSelect={() => {
                if (card.key !== selected && card.key !== closed?.key) choose(card);
              }}
              onPreview={() => preview(card.key)}
              onFocus={() => setFocusedCard(card.key)}
            />
          ))}
        </ul>
      )}
    </section>
  );

  if (screensOnly)
    return (
      <div ref={gridRef} tabIndex={-1} onKeyDown={moveFocus} className="@container pb-5 outline-none">
        {listError && <LoadError message={listError} />}
        {targets !== null && grid(t("settings.capture.screens"), monitors, t("settings.capture.noScreens"))}
      </div>
    );

  return (
    <>
      <SettingRow label={t("settings.capture.cursor")}>
        <Switch
          checked={draft.video.capture_cursor}
          label={t("settings.capture.cursor")}
          onChange={(capture_cursor) => video({ capture_cursor })}
        />
      </SettingRow>
      <SettingRow label={t("settings.capture.border")} help={t("settings.capture.borderHelp")}>
        <Switch
          checked={draft.video.draw_border}
          label={t("settings.capture.border")}
          onChange={(draw_border) => video({ draw_border })}
        />
      </SettingRow>
      {listError && <LoadError message={listError} />}
      <div
        ref={gridRef}
        tabIndex={-1}
        onKeyDown={moveFocus}
        onFocus={(event) => {
          if (event.target === event.currentTarget)
            event.currentTarget.querySelector<HTMLElement>('[data-card][tabindex="0"]')?.focus();
        }}
        className="@container pb-5 outline-none"
      >
        {targets !== null && grid(t("settings.capture.screens"), monitors, t("settings.capture.noScreens"))}
        {targets !== null && grid(t("settings.capture.windows"), windows, t("settings.capture.noWindows"))}
      </div>
      <ConfigFooter config={config} focusAfter={gridRef} />
    </>
  );
}
