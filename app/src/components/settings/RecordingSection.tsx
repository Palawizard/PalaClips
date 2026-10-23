import { useRef } from "react";
import { useT } from "../../lib/i18n.ts";
import type { RecordSplit } from "../../lib/ipc.ts";
import { NumberField, Segmented, SettingRow, Switch } from "./Settings.tsx";
import { ConfigFooter, LoadError, useConfigDraft } from "./configDraft.tsx";

type SplitKind = RecordSplit["kind"];

const SPLITS: SplitKind[] = ["never", "duration", "size"];
const DEFAULT_MINUTES = 30;
const DEFAULT_MEGABYTES = 4_000;

function splitOf(kind: SplitKind, current: RecordSplit): RecordSplit {
  if (kind === current.kind) return current;
  if (kind === "duration") return { kind, minutes: DEFAULT_MINUTES };
  if (kind === "size") return { kind, megabytes: DEFAULT_MEGABYTES };
  return { kind };
}

export function RecordingSection() {
  const t = useT();
  const { config, loadError } = useConfigDraft("settings.sections.recording");
  const footerFocus = useRef<HTMLDivElement>(null);
  if (!config) return <LoadError message={loadError} />;
  const { draft } = config;
  const record = draft.record;
  const setRecord = (patch: Partial<typeof record>) => config.change({ ...draft, record: { ...record, ...patch } });

  return (
    <div ref={footerFocus} tabIndex={-1} className="outline-none">
      <SettingRow label={t("settings.recording.includeReplay")}>
        <Switch
          checked={record.include_replay}
          label={t("settings.recording.includeReplay")}
          onChange={(include_replay) => setRecord({ include_replay })}
        />
      </SettingRow>
      <SettingRow label={t("settings.recording.split")}>
        <span className="flex items-center gap-2">
          <Segmented<SplitKind>
            label={t("settings.recording.split")}
            value={record.split.kind}
            options={SPLITS.map((value) => ({ value, label: t(`settings.recording.splits.${value}`) }))}
            onChange={(kind) => setRecord({ split: splitOf(kind, record.split) })}
          />
          {record.split.kind === "duration" && (
            <NumberField
              value={record.split.minutes}
              min={1}
              max={600}
              unit={t("units.minUnit")}
              label={t("settings.recording.splitMinutes")}
              onCommit={(minutes) => setRecord({ split: { kind: "duration", minutes } })}
            />
          )}
          {record.split.kind === "size" && (
            <NumberField
              value={record.split.megabytes}
              min={100}
              max={1_000_000}
              unit={t("units.mbUnit")}
              label={t("settings.recording.splitMegabytes")}
              onCommit={(megabytes) => setRecord({ split: { kind: "size", megabytes } })}
            />
          )}
        </span>
      </SettingRow>
      <ConfigFooter config={config} focusAfter={footerFocus} />
    </div>
  );
}
