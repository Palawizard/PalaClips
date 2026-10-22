import { ChevronDown } from "lucide-react";
import { type ReactNode, useEffect, useId, useState } from "react";
import { number, useT } from "../lib/i18n.ts";
import {
  type ActiveAlert,
  CPU_ENCODER_CODECS,
  type PerformanceStats,
  onEngineEvent,
  setPerformancePanel,
} from "../lib/ipc.ts";
import type { ReplayPhase } from "../lib/useEngine.ts";

const OPEN_KEY = "palaclips.performance.open";

function readOpen(): boolean {
  try {
    return window.localStorage.getItem(OPEN_KEY) === "1";
  } catch {
    return false;
  }
}

function saveOpen(open: boolean) {
  try {
    window.localStorage.setItem(OPEN_KEY, open ? "1" : "0");
  } catch {}
}

function lacksEncoder(alerts: ActiveAlert[]): boolean {
  return alerts.some(
    (alert) =>
      ["no_hardware_encoder", "no_video_encoder"].some(
        (code) => alert.code === code || JSON.stringify(alert.params ?? {}).includes(`"${code}"`),
      ),
  );
}

function Cell({ title, tone, children }: { title: string; tone?: "warn" | "muted"; children: ReactNode }) {
  return (
    <section aria-label={title} className="flex min-w-0 flex-1 basis-[180px] flex-col gap-2 px-5 py-4">
      <h3
        className={`truncate text-[15px] font-semibold ${
          tone === "warn" ? "text-warn" : tone === "muted" ? "text-ink-2" : "text-ink"
        }`}
        title={title}
      >
        {title}
      </h3>
      {children}
    </section>
  );
}

function Figure({ value, unit, idle }: { value: string; unit: string; idle?: boolean }) {
  return (
    <p className="flex items-baseline gap-1.5">
      <span className={`score text-[20px] leading-none font-medium ${idle ? "text-ink-3" : "text-ink"}`}>{value}</span>
      <span className="text-[13px] text-ink-2">{unit}</span>
    </p>
  );
}

function Empty() {
  return <p className="score text-[20px] leading-none font-medium text-ink-3">—</p>;
}

function Detail({ children }: { children: ReactNode }) {
  return <p className="text-[13px] text-ink-2">{children}</p>;
}

export function PerformancePanel({ phase, alerts }: { phase: ReplayPhase; alerts: ActiveAlert[] }) {
  const t = useT();
  const regionId = useId();
  const [open, setOpen] = useState(readOpen);
  const [stats, setStats] = useState<PerformanceStats | null>(null);

  useEffect(() => {
    if (!open) return;
    let active = true;
    const listening = onEngineEvent((event) => {
      if (active && event.type === "performance_stats") setStats(event);
    });
    setPerformancePanel(true).catch(() => undefined);
    return () => {
      active = false;
      listening.then((stop) => stop());
      setPerformancePanel(false).catch(() => undefined);
      setStats(null);
    };
  }, [open]);

  const toggle = () => {
    setOpen((current) => {
      saveOpen(!current);
      return !current;
    });
  };

  const capture = stats?.capture ?? null;
  const percent = (value: number | null | undefined, digits = 1) =>
    value === null || value === undefined ? "—" : number(value, digits);
  const megabytes = (value: number | null | undefined) =>
    value === null || value === undefined ? "—" : number(value / 1e6);
  const cpuCodec = capture ? CPU_ENCODER_CODECS[capture.encoder] : undefined;
  const captureTitle = capture
    ? cpuCodec
      ? t("performance.cpuEncoder", { codec: t(`codecs.${cpuCodec}`) })
      : capture.encoder
    : lacksEncoder(alerts)
      ? t("performance.noEncoder")
      : t(`status.phase.${phase}`);

  return (
    <section aria-labelledby={`${regionId}-title`} className="border-t border-line">
      <h2 id={`${regionId}-title`} className="text-[17px] font-semibold">
        <button
          type="button"
          aria-expanded={open}
          aria-controls={regionId}
          onClick={toggle}
          className="pressable flex w-full items-center gap-2 px-5 pt-4 pb-3 text-left hover:bg-surface-hover"
        >
          {t("performance.title")}
          <ChevronDown size={18} aria-hidden className={`export-chevron text-ink-3 ${open ? "rotate-180" : ""}`} />
        </button>
      </h2>
      {open && (
        <div
          id={regionId}
          className="performance-cells flex flex-wrap border-t border-line [&>section+section]:border-l [&>section+section]:border-line"
        >
          <Cell title={t("performance.engine")}>
            <Figure value={percent(stats?.cpu_percent, 2)} unit={t("performance.cpu")} idle={!stats} />
            {stats?.cpu_max_percent != null && (
              <Detail>{t("performance.max", { value: t("units.percent", { value: percent(stats.cpu_max_percent, 2) }) })}</Detail>
            )}
            <Figure value={megabytes(stats?.engine_bytes)} unit={t("performance.ramOutsideBuffer")} idle={!stats} />
          </Cell>
          <Cell title={t("performance.buffer")} tone={capture ? undefined : "muted"}>
            {capture ? (
              <>
                <Figure value={megabytes(stats?.buffer_bytes)} unit={t("units.mbUnit")} />
                <Detail>{t("units.seconds", { value: number(stats?.buffered_secs ?? 0, 1) })}</Detail>
              </>
            ) : (
              <Empty />
            )}
          </Cell>
          <Cell
            title={captureTitle}
            tone={capture ? undefined : lacksEncoder(alerts) ? "warn" : "muted"}
          >
            {capture ? (
              <>
                <p className="score truncate text-[17px] font-medium whitespace-nowrap text-ink-2">
                  {`${capture.width}×${capture.height} · ${t("units.fps", { value: capture.fps })}`}
                </p>
                <Detail>{t("performance.lag", { value: t("units.ms", { value: number(capture.lag_max_ms, 1) }) })}</Detail>
              </>
            ) : (
              <Empty />
            )}
          </Cell>
          <Cell title={t("performance.frames")} tone={capture ? undefined : "muted"}>
            {capture ? (
              <>
                <Figure
                  value={t("units.percent", { value: percent(capture.repeated_percent) })}
                  unit={t("performance.repeated")}
                />
                <Figure value={number(capture.late_total)} unit={t("performance.late")} />
                <Detail>{t("performance.lastMinute", { count: capture.late_last_minute })}</Detail>
              </>
            ) : (
              <Empty />
            )}
          </Cell>
        </div>
      )}
    </section>
  );
}
