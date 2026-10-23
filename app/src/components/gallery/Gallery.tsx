import { Search, Star } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { DraftRequest } from "../../lib/compressDrafts.ts";
import { type Period, clipTitle, sinceOf } from "../../lib/gallery.ts";
import { errorText, t as translate, useT } from "../../lib/i18n.ts";
import {
  type Clip,
  type ClipKind,
  type ClipSort,
  type EngineConfig,
  clipGames,
  copyClip,
  listClips,
  onLibraryChanged,
  renameClip,
  revealClip,
  setClipFavorite,
  trashClip,
} from "../../lib/ipc.ts";
import { Segmented, Select } from "../settings/Settings.tsx";
import type { ClipAction } from "./ClipCard.tsx";
import { ClipGrid } from "./ClipGrid.tsx";
import { Player } from "./Player.tsx";
import { type Shortcut, Shortcuts } from "./Shortcuts.tsx";

const SEARCH_DELAY_MS = 150;
const REFRESH_DELAY_MS = 200;
const NOTICE_MS = 6_000;
const TRASH_DELAY_MS = 6_000;
const SKELETON_CARDS = 8;
const PERIODS: Period[] = ["all", "today", "week", "month"];
const SORTS: ClipSort[] = ["newest", "oldest", "largest", "longest", "name"];
const KINDS = ["all", "replay", "recording"] as const;
type KindFilter = (typeof KINDS)[number];

interface Filters {
  search: string;
  game: string;
  kind: KindFilter;
  favorites: boolean;
  period: Period;
  sort: ClipSort;
}

interface PendingTrash {
  clip: Clip;
  timer: number;
}

const NO_FILTERS: Filters = { search: "", game: "", kind: "all", favorites: false, period: "all", sort: "newest" };

function gridShortcuts(): Shortcut[] {
  return [
    { keys: [translate("keys.arrows")], label: translate("shortcuts.move") },
    { keys: [translate("keys.enter")], label: translate("shortcuts.open") },
    { keys: ["F"], label: translate("shortcuts.favorite") },
    { keys: ["F2"], label: translate("shortcuts.rename") },
    { keys: ["Ctrl+C"], label: translate("shortcuts.copy") },
    { keys: [translate("keys.delete")], label: translate("shortcuts.trash") },
    { keys: [translate("keys.menu")], label: translate("shortcuts.menu") },
  ];
}

