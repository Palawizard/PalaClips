import { useEffect, useMemo, useState } from "react";
import { Alerts } from "./components/Alerts.tsx";
import { Gallery } from "./components/gallery/Gallery.tsx";
import { LeaveGuardProvider, useLeaveGuard } from "./components/LeaveDialog.tsx";
import { Onboarding } from "./components/onboarding/Onboarding.tsx";
import { PerformancePanel } from "./components/PerformancePanel.tsx";
import { Rail, type View } from "./components/Rail.tsx";
import { RecentClips } from "./components/RecentClips.tsx";
import { Roster } from "./components/Roster.tsx";
import { AudioSection } from "./components/settings/AudioSection.tsx";
import { CaptureSection } from "./components/settings/CaptureSection.tsx";
import { type SectionId, Settings } from "./components/settings/Settings.tsx";
import { StatusBanner } from "./components/StatusBanner.tsx";
import { useT } from "./lib/i18n.ts";
import { onboardingNeeded } from "./lib/ipc.ts";
import { useEngine } from "./lib/useEngine.ts";

export type Destination = { view: View; section?: SectionId };

export default function App() {
  return (
    <LeaveGuardProvider>
      <Shell />
    </LeaveGuardProvider>
  );
}

function PageTitle({ children }: { children: string }) {
  return <h1 className="border-b border-line px-5 pt-6 pb-4 text-[22px] font-semibold">{children}</h1>;
}

function Shell() {
  const t = useT();
  const guard = useLeaveGuard();
  const [view, setView] = useState<View>("status");
  const [section, setSection] = useState<SectionId>("replay");
  const engine = useEngine();
  const [onboarding, setOnboarding] = useState<boolean | null>(null);

  useEffect(() => {
    onboardingNeeded().then(setOnboarding, () => setOnboarding(false));
  }, []);
  const appOrder = useMemo(
    () => engine.config?.audio.app_tracks.filter((track) => track.enabled).map((track) => track.label) ?? [],
    [engine.config],
  );

  const open = ({ view: nextView, section: nextSection }: Destination) => {
    if (nextView === view && (!nextSection || nextSection === section)) return;
    guard.leave(() => {
      setView(nextView);
      if (nextSection) setSection(nextSection);
    });
  };

  if (onboarding === null) return null;
  if (onboarding)
    return (
      <Onboarding
        engine={engine}
        onDone={(next) => {
          setOnboarding(false);
          setView(next);
        }}
      />
    );

  return (
    <div className="grid h-full grid-cols-[auto_minmax(0,1fr)]">
      <Rail view={view} onChange={(next) => open({ view: next })} />
      <main className="flex min-w-0 flex-col overflow-y-auto">
        {view === "status" && (
          <>
            <h1 className="sr-only">{t("status.title")}</h1>
            <StatusBanner engine={engine} onOpen={open} />
            {engine.actionError && (
              <p role="alert" className="border-b border-line bg-surface-2 px-5 py-3 text-warn">
                {engine.actionError}
              </p>
            )}
            <Alerts alerts={engine.status?.alerts ?? []} onOpen={open} />
            <div className="grid flex-1 grid-cols-[minmax(0,1fr)_minmax(260px,320px)] max-[1000px]:grid-cols-1 max-[1000px]:flex-none">
              <div className="flex min-w-0 flex-col border-r border-line max-[1000px]:border-r-0">
                <Roster
                  tracks={engine.status?.tracks ?? []}
                  appOrder={appOrder}
                  flashAt={engine.savedAt}
                  onOpenAudio={() => open({ view: "audio" })}
                />
                <PerformancePanel phase={engine.phase} alerts={engine.status?.alerts ?? []} />
              </div>
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
        {view === "gallery" && <Gallery config={engine.config} saveKeys={engine.saveKeys} />}
        {view === "capture" && (
          <>
            <PageTitle>{t("nav.capture")}</PageTitle>
            <CaptureSection />
          </>
        )}
        {view === "audio" && (
          <>
            <PageTitle>{t("nav.audio")}</PageTitle>
            <AudioSection status={engine.status} />
          </>
        )}
        {view === "settings" && <Settings section={section} onSection={(next) => open({ view: "settings", section: next })} />}
      </main>
    </div>
  );
}
