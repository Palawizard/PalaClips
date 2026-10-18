import { convertFileSrc } from "@tauri-apps/api/core";
import { Copy, Ellipsis, Film, FolderOpen, Pencil, Play, Star, Trash2 } from "lucide-react";
import { type KeyboardEvent, useEffect, useRef, useState } from "react";
import { bytes, clock } from "../../lib/format.ts";
import { autoNamed, clipTitle, dayLabel, timeOfDay } from "../../lib/gallery.ts";
import { useT } from "../../lib/i18n.ts";
import type { Clip } from "../../lib/ipc.ts";
import { type TrackLabels, kindOfTitle, trackColor } from "../../lib/tracks.ts";

export type ClipAction = "open" | "favorite" | "rename" | "copy" | "reveal" | "trash";

export const CARD_SHORTCUTS = "Enter F F2 Delete Control+C Shift+F10";
const FAVORITE_KEY = "F";

function NameEditor({
  clip,
  error,
  onCommit,
  onCancel,
}: {
  clip: Clip;
  error: string | null;
  onCommit: (name: string) => void;
  onCancel: () => void;
}) {
  const t = useT();
  const [text, setText] = useState(clip.name);
  const input = useRef<HTMLInputElement>(null);
  useEffect(() => {
    input.current?.focus();
    input.current?.select();
  }, []);
  const commit = () => {
    const next = text.trim();
    if (next === "" || next === clip.name) onCancel();
    else onCommit(next);
  };
  return (
    <div className="relative z-20 flex flex-col gap-1">
      <input
        ref={input}
        type="text"
        value={text}
        spellCheck={false}
        aria-label={t("gallery.renameLabel", { name: clip.name })}
        aria-invalid={error ? true : undefined}
        onChange={(event) => setText(event.target.value)}
        onBlur={commit}
        onKeyDown={(event) => {
          event.stopPropagation();
          if (event.key === "Enter") commit();
          if (event.key === "Escape") onCancel();
        }}
        className="h-8 w-full rounded-md border border-ink-3 bg-surface px-2 text-[14px] font-semibold text-ink aria-invalid:border-warn"
      />
      {error && (
        <p role="alert" className="text-[12px] text-warn">
          {error}
        </p>
      )}
    </div>
  );
}

