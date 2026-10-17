import { CircleAlert, Info, TriangleAlert, X } from "lucide-react";
import { useRef, useState } from "react";
import type { ActiveAlert } from "../lib/ipc.ts";

const ICONS = { info: Info, warning: TriangleAlert, error: CircleAlert };
const TONE = { info: "text-ink-2", warning: "text-warn", error: "text-warn" };

export function Alerts({ alerts }: { alerts: ActiveAlert[] }) {
  const [dismissed, setDismissed] = useState<Record<string, string>>({});
  const list = useRef<HTMLUListElement>(null);
  const visible = alerts.filter((alert) => dismissed[alert.code] !== alert.message);
  if (visible.length === 0) return null;

  const dismiss = (alert: ActiveAlert, index: number) => {
    setDismissed((current) => ({ ...current, [alert.code]: alert.message }));
    const buttons = list.current?.querySelectorAll<HTMLButtonElement>("button");
    const next = buttons?.[index + 1] ?? buttons?.[index - 1];
    next?.focus();
  };

  return (
    <ul ref={list} aria-label="Alertes" aria-live="polite" className="flex flex-col border-b border-line">
      {visible.map((alert, index) => {
        const Icon = ICONS[alert.severity];
        return (
          <li
            key={alert.code}
            className="alert-item flex items-start gap-3 border-b border-line bg-surface-2 px-5 py-3 last:border-b-0"
          >
            <Icon size={18} className={`mt-0.5 shrink-0 ${TONE[alert.severity]}`} aria-hidden />
            <p className="max-w-[90ch] flex-1 text-ink">{alert.message}</p>
            <button
              type="button"
              onClick={() => dismiss(alert, index)}
              className="pressable -m-1 rounded-md p-1 text-ink-3 hover:bg-surface-hover hover:text-ink"
              aria-label="Masquer cette alerte"
            >
              <X size={16} aria-hidden />
            </button>
          </li>
        );
      })}
    </ul>
  );
}
