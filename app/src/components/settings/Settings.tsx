import { type KeyboardEvent, type ReactNode, useRef, useState } from "react";
import { GeneralSection } from "./GeneralSection.tsx";
import { HotkeysSection } from "./HotkeysSection.tsx";

type SectionId = "capture" | "audio" | "replay" | "hotkeys" | "storage" | "general";

const SECTIONS: { id: SectionId; label: string }[] = [
  { id: "capture", label: "Capture" },
  { id: "audio", label: "Audio" },
  { id: "replay", label: "Replay & qualité" },
  { id: "hotkeys", label: "Raccourcis" },
  { id: "storage", label: "Stockage" },
  { id: "general", label: "Général" },
];

export function SettingRow({
  label,
  help,
  error,
  children,
}: {
  label: string;
  help?: ReactNode;
  error?: string | null;
  children: ReactNode;
}) {
  return (
    <div className="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-x-6 gap-y-1 border-b border-line px-6 py-4">
      <div className="min-w-0">
        <p className="text-[15px] font-semibold">{label}</p>
        {help && <p className="mt-0.5 max-w-[62ch] text-[13px] text-ink-2">{help}</p>}
      </div>
      <div className="justify-self-end">{children}</div>
      {error && (
        <p role="alert" className="col-span-2 text-[13px] text-warn">
          {error}
        </p>
      )}
    </div>
  );
}

export function Switch({
  checked,
  label,
  onChange,
}: {
  checked: boolean;
  label: string;
  onChange: (checked: boolean) => void;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      onClick={() => onChange(!checked)}
      className={`pressable relative h-6 w-11 rounded-full border ${
        checked ? "border-ink bg-ink" : "border-line-strong bg-surface-2"
      }`}
    >
      <span
        aria-hidden
        className={`switch-thumb absolute top-[3px] left-[3px] size-4 rounded-full ${
          checked ? "translate-x-5 bg-ground" : "bg-ink-3"
        }`}
      />
    </button>
  );
}

export function Segmented<T extends string>({
  value,
  options,
  label,
  onChange,
}: {
  value: T;
  options: { value: T; label: string }[];
  label: string;
  onChange: (value: T) => void;
}) {
  const buttons = useRef<(HTMLButtonElement | null)[]>([]);
  const move = (event: KeyboardEvent, index: number) => {
    const step = event.key === "ArrowRight" || event.key === "ArrowDown" ? 1 : event.key === "ArrowLeft" || event.key === "ArrowUp" ? -1 : 0;
    if (!step) return;
    event.preventDefault();
    const next = (index + step + options.length) % options.length;
    const target = options[next];
    if (!target) return;
    onChange(target.value);
    buttons.current[next]?.focus();
  };
  return (
    <div role="radiogroup" aria-label={label} className="flex rounded-md border border-line-strong p-0.5">
      {options.map((option, index) => {
        const selected = option.value === value;
        return (
          <button
            key={option.value}
            ref={(element) => {
              buttons.current[index] = element;
            }}
            type="button"
            role="radio"
            aria-checked={selected}
            tabIndex={selected ? 0 : -1}
            onClick={() => onChange(option.value)}
            onKeyDown={(event) => move(event, index)}
            className={`pressable rounded-[4px] px-3 py-1.5 text-[13px] font-medium ${
              selected ? "bg-ink text-ground" : "text-ink-2 hover:text-ink"
            }`}
          >
            {option.label}
          </button>
        );
      })}
    </div>
  );
}

function Pending({ title }: { title: string }) {
  return (
    <div className="px-6 py-6 text-ink-2">
      <p>Section en préparation.</p>
      <p className="sr-only">{title}</p>
    </div>
  );
}

export function Settings() {
  const [section, setSection] = useState<SectionId>("hotkeys");
  const label = SECTIONS.find((item) => item.id === section)?.label ?? "";

  return (
    <div className="grid min-h-full grid-cols-[200px_minmax(0,1fr)]">
      <nav aria-label="Sections des réglages" className="border-r border-line bg-surface py-4">
        <h1 className="px-5 pb-3 text-[17px] font-semibold">Réglages</h1>
        <ul className="flex flex-col gap-0.5 px-2">
          {SECTIONS.map((item) => {
            const active = item.id === section;
            return (
              <li key={item.id}>
                <button
                  type="button"
                  onClick={() => setSection(item.id)}
                  aria-current={active ? "page" : undefined}
                  className={`pressable w-full rounded-md px-3 py-2 text-left text-[14px] ${
                    active ? "bg-surface-2 font-semibold text-ink" : "text-ink-2 hover:bg-surface-hover hover:text-ink"
                  }`}
                >
                  {item.label}
                </button>
              </li>
            );
          })}
        </ul>
      </nav>
      <section aria-labelledby="settings-section-title" className="min-w-0">
        <h2 id="settings-section-title" className="border-b border-line px-6 pt-6 pb-4 text-[22px] font-semibold">
          {label}
        </h2>
        {section === "hotkeys" && <HotkeysSection />}
        {section === "general" && <GeneralSection />}
        {section !== "hotkeys" && section !== "general" && <Pending title={label} />}
      </section>
    </div>
  );
}
