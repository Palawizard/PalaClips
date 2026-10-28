import { convertFileSrc } from "@tauri-apps/api/core";
import { Film } from "lucide-react";
import type { ClipSummary } from "../lib/ipc.ts";
import { clock, relativeTime } from "../lib/format.ts";
import { dayLabel, timeOfDay } from "../lib/gallery.ts";
import { useT } from "../lib/i18n.ts";
import { type TrackLabels, kindOfTitle, trackColor } from "../lib/tracks.ts";

export function ClipCard({
  clip,
  appOrder,
  labels,
  fresh,
  onOpen,
}: {
  clip: ClipSummary;
  appOrder: string[];
  labels: TrackLabels | null;
  fresh: boolean;
  onOpen?: () => void;
}) {
  const t = useT();
  return (
    <li
      data-fresh={fresh}
      className={`clip-card relative overflow-hidden rounded-md border border-line bg-surface ${
        onOpen ? "hover:border-line-strong hover:bg-surface-hover" : ""
      }`}
    >
      <div className="relative aspect-video bg-meter-ground">
        {clip.thumbnail ? (
          <img src={convertFileSrc(clip.thumbnail)} alt="" className="size-full object-cover" draggable={false} />
        ) : (
          <Film size={22} className="absolute inset-0 m-auto text-ink-3" aria-hidden />
        )}
        <span className="score absolute right-1.5 bottom-1.5 rounded-[3px] bg-ground/85 px-1.5 text-[14px] font-semibold text-ink">
          {clock(clip.duration_secs)}
        </span>
      </div>
      <div className="flex flex-col gap-1.5 px-3 py-2.5">
        <div className="flex items-baseline justify-between gap-2">
          <p className="truncate font-semibold" title={clip.path}>
            {clip.game || t("clips.fallbackName")}
          </p>
          <p className="shrink-0 text-[12px] text-ink-3">{relativeTime(clip.modified_ms)}</p>
        </div>
        <ul
          aria-label={t("clips.tracks", { count: clip.tracks.length, names: clip.tracks.join(", ") })}
          className="flex flex-wrap gap-1"
        >
          {clip.tracks.map((title) => (
            <li
              key={title}
              title={title}
              className="h-1.5 w-5 rounded-full"
              style={{ background: trackColor(kindOfTitle(title, labels), title, appOrder) }}
            />
          ))}
        </ul>
      </div>
      {onOpen && (
        <button
          type="button"
          data-open
          aria-label={t("clips.card", {
            game: clip.game || t("clips.fallbackName"),
            date: `${dayLabel(clip.modified_ms)} ${timeOfDay(clip.modified_ms)}`,
            duration: clock(clip.duration_secs),
            tracks: t("clips.tracks", { count: clip.tracks.length, names: clip.tracks.join(", ") }),
          })}
          aria-keyshortcuts="Enter"
          title={t("clips.openHint")}
          onClick={(event) => {
            if (event.detail === 0) onOpen();
          }}
          onDoubleClick={onOpen}
          className="absolute inset-0 rounded-md"
        />
      )}
    </li>
  );
}

export function RecentClips({
  clips,
  appOrder,
  labels,
  freshClip,
  saveKeys,
  onOpen,
}: {
  clips: ClipSummary[];
  appOrder: string[];
  labels: TrackLabels | null;
  freshClip: string | null;
  saveKeys: string | null;
  onOpen: (path: string) => void;
}) {
  const t = useT();
  const [beforeKeys, afterKeys] = t("clips.emptyWithKeys").split("{keys}");
  return (
    <section
      aria-labelledby="clips-title"
      className="flex min-w-0 flex-col gap-3 overflow-y-auto px-5 pt-5 pb-6 max-[1000px]:overflow-visible"
    >
      <h2 id="clips-title" className="text-[17px] font-semibold">
        {t("clips.title")}
      </h2>
      {clips.length === 0 ? (
        <div className="rounded-md border border-dashed border-line-strong px-4 py-6 text-ink-2">
          <p className="font-semibold text-ink">{t("clips.emptyTitle")}</p>
          <p className="mt-1">
            {saveKeys ? (
              <>
                {beforeKeys}
                <kbd className="score font-semibold text-ink">{saveKeys}</kbd>
                {afterKeys}
              </>
            ) : (
              t("clips.emptyNoKeys")
            )}
          </p>
        </div>
      ) : (
        <ul className="grid grid-cols-1 gap-3 max-[1000px]:grid-cols-[repeat(auto-fill,minmax(220px,1fr))]">
          {clips.map((clip) => (
            <ClipCard
              key={clip.path}
              clip={clip}
              appOrder={appOrder}
              labels={labels}
              fresh={clip.path === freshClip}
              onOpen={() => onOpen(clip.path)}
            />
          ))}
        </ul>
      )}
    </section>
  );
}
