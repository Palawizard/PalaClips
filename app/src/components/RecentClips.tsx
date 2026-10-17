import { convertFileSrc } from "@tauri-apps/api/core";
import { Film } from "lucide-react";
import type { ClipSummary } from "../lib/ipc.ts";
import { clock, relativeTime } from "../lib/format.ts";
import { trackColor } from "../lib/tracks.ts";

function ClipCard({ clip, appOrder, fresh }: { clip: ClipSummary; appOrder: string[]; fresh: boolean }) {
  return (
    <li
      data-fresh={fresh}
      className="clip-card overflow-hidden rounded-md border border-line bg-surface"
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
            {clip.game || "Clip"}
          </p>
          <p className="shrink-0 text-[12px] text-ink-3">{relativeTime(clip.modified_ms)}</p>
        </div>
        <ul aria-label={`${clip.tracks.length} pistes : ${clip.tracks.join(", ")}`} className="flex flex-wrap gap-1">
          {clip.tracks.map((title) => (
            <li
              key={title}
              title={title}
              className="h-1.5 w-5 rounded-full"
              style={{ background: trackColor(title, appOrder) }}
            />
          ))}
        </ul>
      </div>
    </li>
  );
}

export function RecentClips({
  clips,
  appOrder,
  freshClip,
  saveKeys,
}: {
  clips: ClipSummary[];
  appOrder: string[];
  freshClip: string | null;
  saveKeys: string | null;
}) {
  return (
    <section aria-labelledby="clips-title" className="flex min-w-0 flex-col gap-3 px-5 pt-5 pb-6">
      <h2 id="clips-title" className="text-[17px] font-semibold">
        Derniers clips
      </h2>
      {clips.length === 0 ? (
        <div className="rounded-md border border-dashed border-line-strong px-4 py-6 text-ink-2">
          <p className="font-semibold text-ink">Aucun clip pour l'instant</p>
          <p className="mt-1">
            {saveKeys ? (
              <>
                Appuie sur <kbd className="score font-semibold text-ink">{saveKeys}</kbd> en jeu pour sauvegarder
                les dernières secondes.
              </>
            ) : (
              "Choisis un raccourci de sauvegarde dans Réglages → Raccourcis."
            )}
          </p>
        </div>
      ) : (
        <ul className="grid grid-cols-1 gap-3 max-[1000px]:grid-cols-[repeat(auto-fill,minmax(220px,1fr))]">
          {clips.map((clip) => (
            <ClipCard key={clip.path} clip={clip} appOrder={appOrder} fresh={clip.path === freshClip} />
          ))}
        </ul>
      )}
    </section>
  );
}
