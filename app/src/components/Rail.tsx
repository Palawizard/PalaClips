import type { LucideIcon } from "lucide-react";
import { AudioLines, Clapperboard, Gauge, Monitor, Shrink, SlidersHorizontal } from "lucide-react";
import { useT } from "../lib/i18n.ts";

export type View = "status" | "gallery" | "compress" | "capture" | "audio" | "settings";

const ITEMS: { view: View; icon: LucideIcon }[] = [
  { view: "status", icon: Gauge },
  { view: "gallery", icon: Clapperboard },
  { view: "capture", icon: Monitor },
  { view: "audio", icon: AudioLines },
  { view: "compress", icon: Shrink },
  { view: "settings", icon: SlidersHorizontal },
];

export function Mark({ className = "mb-5 size-8" }: { className?: string }) {
  const t = useT();
  return (
    <svg viewBox="0 0 64 64" className={`${className} text-ink`} role="img" aria-label={t("app.name")}>
      <g transform="translate(3 0)" fill="currentColor">
        <polygon points="14,10 34.6,21.3 14,21.3" />
        <polygon points="14,26.3 43.7,26.3 54,32 43.7,37.7 14,37.7" />
        <polygon points="14,42.7 34.6,42.7 14,54" />
      </g>
    </svg>
  );
}

export function Rail({ view, onChange }: { view: View; onChange: (view: View) => void }) {
  const t = useT();
  return (
    <nav aria-label={t("nav.label")} className="flex w-[84px] flex-col items-center gap-1 border-r border-line bg-rail py-4">
      <Mark />
      {ITEMS.map(({ view: target, icon: Icon }) => {
        const current = target === view;
        return (
          <button
            key={target}
            type="button"
            onClick={() => onChange(target)}
            aria-current={current ? "page" : undefined}
            className={`pressable flex w-[68px] flex-col items-center gap-1 rounded-md py-2.5 text-[12px] font-medium ${
              current ? "bg-surface-2 text-ink" : "text-ink-3 hover:bg-surface hover:text-ink"
            }`}
          >
            <Icon size={20} strokeWidth={current ? 2.2 : 1.8} aria-hidden />
            {t(`nav.${target}`)}
          </button>
        );
      })}
    </nav>
  );
}
