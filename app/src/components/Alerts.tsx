import { CircleAlert, Info, TriangleAlert, X } from "lucide-react";
import { useRef, useState } from "react";
import type { Destination } from "../App.tsx";
import { type Key, message, useT } from "../lib/i18n.ts";
import type { ActiveAlert } from "../lib/ipc.ts";

const ICONS = { info: Info, warning: TriangleAlert, error: CircleAlert };
const TONE = { info: "text-ink-2", warning: "text-warn", error: "text-warn" };
const FOCUS_FALLBACK = "roster-title";

const CAPTURE: Destination = { view: "capture" };
const AUDIO: Destination = { view: "audio" };
const REPLAY: Destination = { view: "settings", section: "replay" };
const FIXED_IN: Record<string, Destination> = {
  capture_fallback_monitor: CAPTURE,
  captured_window_closed: CAPTURE,
  window_missing: CAPTURE,
  monitor_missing: CAPTURE,
  primary_monitor_missing: CAPTURE,
  app_track_silent: AUDIO,
  audio_device_unavailable: AUDIO,
  no_audio_device: AUDIO,
  device_in_exclusive_use: AUDIO,
  cpu_encoder_overloaded: REPLAY,
  capture_gpu_saturated: REPLAY,
  no_hardware_encoder: REPLAY,
  codec_unavailable: REPLAY,
  hotkey_unavailable: { view: "settings", section: "hotkeys" },
  hotkeys_taken: { view: "settings", section: "hotkeys" },
  update_available: { view: "settings", section: "general" },
};

function destinationName(destination: Destination): Key {
  return destination.section ? `settings.sections.${destination.section}` : `nav.${destination.view}`;
}

export function Alerts({ alerts, onOpen }: { alerts: ActiveAlert[]; onOpen: (destination: Destination) => void }) {
  const t = useT();
  const [dismissed, setDismissed] = useState<Record<string, string>>({});
  const list = useRef<HTMLUListElement>(null);
  const visible = alerts.filter((alert) => dismissed[alert.code] !== alert.message);

  const dismiss = (alert: ActiveAlert, index: number) => {
    setDismissed((current) => ({ ...current, [alert.code]: alert.message }));
    const buttons = list.current?.querySelectorAll<HTMLButtonElement>("button[data-dismiss]");
    const next = buttons?.[index + 1] ?? buttons?.[index - 1] ?? document.getElementById(FOCUS_FALLBACK);
    next?.focus();
  };

  return (
    <ul
      ref={list}
      aria-label={t("alerts.label")}
      aria-live="assertive"
      className={visible.length > 0 ? "flex flex-col border-b border-line" : "sr-only"}
    >
      {visible.map((alert, index) => {
        const Icon = ICONS[alert.severity];
        const fix = FIXED_IN[alert.code];
        return (
          <li
            key={alert.code}
            className="alert-item flex flex-wrap items-start gap-3 border-b border-line bg-surface-2 px-5 py-3 last:border-b-0"
          >
            <Icon size={18} className={`mt-0.5 shrink-0 ${TONE[alert.severity]}`} aria-hidden />
            <p className="max-w-[90ch] min-w-[240px] flex-1 text-ink">{message(alert)}</p>
            {fix && (
              <button
                type="button"
                onClick={() => onOpen(fix)}
                className="pressable -my-1 h-8 shrink-0 rounded-md border border-line-strong px-2.5 text-[13px] font-medium text-ink hover:bg-surface-hover"
              >
                {t("alerts.open", { page: t(destinationName(fix)) })}
              </button>
            )}
            <button
              type="button"
              data-dismiss
              onClick={() => dismiss(alert, index)}
              className="pressable -m-1 rounded-md p-1 text-ink-3 hover:bg-surface-hover hover:text-ink"
              aria-label={t("alerts.dismiss")}
            >
              <X size={16} aria-hidden />
            </button>
          </li>
        );
      })}
    </ul>
  );
}