export function Gallery({
  config,
  saveKeys,
  onCompress,
}: {
  config: EngineConfig | null;
  saveKeys: string | null;
  onCompress: (requests: DraftRequest[]) => void;
}) {
  const t = useT();
  const [filters, setFilters] = useState<Filters>(NO_FILTERS);
  const [search, setSearch] = useState("");
  const [clips, setClips] = useState<Clip[] | null>(null);
  const [games, setGames] = useState<string[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [pendingTrash, setPendingTrash] = useState<PendingTrash | null>(null);
  const [focused, setFocused] = useState(0);
  const [restoreFocus, setRestoreFocus] = useState(0);
  const [renaming, setRenaming] = useState<string | null>(null);
  const [renameError, setRenameError] = useState<string | null>(null);
  const [open, setOpen] = useState<Clip | null>(null);
  const scroller = useRef<HTMLDivElement>(null);
  const request = useRef(0);
  const trashQueue = useRef<PendingTrash | null>(null);
  trashQueue.current = pendingTrash;
  const labels = config?.audio ?? null;
  const appOrder = useMemo(
    () => config?.audio.app_tracks.filter((track) => track.enabled).map((track) => track.label) ?? [],
    [config],
  );

  const load = useCallback(() => {
    const id = ++request.current;
    const kind: ClipKind | null = filters.kind === "all" ? null : filters.kind;
    listClips({
      game: filters.game || null,
      kind,
      favorites: filters.favorites,
      since_ms: sinceOf(filters.period),
      search: filters.search || null,
      sort: filters.sort,
    }).then(
      (list) => {
        if (id !== request.current) return;
        setError(null);
        const hidden = trashQueue.current?.clip.path;
        setClips(hidden ? list.filter((clip) => clip.path !== hidden) : list);
      },
      (failure) => {
        if (id !== request.current) return;
        setError(errorText(failure));
        setClips((current) => current ?? []);
      },
    );
    clipGames().then(setGames, () => setGames([]));
  }, [filters]);

  useEffect(load, [load]);

  useEffect(() => {
    const timer = window.setTimeout(() => setFilters((current) => ({ ...current, search: search.trim() })), SEARCH_DELAY_MS);
    return () => window.clearTimeout(timer);
  }, [search]);

  useEffect(() => {
    let timer = 0;
    const unlisten = onLibraryChanged(() => {
      window.clearTimeout(timer);
      timer = window.setTimeout(load, REFRESH_DELAY_MS);
    });
    return () => {
      window.clearTimeout(timer);
      unlisten.then((stop) => stop());
    };
  }, [load]);

  useEffect(() => {
    if (!notice || pendingTrash) return;
    const timer = window.setTimeout(() => setNotice(null), NOTICE_MS);
    return () => window.clearTimeout(timer);
  }, [notice, pendingTrash]);

  const commitTrash = useCallback((pending: PendingTrash) => {
    window.clearTimeout(pending.timer);
    trashClip(pending.clip.path).catch((failure) => setError(errorText(failure)));
  }, []);

  useEffect(() => {
    const flush = () => {
      if (trashQueue.current) commitTrash(trashQueue.current);
      trashQueue.current = null;
    };
    window.addEventListener("pagehide", flush);
    return () => {
      window.removeEventListener("pagehide", flush);
      flush();
    };
  }, [commitTrash]);

  useEffect(() => {
    setFocused((current) => Math.min(current, Math.max(0, (clips?.length ?? 1) - 1)));
  }, [clips]);

  const update = (patch: Partial<Filters>) => {
    setFilters((current) => ({ ...current, ...patch }));
    setFocused(0);
    scroller.current?.scrollTo({ top: 0 });
  };

  const scheduleTrash = (clip: Clip) => {
    if (trashQueue.current) commitTrash(trashQueue.current);
    trashQueue.current = null;
    const timer = window.setTimeout(() => {
      const current = trashQueue.current;
      if (current?.clip.path !== clip.path) return;
      commitTrash(current);
      trashQueue.current = null;
      setPendingTrash(null);
      setNotice(null);
    }, TRASH_DELAY_MS);
    setPendingTrash({ clip, timer });
    setNotice(t("gallery.trashPending", { name: clipTitle(clip) }));
    setClips((current) => current?.filter((item) => item.path !== clip.path) ?? null);
  };

  const undoTrash = () => {
    if (!pendingTrash) return;
    window.clearTimeout(pendingTrash.timer);
    setPendingTrash(null);
    setNotice(null);
    load();
    setRestoreFocus((current) => current + 1);
  };

  const act = (clip: Clip, action: ClipAction) => {
    setNotice(null);
    setError(null);
    const fail = (failure: unknown) => setError(errorText(failure));
    if (action === "open") setOpen(clip);
    else if (action === "favorite")
      setClipFavorite(clip.path, !clip.favorite).then(
        () => setClips((current) => current?.map((item) => (item.path === clip.path ? { ...item, favorite: !clip.favorite } : item)) ?? null),
        fail,
      );
    else if (action === "rename") {
      setRenameError(null);
      setRenaming(clip.path);
    } else if (action === "copy") copyClip(clip.path).then(() => setNotice(t("gallery.copied")), fail);
    else if (action === "compress") onCompress([{ path: clip.path }]);
    else if (action === "reveal") revealClip(clip.path).catch(fail);
    else if (action === "trash") {
      scheduleTrash(clip);
      setRestoreFocus((current) => current + 1);
    }
  };

  const rename = (clip: Clip, name: string) => {
    renameClip(clip.path, name).then(
      (path) => {
        setRenaming(null);
        setClips((current) => current?.map((item) => (item.path === clip.path ? { ...item, path, name } : item)) ?? null);
        setRestoreFocus((current) => current + 1);
      },
      (failure) => setRenameError(errorText(failure)),
    );
  };

  if (open) {
    return (
      <Player
        clip={open}
        labels={labels}
        appOrder={appOrder}
        onBack={() => {
          setOpen(null);
          load();
          setRestoreFocus((current) => current + 1);
        }}
        onTrash={(clip) => {
          setOpen(null);
          scheduleTrash(clip);
          setRestoreFocus((current) => current + 1);
        }}
        onChange={(changed) => setOpen(changed)}
        onCompress={onCompress}
      />
    );
  }

  const filtered =
    filters.search !== "" || filters.game !== "" || filters.kind !== "all" || filters.favorites || filters.period !== "all";

  return (
    <section aria-labelledby="gallery-title" className="flex min-h-0 flex-1 flex-col">
      <header className="flex flex-col gap-3 border-b border-line px-5 pt-5 pb-3">
        <div className="flex flex-wrap items-center gap-x-3 gap-y-2">
          <h1 id="gallery-title" className="text-[22px] font-semibold">
            {t("nav.gallery")}
          </h1>
          {clips && (
            <span className="score text-[17px] font-medium text-ink-2">
              {t("gallery.count", { count: clips.length })}
            </span>
          )}
          <span className="ml-auto flex items-center gap-2">
            <Select
              label={t("gallery.sort")}
              value={filters.sort}
              options={SORTS.map((value) => ({ value, label: t(`gallery.sorts.${value}`) }))}
              onChange={(sort) => update({ sort })}
            />
            <Shortcuts shortcuts={gridShortcuts()} />
          </span>
        </div>
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
            value={filters.game}
            className="max-w-52 min-w-40"
            options={[{ value: "", label: t("gallery.allGames") }, ...games.map((game) => ({ value: game, label: game }))]}
            onChange={(game) => update({ game })}
          />
          <Segmented<KindFilter>
            label={t("gallery.kind")}
            value={filters.kind}
            options={KINDS.map((value) => ({ value, label: t(`gallery.kinds.${value}`) }))}
            onChange={(kind) => update({ kind })}
          />
          <button
            type="button"
            aria-pressed={filters.favorites}
            onClick={() => update({ favorites: !filters.favorites })}
            className={`pressable inline-flex h-9 items-center gap-2 rounded-md border px-3 text-[14px] font-medium ${
              filters.favorites ? "border-ink bg-surface-2 text-ink" : "border-line-strong text-ink-2 hover:bg-surface-hover hover:text-ink"
            }`}
          >
            <Star size={16} fill={filters.favorites ? "currentColor" : "none"} aria-hidden />
            {t("gallery.favorites")}
          </button>
          <Select
            label={t("gallery.period")}
            value={filters.period}
            options={PERIODS.map((value) => ({ value, label: t(`gallery.periods.${value}`) }))}
            onChange={(period) => update({ period })}
          />
        </div>
      </header>
      {error && (
        <p role="alert" className="border-b border-line bg-surface-2 px-5 py-3 text-[13px] text-warn">
          {error}
        </p>
      )}
      <div role="status" className={notice ? "flex items-center gap-3 border-b border-line bg-surface-2 px-5 py-2" : "sr-only"}>
        {notice && <p className="flex-1 py-1 text-[13px] text-ink">{notice}</p>}
        {notice && pendingTrash && (
          <button
            type="button"
            onClick={undoTrash}
            className="pressable h-8 rounded-md border border-line-strong px-3 text-[13px] font-medium text-ink hover:bg-surface-hover"
          >
            {t("gallery.undo")}
          </button>
        )}
      </div>
      <div ref={scroller} className="min-h-0 flex-1 overflow-y-auto pt-1">
        {clips === null ? (
          <div aria-busy="true" aria-label={t("gallery.loading")} className="grid grid-cols-[repeat(auto-fill,minmax(232px,1fr))] gap-3 px-5 pt-14">
            {Array.from({ length: SKELETON_CARDS }, (_, index) => (
              <div key={index} className="skeleton aspect-[16/12] rounded-md bg-surface" />
            ))}
          </div>
        ) : clips.length === 0 ? (
          <div className="px-5 pt-6">
            <div className="max-w-[60ch] rounded-md border border-dashed border-line-strong px-4 py-6 text-ink-2">
              {filtered ? (
                <>
                  <p className="font-semibold text-ink">{t("gallery.noMatch")}</p>
                  <button
                    type="button"
                    onClick={() => {
                      setSearch("");
                      update(NO_FILTERS);
                    }}
                    className="pressable mt-3 h-9 rounded-md border border-line-strong px-3.5 font-medium text-ink hover:bg-surface-hover"
                  >
                    {t("gallery.clearFilters")}
                  </button>
                </>
              ) : (
                <>
                  <p className="font-semibold text-ink">{t("clips.emptyTitle")}</p>
                  <p className="mt-1">
                    {saveKeys ? t("clips.emptyWithKeys", { keys: saveKeys }) : t("clips.emptyNoKeys")}
                  </p>
                </>
              )}
            </div>
          </div>
        ) : (
          <ClipGrid
            clips={clips}
            sort={filters.sort}
            labels={labels}
            appOrder={appOrder}
            focused={focused}
            restoreFocus={restoreFocus}
            renaming={renaming}
            renameError={renameError}
            scroller={scroller}
            onFocus={setFocused}
            onAction={act}
            onRename={rename}
            onRenameCancel={() => {
              setRenaming(null);
              setRenameError(null);
              setRestoreFocus((current) => current + 1);
            }}
          />
        )}
      </div>
    </section>
  );
}
