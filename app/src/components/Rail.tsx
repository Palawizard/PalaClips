import type { LucideIcon } from "lucide-react";
import { Clapperboard, Gauge, SlidersHorizontal } from "lucide-react";
import { useT } from "../lib/i18n.ts";

export type View = "status" | "gallery" | "settings";

const ITEMS: { view: View; icon: LucideIcon }[] = [
  { view: "status", icon: Gauge },
  { view: "gallery", icon: Clapperboard },
  { view: "settings", icon: SlidersHorizontal },
];

function Mark() {
  const t = useT();
  return (
    <svg viewBox="200 200 624 624" className="mb-5 size-8 text-ink" role="img" aria-label={t("app.name")}>
      <circle cx="512" cy="512" r="276" fill="none" stroke="currentColor" strokeOpacity="0.25" strokeWidth="56" />
      <path d="M512 236 A276 276 0 1 1 268 386" fill="none" stroke="currentColor" strokeWidth="56" strokeLinecap="round" />
      <path
        d="M236 300 L282 404 L388 362"
        fill="none"
        stroke="currentColor"
        strokeWidth="48"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
      <path d="M452 404 L452 620 L628 512 Z" fill="currentColor" />
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
