import type { LucideIcon } from "lucide-react";
import { Clapperboard, Gauge, SlidersHorizontal } from "lucide-react";
import logo from "../../src-tauri/icons/app-icon.svg";

export type View = "status" | "gallery" | "settings";

const ITEMS: { view: View; label: string; icon: LucideIcon }[] = [
  { view: "status", label: "État", icon: Gauge },
  { view: "gallery", label: "Galerie", icon: Clapperboard },
  { view: "settings", label: "Réglages", icon: SlidersHorizontal },
];

export function Rail({ view, onChange }: { view: View; onChange: (view: View) => void }) {
  return (
    <nav aria-label="Navigation" className="flex w-[84px] flex-col items-center gap-1 border-r border-line bg-rail py-4">
      <img src={logo} alt="PalaClips" className="mb-5 size-9" draggable={false} />
      {ITEMS.map(({ view: target, label, icon: Icon }) => {
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
            {label}
          </button>
        );
      })}
    </nav>
  );
}
