import { useMemo, useState } from "react";
import { Alerts } from "./components/Alerts.tsx";
import { Rail, type View } from "./components/Rail.tsx";
import { RecentClips } from "./components/RecentClips.tsx";
import { Roster } from "./components/Roster.tsx";
import { StatusBanner } from "./components/StatusBanner.tsx";
import { useEngine } from "./lib/useEngine.ts";

function Pending({ title, text }: { title: string; text: string }) {
  return (
    <section className="max-w-[60ch] px-8 py-10">
      <h1 className="text-[22px] font-semibold">{title}</h1>
      <p className="mt-2 text-ink-2">{text}</p>
    </section>
  );
}

export default function App() {
  const [view, setView] = useState<View>("status");
  const engine = useEngine();
  const appOrder = useMemo(
    () => engine.config?.audio.app_tracks.filter((track) => track.enabled).map((track) => track.label) ?? [],
    [engine.config],
  );

  return (
    <div className="grid h-full grid-cols-[auto_minmax(0,1fr)]">
      <Rail view={view} onChange={setView} />
      <main className="flex min-w-0 flex-col overflow-y-auto">
        {view === "status" && (
          <>
            <h1 className="sr-only">État de PalaClips</h1>
            <StatusBanner engine={engine} />
            {engine.actionError && (
              <p role="alert" className="border-b border-line bg-surface-2 px-5 py-3 text-warn">
                {engine.actionError}
              </p>
            )}
            <Alerts alerts={engine.status?.alerts ?? []} />
            <div className="grid flex-1 grid-cols-[minmax(0,1fr)_minmax(260px,320px)] max-[1000px]:grid-cols-1 max-[1000px]:flex-none">
              <Roster tracks={engine.status?.tracks ?? []} appOrder={appOrder} flashAt={engine.savedAt} />
              <RecentClips clips={engine.clips} appOrder={appOrder} freshClip={engine.freshClip} />
            </div>
          </>
        )}
        {view === "gallery" && (
          <Pending title="Galerie" text="La galerie complète, avec le lecteur et la découpe, arrive ici. Tes derniers clips sont déjà sur l'écran État." />
        )}
        {view === "settings" && (
          <Pending title="Réglages" text="Capture, audio, replay et qualité, raccourcis, stockage : les réglages arrivent ici." />
        )}
      </main>
    </div>
  );
}
