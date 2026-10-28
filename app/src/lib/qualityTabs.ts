import type { EngineConfig } from "./ipc.ts";

export const QUALITY_TABS = ["replay", "recording", "common"] as const;
export type QualityTab = (typeof QUALITY_TABS)[number];

const TAB_FIELDS: Record<QualityTab, (config: EngineConfig) => unknown> = {
  replay: ({ replay, video }) => [
    replay.duration_secs,
    replay.after_secs,
    replay.storage,
    video.codec,
    video.quality,
    video.bitrate_kbps,
    video.fps,
    video.resolution,
  ],
  recording: ({ record }) => record.video,
  common: ({ video }) => [video.encoder, video.keyframe_interval_secs, video.gpu_priority],
};

export function changedTabs(saved: EngineConfig, draft: EngineConfig): QualityTab[] {
  return QUALITY_TABS.filter((tab) => JSON.stringify(TAB_FIELDS[tab](saved)) !== JSON.stringify(TAB_FIELDS[tab](draft)));
}

let sessionTab: QualityTab = "replay";

export function rememberedTab(): QualityTab {
  return sessionTab;
}

export function rememberTab(tab: QualityTab) {
  sessionTab = tab;
}

export function nextTab(current: QualityTab, key: string): QualityTab | null {
  const index = QUALITY_TABS.indexOf(current);
  const count = QUALITY_TABS.length;
  const steps: Record<string, number> = { ArrowRight: index + 1, ArrowLeft: index - 1 + count, Home: 0, End: count - 1 };
  const target = steps[key];
  return target === undefined ? null : (QUALITY_TABS[target % count] ?? null);
}