function ActionMenu({
  clip,
  open,
  onOpenChange,
  onAction,
  onClosed,
}: {
  clip: Clip;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onAction: (action: ClipAction) => void;
  onClosed: () => void;
}) {
  const t = useT();
  const root = useRef<HTMLDivElement>(null);
  const items: { action: ClipAction; label: string; keys: string | null; icon: typeof Copy }[] = [
    { action: "open", label: t("gallery.openItem"), keys: t("keys.enter"), icon: Play },
    {
      action: "favorite",
      label: clip.favorite ? t("gallery.unfavorite") : t("gallery.favorite"),
      keys: "F",
      icon: Star,
    },
    { action: "rename", label: t("gallery.rename"), keys: "F2", icon: Pencil },
    { action: "copy", label: t("gallery.copy"), keys: "Ctrl+C", icon: Copy },
    { action: "reveal", label: t("gallery.reveal"), keys: null, icon: FolderOpen },
    { action: "trash", label: t("gallery.trash"), keys: t("keys.delete"), icon: Trash2 },
  ];

  useEffect(() => {
    if (!open) return;
    root.current?.querySelector<HTMLElement>('[role="menuitem"]')?.focus();
    const close = (event: PointerEvent) => {
      if (!root.current?.contains(event.target as Node)) onOpenChange(false);
    };
    window.addEventListener("pointerdown", close);
    return () => window.removeEventListener("pointerdown", close);
  }, [open, onOpenChange]);

  const move = (event: KeyboardEvent<HTMLDivElement>) => {
    event.stopPropagation();
    const entries = [...(root.current?.querySelectorAll<HTMLElement>('[role="menuitem"]') ?? [])];
    const index = entries.indexOf(document.activeElement as HTMLElement);
    if (event.key === "Escape" || event.key === "Tab") {
      event.preventDefault();
      onOpenChange(false);
      onClosed();
    } else if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      const step = event.key === "ArrowDown" ? 1 : -1;
      entries[(index + step + entries.length) % entries.length]?.focus();
    } else if (event.key === "Home" || event.key === "End") {
      event.preventDefault();
      entries[event.key === "Home" ? 0 : entries.length - 1]?.focus();
    }
  };

  return (
    <div ref={root} className="relative" onKeyDown={move}>
      <button
        type="button"
        tabIndex={-1}
        aria-haspopup="menu"
        aria-expanded={open}
        aria-label={t("gallery.actions", { name: clipTitle(clip) })}
        title={t("shortcuts.withKey", { label: t("gallery.actions", { name: clipTitle(clip) }), key: t("keys.menu") })}
        onClick={() => onOpenChange(!open)}
        className="pressable flex size-8 items-center justify-center rounded-md text-ink-3 hover:bg-surface-hover hover:text-ink"
      >
        <Ellipsis size={18} aria-hidden />
      </button>
      {open && (
        <div
          role="menu"
          aria-label={t("gallery.actions", { name: clipTitle(clip) })}
          className="clip-menu absolute top-9 right-0 z-30 flex min-w-60 flex-col rounded-md border border-line-strong bg-surface-2 p-1"
        >
          {items.map(({ action, label, keys, icon: Icon }) => (
            <button
              key={action}
              type="button"
              role="menuitem"
              tabIndex={-1}
              aria-keyshortcuts={keys ?? undefined}
              onClick={() => {
                onOpenChange(false);
                onClosed();
                onAction(action);
              }}
              className="pressable flex h-9 items-center gap-2.5 rounded-[4px] px-2.5 text-left text-[14px] text-ink hover:bg-surface-hover"
            >
              <Icon size={16} aria-hidden />
              <span className="flex-1">{label}</span>
              {keys && <span className="score text-[13px] text-ink-2">{keys}</span>}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

export function ClipCard({
  clip,
  labels,
  appOrder,
  focusable,
  renaming,
  renameError,
  onAction,
  onRename,
  onRenameCancel,
  onFocus,
  onKeyDown,
}: {
  clip: Clip;
  labels: TrackLabels | null;
  appOrder: string[];
  focusable: boolean;
  renaming: boolean;
  renameError: string | null;
  onAction: (action: ClipAction) => void;
  onRename: (name: string) => void;
  onRenameCancel: () => void;
  onFocus: () => void;
  onKeyDown: (event: KeyboardEvent<HTMLButtonElement>) => void;
}) {
  const t = useT();
  const [menuOpen, setMenuOpen] = useState(false);
  const card = useRef<HTMLButtonElement>(null);
  const recording = clip.kind === "recording";
  const named = !autoNamed(clip);
  const title = clipTitle(clip);
  const tracks = t("clips.tracks", { count: clip.tracks.length, names: clip.tracks.join(", ") });
  const keyDown = (event: KeyboardEvent<HTMLButtonElement>) => {
    if (event.key === "ContextMenu" || (event.key === "F10" && event.shiftKey)) {
      event.preventDefault();
      setMenuOpen(true);
      return;
    }
    onKeyDown(event);
  };

  return (
    <div
      data-favorite={clip.favorite}
      className="gallery-card group relative flex h-full flex-col overflow-hidden rounded-md border border-line bg-surface hover:border-line-strong hover:bg-surface-hover"
    >
      <div className="relative aspect-video overflow-hidden bg-meter-ground">
        {clip.thumbnail ? (
          <img
            src={convertFileSrc(clip.thumbnail)}
            alt=""
            loading="lazy"
            decoding="async"
            draggable={false}
            className="size-full object-cover"
          />
        ) : (
          <Film size={24} className="absolute inset-0 m-auto text-ink-3" aria-hidden />
        )}
        <span className="card-play absolute inset-0 m-auto flex size-11 items-center justify-center rounded-full bg-ground/80 text-ink opacity-0 group-hover:opacity-100">
          <Play size={20} fill="currentColor" aria-hidden />
        </span>
        {recording && (
          <span className="score absolute top-1.5 left-1.5 rounded-[3px] border border-ink/70 bg-ground/60 px-1.5 text-[13px] font-semibold text-ink">
            {t("gallery.recordingChip")}
          </span>
        )}
        <span className="score absolute right-1.5 bottom-1.5 rounded-[3px] bg-ground/85 px-1.5 text-[14px] font-semibold text-ink">
          {clock(clip.duration_secs)}
        </span>
      </div>
      <div className="flex min-w-0 flex-1 flex-col gap-1.5 px-3 pt-2.5 pb-3">
        {renaming ? (
          <NameEditor clip={clip} error={renameError} onCommit={onRename} onCancel={onRenameCancel} />
        ) : (
          <p className="truncate pr-16 text-[14px] font-semibold" title={clip.name}>
            {title}
          </p>
        )}
        <p className="flex min-w-0 items-baseline gap-1.5 text-[12px] text-ink-2">
          {named && (
            <>
              <span className="truncate">{clip.game || t("clips.fallbackName")}</span>
              <span aria-hidden className="text-ink-3">
                ·
              </span>
            </>
          )}
          <span className="score shrink-0 text-[13px]">{timeOfDay(clip.modified_ms)}</span>
          <span aria-hidden className="text-ink-3">
            ·
          </span>
          <span className="score shrink-0 text-[13px]">{bytes(clip.size_bytes)}</span>
        </p>
        <ul aria-hidden className="mt-auto flex flex-wrap gap-1 pt-0.5">
          {clip.tracks.map((trackTitle, index) => (
            <li
              key={`${index}:${trackTitle}`}
              title={trackTitle}
              className="h-1.5 w-5 rounded-full"
              style={{ background: trackColor(kindOfTitle(trackTitle, labels), trackTitle, appOrder) }}
            />
          ))}
        </ul>
      </div>
      <button
        ref={card}
        type="button"
        data-clip={clip.path}
        tabIndex={focusable ? 0 : -1}
        aria-label={t("gallery.card", {
          name: title,
          game: clip.game || t("clips.fallbackName"),
          date: `${dayLabel(clip.modified_ms)} ${timeOfDay(clip.modified_ms)}`,
          duration: clock(clip.duration_secs),
          tracks,
        })}
        aria-description={clip.favorite ? t("gallery.favorites") : undefined}
        aria-keyshortcuts={CARD_SHORTCUTS}
        onClick={() => onAction("open")}
        onFocus={onFocus}
        onKeyDown={keyDown}
        className="absolute inset-0 rounded-md"
      />
      <div className="absolute top-[calc(56.25%+4px)] right-1.5 z-10 flex items-center">
        <button
          type="button"
          tabIndex={-1}
          aria-pressed={clip.favorite}
          aria-label={clip.favorite ? t("gallery.unfavorite") : t("gallery.favorite")}
          title={t("shortcuts.withKey", {
            label: clip.favorite ? t("gallery.unfavorite") : t("gallery.favorite"),
            key: FAVORITE_KEY,
          })}
          onClick={() => onAction("favorite")}
          className={`pressable flex size-8 items-center justify-center rounded-md hover:bg-surface-hover ${
            clip.favorite ? "text-ink" : "text-ink-3 opacity-0 group-focus-within:opacity-100 group-hover:opacity-100"
          }`}
        >
          <Star size={17} fill={clip.favorite ? "currentColor" : "none"} aria-hidden />
        </button>
        <div
          className={
            menuOpen ? "opacity-100" : "opacity-0 group-focus-within:opacity-100 group-hover:opacity-100"
          }
        >
          <ActionMenu
            clip={clip}
            open={menuOpen}
            onOpenChange={setMenuOpen}
            onAction={onAction}
            onClosed={() => card.current?.focus()}
          />
        </div>
      </div>
    </div>
  );
}
