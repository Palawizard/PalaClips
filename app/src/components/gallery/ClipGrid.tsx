import { type KeyboardEvent, type RefObject, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { type GridRow, gridRows, neighbour } from "../../lib/gallery.ts";
import type { Clip, ClipSort } from "../../lib/ipc.ts";
import type { TrackLabels } from "../../lib/tracks.ts";
import { type ClipAction, ClipCard } from "./ClipCard.tsx";

const INSET = 20;
const GAP = 12;
const MIN_CARD = 232;
const CARD_BODY = 84;
const DAY_ROW = 46;
const OVERSCAN = 600;

interface Layout {
  columns: number;
  cardWidth: number;
  cardRow: number;
}

function layoutFor(width: number): Layout {
  const inner = Math.max(MIN_CARD, width - INSET * 2);
  const columns = Math.max(1, Math.floor((inner + GAP) / (MIN_CARD + GAP)));
  const cardWidth = (inner - GAP * (columns - 1)) / columns;
  return { columns, cardWidth, cardRow: Math.round((cardWidth * 9) / 16 + CARD_BODY + GAP) };
}

export function ClipGrid({
  clips,
  sort,
  labels,
  appOrder,
  focused,
  restoreFocus,
  renaming,
  renameError,
  scroller,
  onFocus,
  onAction,
  onRename,
  onRenameCancel,
}: {
  clips: Clip[];
  sort: ClipSort;
  labels: TrackLabels | null;
  appOrder: string[];
  focused: number;
  restoreFocus: number;
  renaming: string | null;
  renameError: string | null;
  scroller: RefObject<HTMLDivElement | null>;
  onFocus: (index: number) => void;
  onAction: (clip: Clip, action: ClipAction) => void;
  onRename: (clip: Clip, name: string) => void;
  onRenameCancel: () => void;
}) {
  const [width, setWidth] = useState(0);
  const [viewport, setViewport] = useState({ top: 0, height: 800 });
  const pendingFocus = useRef(false);
  const root = useRef<HTMLDivElement>(null);
  const layout = layoutFor(width);
  const rows = useMemo(() => gridRows(clips, layout.columns, sort), [clips, layout.columns, sort]);
  const offsets = useMemo(() => {
    let top = 0;
    return rows.map((row) => {
      const at = top;
      top += row.kind === "day" ? DAY_ROW : layout.cardRow;
      return at;
    });
  }, [rows, layout.cardRow]);
  const total = rows.length ? (offsets[offsets.length - 1] ?? 0) + (rows[rows.length - 1]?.kind === "day" ? DAY_ROW : layout.cardRow) : 0;

  useLayoutEffect(() => {
    const element = root.current?.parentElement;
    if (!element) return;
    let frame = 0;
    const measure = () => {
      setWidth(element.clientWidth);
      setViewport({ top: element.scrollTop, height: element.clientHeight });
    };
    const onScroll = () => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => setViewport({ top: element.scrollTop, height: element.clientHeight }));
    };
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(element);
    element.addEventListener("scroll", onScroll, { passive: true });
    return () => {
      cancelAnimationFrame(frame);
      observer.disconnect();
      element.removeEventListener("scroll", onScroll);
    };
  }, []);

  useEffect(() => {
    if (!pendingFocus.current) return;
    const target = scroller.current?.querySelector<HTMLElement>(`[data-index="${focused}"] [data-clip]`);
    if (!target) return;
    pendingFocus.current = false;
    target.focus({ preventScroll: true });
  });

  useEffect(() => {
    if (restoreFocus === 0) return;
    pendingFocus.current = true;
    reveal(focused);
    setViewport((current) => ({ ...current }));
  }, [restoreFocus]);

  const reveal = (index: number) => {
    const element = scroller.current;
    const at = rows.findIndex((row) => row.kind === "cards" && index >= row.first && index < row.first + row.clips.length);
    if (!element || at < 0) return;
    const top = offsets[at] ?? 0;
    const bottom = top + layout.cardRow;
    const previous = rows[at - 1];
    const headroom = previous?.kind === "day" ? DAY_ROW : 0;
    if (top - headroom < element.scrollTop) element.scrollTop = top - headroom;
    else if (bottom > element.scrollTop + element.clientHeight) element.scrollTop = bottom - element.clientHeight;
  };

  const keyDown = (clip: Clip, index: number) => (event: KeyboardEvent<HTMLButtonElement>) => {
    const steps: Record<string, "left" | "right" | "up" | "down"> = {
      ArrowLeft: "left",
      ArrowRight: "right",
      ArrowUp: "up",
      ArrowDown: "down",
    };
    const step = steps[event.key];
    if (step) {
      event.preventDefault();
      const next = neighbour(rows, index, step);
      if (next !== index) {
        pendingFocus.current = true;
        reveal(next);
        onFocus(next);
      }
      return;
    }
    const shortcut: ClipAction | null =
      event.key === "F2"
        ? "rename"
        : event.key === "Delete"
          ? "trash"
          : event.key.toLowerCase() === "f" && !event.ctrlKey && !event.altKey
            ? "favorite"
            : event.key.toLowerCase() === "c" && event.ctrlKey
              ? "copy"
              : null;
    if (shortcut) {
      event.preventDefault();
      onAction(clip, shortcut);
    }
  };

  const top = viewport.top - OVERSCAN;
  const bottom = viewport.top + viewport.height + OVERSCAN;
  const visible: { row: GridRow; at: number; index: number }[] = [];
  rows.forEach((row, index) => {
    const at = offsets[index] ?? 0;
    const height = row.kind === "day" ? DAY_ROW : layout.cardRow;
    if (at + height >= top && at <= bottom) visible.push({ row, at, index });
  });

  return (
    <div ref={root} role="grid" aria-rowcount={rows.length} className="relative" style={{ height: total + INSET }}>
      {visible.map(({ row, at, index: rowIndex }) =>
        row.kind === "day" ? (
          <div
            key={row.key}
            role="row"
            aria-rowindex={rowIndex + 1}
            className="absolute inset-x-0 flex items-end px-5 pb-2.5"
            style={{ top: at, height: DAY_ROW }}
          >
            <h2 role="rowheader" className="flex items-baseline gap-2 text-[15px] font-semibold">
              {row.label}
              <span className="score text-[15px] font-medium text-ink-3">{row.count}</span>
            </h2>
          </div>
        ) : (
          <div
            key={row.key}
            role="row"
            aria-rowindex={rowIndex + 1}
            className="absolute inset-x-0 grid px-5"
            style={{
              top: at,
              height: layout.cardRow - GAP,
              gridTemplateColumns: `repeat(${layout.columns}, minmax(0, 1fr))`,
              columnGap: GAP,
            }}
          >
            {row.clips.map((clip, offset) => {
              const index = row.first + offset;
              return (
                <div key={clip.path} role="gridcell" data-index={index} className="min-w-0">
                  <ClipCard
                    clip={clip}
                    labels={labels}
                    appOrder={appOrder}
                    focusable={index === focused}
                    renaming={renaming === clip.path}
                    renameError={renaming === clip.path ? renameError : null}
                    onAction={(action) => onAction(clip, action)}
                    onRename={(name) => onRename(clip, name)}
                    onRenameCancel={onRenameCancel}
                    onFocus={() => onFocus(index)}
                    onKeyDown={keyDown(clip, index)}
                  />
                </div>
              );
            })}
          </div>
        ),
      )}
    </div>
  );
}
