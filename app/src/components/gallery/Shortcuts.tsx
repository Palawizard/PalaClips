import { Keyboard } from "lucide-react";
import { useEffect, useId, useRef, useState } from "react";
import { useT } from "../../lib/i18n.ts";

export interface Shortcut {
  keys: string[];
  label: string;
}

export function Shortcuts({ shortcuts }: { shortcuts: Shortcut[] }) {
  const t = useT();
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const panelId = useId();

  useEffect(() => {
    if (!open) return;
    const close = (event: PointerEvent) => {
      if (!root.current?.contains(event.target as Node)) setOpen(false);
    };
    window.addEventListener("pointerdown", close);
    return () => window.removeEventListener("pointerdown", close);
  }, [open]);

  return (
    <div
      ref={root}
      className="relative"
      onKeyDown={(event) => {
        if (event.key === "Escape" && open) {
          event.stopPropagation();
          setOpen(false);
          trigger.current?.focus();
        }
      }}
    >
      <button
        ref={trigger}
        type="button"
        aria-expanded={open}
        aria-controls={panelId}
        title={t("shortcuts.title")}
        aria-label={t("shortcuts.title")}
        onClick={() => setOpen((current) => !current)}
        className="pressable flex size-9 items-center justify-center rounded-md border border-line-strong text-ink-2 hover:bg-surface-hover hover:text-ink"
      >
        <Keyboard size={17} aria-hidden />
      </button>
      {open && (
        <div
          id={panelId}
          role="dialog"
          aria-label={t("shortcuts.title")}
          className="clip-menu absolute top-11 right-0 z-30 w-80 rounded-md border border-line-strong bg-surface-2 p-3"
        >
          <dl className="grid grid-cols-[auto_minmax(0,1fr)] items-center gap-x-4 gap-y-2 text-[13px]">
            {shortcuts.map((shortcut) => (
              <div key={shortcut.label} className="contents">
                <dt className="flex flex-wrap gap-1">
                  {shortcut.keys.map((key) => (
                    <kbd
                      key={key}
                      className="score min-w-7 rounded-[4px] bg-surface px-1.5 py-0.5 text-center text-[13px] font-semibold text-ink"
                    >
                      {key}
                    </kbd>
                  ))}
                </dt>
                <dd className="text-ink-2">{shortcut.label}</dd>
              </div>
            ))}
          </dl>
        </div>
      )}
    </div>
  );
}
