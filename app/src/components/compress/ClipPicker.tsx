import { convertFileSrc } from "@tauri-apps/api/core";
import { Check, Film, Search, Star } from "lucide-react";
import { type KeyboardEvent, type MouseEvent, useEffect, useRef, useState } from "react";
import { bytes, clock } from "../../lib/format.ts";
import { autoNamed, clipTitle, dayLabel, timeOfDay } from "../../lib/gallery.ts";
import { errorText, useT } from "../../lib/i18n.ts";
import { type Clip, clipGames, listClips } from "../../lib/ipc.ts";
import { useLibraryChanges } from "../../lib/library.ts";
import { Select } from "../settings/Settings.tsx";

const SEARCH_DELAY_MS = 150;
const COLUMNS_FALLBACK = 4;

function columnsOf(grid: HTMLElement | null): number {
  if (!grid) return COLUMNS_FALLBACK;
  return getComputedStyle(grid).gridTemplateColumns.split(" ").filter(Boolean).length || COLUMNS_FALLBACK;
}

export function ClipPicker({ open, onClose, onAdd }: { open: boolean; onClose: () => void; onAdd: (paths: string[]) => void }) {
  const t = useT();
  const dialog = useRef<HTMLDialogElement>(null);
  const grid = useRef<HTMLDivElement>(null);
  const [search, setSearch] = useState("");
  const [game, setGame] = useState("");
  const [favorites, setFavorites] = useState(false);
  const [games, setGames] = useState<string[]>([]);
  const [clips, setClips] = useState<Clip[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [selected, setSelected] = useState<string[]>([]);
  const [focus, setFocus] = useState(0);
  const [version, setVersion] = useState(0);
  const anchor = useRef(0);

  useLibraryChanges(() => setVersion((current) => current + 1));

  useEffect(() => {
    const element = dialog.current;
    if (!element) return;
    if (open && !element.open) {
      setSelected([]);
      setFocus(0);
      element.showModal();
      clipGames().then(setGames, () => setGames([]));
    } else if (!open && element.open) element.close();
  }, [open]);

  useEffect(() => {
    if (!open) return;
    const timer = window.setTimeout(() => {
      listClips({ search: search || null, game: game || null, favorites, sort: "newest" }).then(
        (list) => {
          setClips(list);
          setError(null);
          setFocus((current) => Math.min(current, Math.max(0, list.length - 1)));
        },
        (failure) => setError(errorText(failure)),
      );
    }, SEARCH_DELAY_MS);
    return () => window.clearTimeout(timer);
  }, [open, search, game, favorites, version]);

  const list = clips ?? [];
  const chosen = list.filter((clip) => selected.includes(clip.path));
  const total = chosen.reduce((sum, clip) => sum + clip.size_bytes, 0);

  const toggle = (index: number, range: boolean) => {
    const clip = list[index];
    if (!clip) return;
    if (range) {
      const [from, to] = [Math.min(anchor.current, index), Math.max(anchor.current, index)];
      const span = list.slice(from, to + 1).map((item) => item.path);
      setSelected((current) => [...new Set([...current, ...span])]);
    } else {
      anchor.current = index;
      setSelected((current) => (current.includes(clip.path) ? current.filter((path) => path !== clip.path) : [...current, clip.path]));
    }
    setFocus(index);
  };

  const add = () => {
    if (selected.length === 0) return;
    onAdd(list.filter((clip) => selected.includes(clip.path)).map((clip) => clip.path));
  };

  const move = (index: number) => {
    const next = Math.max(0, Math.min(list.length - 1, index));
    setFocus(next);
    grid.current?.querySelector<HTMLElement>(`[data-index="${next}"]`)?.focus();
  };

  const keyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    const columns = columnsOf(grid.current);
    const handled = (() => {
      if (event.key === "ArrowRight") move(focus + 1);
      else if (event.key === "ArrowLeft") move(focus - 1);
      else if (event.key === "ArrowDown") move(focus + columns);
      else if (event.key === "ArrowUp") move(focus - columns);
      else if (event.key === "Home") move(0);
      else if (event.key === "End") move(list.length - 1);
      else if (event.key === " ") toggle(focus, event.shiftKey);
      else if (event.key === "Enter") add();
      else if (event.key.toLowerCase() === "a" && event.ctrlKey) setSelected(list.map((clip) => clip.path));
      else return false;
      return true;
    })();
    if (handled) event.preventDefault();
  };

  return (
    <dialog
      ref={dialog}
      aria-labelledby="picker-title"
      onCancel={(event) => {
        event.preventDefault();
        onClose();
      }}
      className="leave-dialog m-auto flex h-[min(80vh,720px)] w-[min(1040px,calc(100vw-48px))] flex-col rounded-md border border-line-strong bg-surface p-0 text-ink shadow-[0_16px_48px_rgb(0_0_0/0.5)] [&:not([open])]:hidden"
    >
      <header className="flex flex-col gap-3 border-b border-line px-5 pt-5 pb-3">
        <h2 id="picker-title" className="text-[17px] font-semibold">
          {t("picker.title")}
        </h2>
        <div className="flex flex-wrap items-center gap-2">
          <label className="relative flex items-center">
            <Search size={16} aria-hidden className="pointer-events-none absolute left-2.5 text-ink-3" />
            <input
              type="search"
              value={search}
              onChange={(event) => setSearch(event.target.value)}
              placeholder={t("gallery.search")}
              aria-label={t("gallery.searchLabel")}
              className="h-9 w-56 rounded-md border border-ink-3 bg-surface pr-2.5 pl-8 text-[14px] text-ink placeholder:text-ink-3 hover:border-ink-2"
            />
          </label>
          <Select
            label={t("gallery.game")}
            value={game}
            className="max-w-52 min-w-40"
            options={[{ value: "", label: t("gallery.allGames") }, ...games.map((name) => ({ value: name, label: name }))]}
            onChange={setGame}
          />
          <button
            type="button"
            aria-pressed={favorites}
            onClick={() => setFavorites((current) => !current)}
            className={`pressable inline-flex h-9 items-center gap-2 rounded-md border px-3 text-[14px] font-medium ${
              favorites ? "border-ink bg-surface-2 text-ink" : "border-line-strong text-ink-2 hover:bg-surface-hover hover:text-ink"
            }`}
          >
            <Star size={16} fill={favorites ? "currentColor" : "none"} aria-hidden />
            {t("gallery.favorites")}
          </button>
        </div>
      </header>

      <div className="min-h-0 flex-1 overflow-y-auto px-5 py-4">
        {error && <p className="text-[13px] text-warn">{error}</p>}
        {clips !== null && list.length === 0 && !error && <p className="text-ink-2">{t("gallery.noMatch")}</p>}
        <div
          ref={grid}
          role="listbox"
          aria-multiselectable
          aria-label={t("picker.title")}
          onKeyDown={keyDown}
          className="grid grid-cols-[repeat(auto-fill,minmax(200px,1fr))] gap-3"
        >
          {list.map((clip, index) => {
            const on = selected.includes(clip.path);
            return (
              <div
                key={clip.path}
                role="option"
                aria-selected={on}
                data-index={index}
                tabIndex={index === focus ? 0 : -1}
                onClick={(event: MouseEvent) => toggle(index, event.shiftKey)}
                className={`picker-card pressable relative cursor-pointer overflow-hidden rounded-md border outline-offset-2 ${
                  on ? "border-ink bg-surface-2" : "border-line bg-surface hover:border-line-strong hover:bg-surface-hover"
                }`}
              >
                <div className="relative aspect-video bg-meter-ground">
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
                    <Film size={22} className="absolute inset-0 m-auto text-ink-3" aria-hidden />
                  )}
                  <span className="score absolute right-1.5 bottom-1.5 rounded-[3px] bg-ground/85 px-1.5 text-[13px] font-semibold">
                    {clock(clip.duration_secs)}
                  </span>
                  <span
                    aria-hidden
                    className={`absolute top-1.5 right-1.5 flex size-6 items-center justify-center rounded-full border-2 ${
                      on ? "border-ink bg-ink text-ground" : "border-ink/70 bg-ground/60 text-transparent"
                    }`}
                  >
                    <Check size={14} strokeWidth={3} />
                  </span>
                </div>
                <div className="flex flex-col gap-0.5 px-2.5 pt-2 pb-2.5">
                  <p className="truncate text-[13px] font-semibold" title={clip.name}>
                    {clipTitle(clip)}
                  </p>
                  <p className="truncate text-[12px] text-ink-2">
                    {autoNamed(clip) ? `${dayLabel(clip.modified_ms)} ${timeOfDay(clip.modified_ms)}` : clip.game} ·{" "}
                    {bytes(clip.size_bytes)}
                  </p>
                </div>
              </div>
            );
          })}
        </div>
      </div>

      <footer className="flex flex-wrap items-center gap-3 border-t border-line bg-surface-2 px-5 py-3">
        <p className="score mr-auto text-[15px] text-ink-2" aria-live="polite">
          {chosen.length > 0 && t("picker.summary", { count: chosen.length, size: bytes(total) })}
        </p>
        <button
          type="button"
          onClick={onClose}
          className="pressable h-9 rounded-md px-3.5 font-medium text-ink-2 hover:bg-surface-hover hover:text-ink"
        >
          {t("picker.cancel")}
        </button>
        <button
          type="button"
          disabled={selected.length === 0}
          onClick={add}
          className="pressable h-9 rounded-md bg-ink px-3.5 font-semibold text-ground hover:bg-ink/90 disabled:cursor-not-allowed disabled:border disabled:border-line disabled:bg-transparent disabled:text-ink-3"
        >
          {t("picker.add", { count: selected.length })}
        </button>
      </footer>
    </dialog>
  );
}
