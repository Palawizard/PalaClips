import { useMemo, useState } from "react";
import { Alerts } from "./components/Alerts.tsx";
import { Rail, type View } from "./components/Rail.tsx";
import { RecentClips } from "./components/RecentClips.tsx";
import { Roster } from "./components/Roster.tsx";
import { Settings } from "./components/settings/Settings.tsx";
import { StatusBanner } from "./components/StatusBanner.tsx";
import { useT } from "./lib/i18n.ts";
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
  const t = useT();
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
            <h1 className="sr-only">{t("status.title")}</h1>
            <StatusBanner engine={engine} />
            {engine.actionError && (
              <p role="alert" className="border-b border-line bg-surface-2 px-5 py-3 text-warn">
                {engine.actionError}
              </p>
            )}
            <Alerts alerts={engine.status?.alerts ?? []} />
            <div className="grid flex-1 grid-cols-[minmax(0,1fr)_minmax(260px,320px)] max-[1000px]:grid-cols-1 max-[1000px]:flex-none">
              <Roster tracks={engine.status?.tracks ?? []} appOrder={appOrder} flashAt={engine.savedAt} />
              <RecentClips
                clips={engine.clips}
                appOrder={appOrder}
                labels={engine.config?.audio ?? null}
                freshClip={engine.freshClip}
                saveKeys={engine.saveKeys}
              />
            </div>
          </>
        )}
        {view === "gallery" && (
          <Pending title={t("nav.gallery")} text={t("gallery.pending")} />
        )}
        {view === "settings" && <Settings status={engine.status} />}
      </main>
    </div>
  );
}
