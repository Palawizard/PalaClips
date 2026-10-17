import { type KeyboardEvent, useEffect, useState } from "react";
import {
  type HotkeyAction,
  type Hotkeys,
  type HotkeysView,
  getHotkeys,
  resumeHotkeys,
  setHotkeys,
  suspendHotkeys,
} from "../../lib/ipc.ts";
import { SettingRow } from "./Settings.tsx";

const ACTIONS: { action: HotkeyAction; label: string }[] = [
  { action: "save_replay", label: "Sauvegarder le replay" },
  { action: "toggle_replay", label: "Armer ou désarmer le replay" },
];

const MODIFIER_KEYS = new Set(["Control", "Alt", "Shift", "Meta", "AltGraph", "OS"]);

function displayKey(part: string): string {
  if (part === "Shift") return "Maj";
  if (part === "Super") return "Win";
  return part;
}

function combination(event: KeyboardEvent): string {
  const parts: string[] = [];
  if (event.ctrlKey) parts.push("Ctrl");
  if (event.altKey) parts.push("Alt");
  if (event.shiftKey) parts.push("Shift");
  if (event.metaKey) parts.push("Super");
  const letter = /^[a-z]$/i.test(event.key) && (event.code === "" || /^Key[A-Z]$/.test(event.code));
  parts.push(letter ? event.key.toUpperCase() : event.code || event.key);
  return parts.join("+");
}

function Keys({ keys }: { keys: string }) {
  return (
    <span className="flex items-center gap-1">
      {keys.split("+").map((part) => (
        <kbd
          key={part}
          className="score min-w-7 rounded-[4px] border border-line-strong bg-surface-2 px-1.5 py-0.5 text-center text-[15px] font-semibold text-ink"
        >
          {displayKey(part)}
        </kbd>
      ))}
    </span>
  );
}

function HotkeyField({
  label,
  keys,
  listening,
  onListen,
  onCapture,
  onClear,
  onCancel,
}: {
  label: string;
  keys: string | null;
  listening: boolean;
  onListen: () => void;
  onCapture: (keys: string) => void;
  onClear: () => void;
  onCancel: () => void;
}) {
  const handleKey = (event: KeyboardEvent<HTMLButtonElement>) => {
    if (!listening) return;
    event.preventDefault();
    event.stopPropagation();
    if (MODIFIER_KEYS.has(event.key)) return;
    const plain = !event.ctrlKey && !event.altKey && !event.shiftKey && !event.metaKey;
    if (plain && event.key === "Escape") onCancel();
    else if (plain && (event.key === "Backspace" || event.key === "Delete")) onClear();
    else onCapture(combination(event));
  };

  return (
    <button
      type="button"
      aria-label={`${label} : ${keys ?? "aucun raccourci"}. ${listening ? "Appuie sur la combinaison, Échap pour annuler, Retour arrière pour retirer." : "Modifier"}`}
      onClick={listening ? undefined : onListen}
      onKeyDown={handleKey}
      onBlur={listening ? onCancel : undefined}
      data-listening={listening}
      className="hotkey-field pressable flex h-10 min-w-56 items-center justify-center rounded-md border border-line-strong bg-surface px-3 hover:border-ink-3"
    >
      {listening ? (
        <span className="text-[13px] font-medium text-ink">Appuie sur la combinaison…</span>
      ) : keys ? (
        <Keys keys={keys} />
      ) : (
        <span className="text-[13px] text-ink-3">Aucun raccourci</span>
      )}
    </button>
  );
}

export function HotkeysSection() {
  const [view, setView] = useState<HotkeysView | null>(null);
  const [listening, setListening] = useState<HotkeyAction | null>(null);
  const [errors, setErrors] = useState<Partial<Record<HotkeyAction, string>>>({});

  const refresh = () => getHotkeys().then(setView, () => setView(null));

  useEffect(() => {
    refresh();
    return () => {
      resumeHotkeys();
    };
  }, []);

  const listen = (action: HotkeyAction) => {
    setErrors((current) => ({ ...current, [action]: undefined }));
    setListening(action);
    suspendHotkeys();
  };

  const cancel = () => {
    setListening(null);
    resumeHotkeys();
  };

  const save = (action: HotkeyAction, keys: string | null) => {
    if (!view) return;
    setListening(null);
    const wanted: Hotkeys = { ...view.hotkeys, [action]: keys };
    setHotkeys(wanted)
      .then(() => setErrors((current) => ({ ...current, [action]: undefined })))
      .catch((error) => setErrors((current) => ({ ...current, [action]: String(error) })))
      .finally(refresh);
  };

  if (!view) return null;
  const conflicts = view.unavailable.length > 0;

  return (
    <>
      {ACTIONS.map(({ action, label }) => {
        const unavailable = view.unavailable.includes(action);
        return (
          <SettingRow
            key={action}
            label={label}
            help={
              listening === action
                ? "Si rien ne s'affiche, la combinaison est déjà prise par une autre application. Échap annule, Retour arrière retire le raccourci."
                : undefined
            }
            error={
              errors[action] ??
              (unavailable ? "Déjà pris par une autre application : choisis une autre combinaison." : null)
            }
          >
            <HotkeyField
              label={label}
              keys={view.hotkeys[action]}
              listening={listening === action}
              onListen={() => listen(action)}
              onCapture={(keys) => save(action, keys)}
              onClear={() => save(action, null)}
              onCancel={cancel}
            />
          </SettingRow>
        );
      })}
      <div className="max-w-[78ch] px-6 py-5 text-[13px] text-ink-2">
        <p className={`font-semibold ${conflicts ? "text-warn" : "text-ink"}`}>
          {conflicts ? "Conflit détecté avec une autre application." : "Aucun conflit détecté."}
        </p>
        <p className="mt-1">
          Discord et l'overlay NVIDIA lisent le clavier sans déclarer leurs raccourcis à Windows : vérifie-les
          (Discord : Paramètres → Raccourcis ; NVIDIA : Alt+Z → Paramètres → Raccourcis clavier).
        </p>
      </div>
    </>
  );
}
