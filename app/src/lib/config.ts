import type { EngineConfig, RecordVideo } from "./ipc.ts";

export type ConfigPatch = { [key: string]: unknown };

function mergeable(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value) && !("kind" in value);
}

export function configPatch<T extends object>(saved: T, next: T): ConfigPatch {
  const patch: ConfigPatch = {};
  for (const [key, value] of Object.entries(next)) {
    const before = (saved as Record<string, unknown>)[key];
    if (mergeable(before) && mergeable(value)) {
      const inner = configPatch(before, value);
      if (Object.keys(inner).length > 0) patch[key] = inner;
    } else if (JSON.stringify(before) !== JSON.stringify(value)) {
      patch[key] = value;
    }
  }
  return patch;
}

export function withPatch<T extends object>(saved: T, patch: ConfigPatch): T {
  const merged: Record<string, unknown> = { ...(saved as Record<string, unknown>) };
  for (const [key, value] of Object.entries(patch)) {
    const before = merged[key];
    merged[key] = mergeable(before) && mergeable(value) ? withPatch(before, value) : value;
  }
  return merged as T;
}

export interface ConfigStore<T> {
  current: () => T | null;
  ready: () => Promise<T>;
  listen: (listener: (config: T) => void) => () => void;
  publish: (config: T) => void;
}

export function createConfigStore<T>(source: {
  load: () => Promise<T>;
  subscribe: (handler: (config: T) => void) => void;
}): ConfigStore<T> {
  let config: T | null = null;
  let loading: Promise<T> | null = null;
  const listeners = new Set<(config: T) => void>();

  const publish = (next: T) => {
    config = next;
    for (const listener of listeners) listener(next);
  };

  const ready = () => {
    if (!loading) {
      source.subscribe(publish);
      loading = source.load().then((loaded) => {
        if (config === null) publish(loaded);
        return config ?? loaded;
      });
    }
    return loading;
  };

  return {
    current: () => config,
    ready,
    listen: (listener) => {
      listeners.add(listener);
      ready().catch(() => {});
      return () => listeners.delete(listener);
    },
    publish,
  };
}

function audioSources(audio: EngineConfig["audio"]): string {
  return JSON.stringify({
    ...audio,
    full_mix_label: null,
    microphone_label: null,
    game_label: null,
    app_tracks: audio.app_tracks.map((track) => ({ ...track, label: "" })),
  });
}

export function restartsReplay(before: EngineConfig, after: EngineConfig): boolean {
  return (
    JSON.stringify(before.capture) !== JSON.stringify(after.capture) ||
    JSON.stringify(before.video) !== JSON.stringify(after.video) ||
    audioSources(before.audio) !== audioSources(after.audio) ||
    before.replay.storage !== after.replay.storage
  );
}

export function recordVideo(config: EngineConfig): EngineConfig["video"] {
  const record = config.record.video;
  return record ? { ...config.video, ...record } : config.video;
}

export function recordingConfig(config: EngineConfig): EngineConfig {
  return { ...config, video: recordVideo(config) };
}

export function recordVideoOf(video: EngineConfig["video"]): RecordVideo {
  const { fps, resolution, codec, quality, bitrate_kbps } = video;
  return { fps, resolution, codec, quality, bitrate_kbps };
}
