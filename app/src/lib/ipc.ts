import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export const ENGINE_EVENT = "engine-event";

export function engineVersion(): Promise<string> {
  return invoke<string>("engine_version");
}

export function defaultConfig(): Promise<EngineConfig> {
  return invoke<EngineConfig>("default_config");
}

export function getConfig(): Promise<EngineConfig> {
  return invoke<EngineConfig>("get_config");
}

export function setConfig(config: EngineConfig): Promise<void> {
  return invoke("set_config", { config });
}

export function resetConfig(): Promise<EngineConfig> {
  return invoke<EngineConfig>("reset_config");
}

export function getState(): Promise<AppStatus> {
  return invoke<AppStatus>("get_state");
}

export function replayArm(): Promise<void> {
  return invoke("replay_arm");
}

export function replayDisarm(): Promise<void> {
  return invoke("replay_disarm");
}

export function replaySave(): Promise<void> {
  return invoke("replay_save");
}

export function listRecentClips(limit: number): Promise<ClipSummary[]> {
  return invoke<ClipSummary[]>("list_recent_clips", { limit });
}

export function storageStatus(): Promise<StorageStatus> {
  return invoke<StorageStatus>("storage_status");
}

export function onEngineEvent(handler: (event: EngineEvent) => void): Promise<UnlistenFn> {
  return listen<EngineEvent>(ENGINE_EVENT, (message) => handler(message.payload));
}

export type VideoCodec = "h264" | "hevc" | "av1";
export type VideoQuality = "quality" | "balanced" | "light" | "custom";
export type ReplayStorage = "ram" | "disk";

export type CaptureTarget =
  | { kind: "primary_monitor" }
  | { kind: "monitor"; device_name: string }
  | { kind: "window"; exe_name: string; title_hint: string | null };

export type OutputResolution = { kind: "native" } | { kind: "height"; height: number };

export interface AppTrack {
  exe_name: string;
  label: string;
  enabled: boolean;
  sync_offset_ms: number;
}

export interface EngineConfig {
  schema_version: number;
  capture: CaptureTarget;
  video: {
    fps: number;
    resolution: OutputResolution;
    codec: VideoCodec;
    quality: VideoQuality;
    bitrate_kbps: number;
    keyframe_interval_secs: number;
    capture_cursor: boolean;
    draw_border: boolean;
  };
  audio: {
    record_full_mix: boolean;
    output_device_id: string | null;
    full_mix_sync_offset_ms: number;
    record_microphone: boolean;
    microphone_device_id: string | null;
    microphone_gain_db: number;
    microphone_sync_offset_ms: number;
    app_tracks: AppTrack[];
    max_app_tracks: number;
    auto_game_track: boolean;
    game_exe: string | null;
  };
  replay: {
    arm_on_startup: boolean;
    duration_secs: number;
    storage: ReplayStorage;
  };
  storage: {
    clips_dir: string | null;
    game_names: Record<string, string>;
  };
}

export type EngineState = "idle" | "replay_armed" | "recording";
export type Severity = "info" | "warning" | "error";

export interface ClipSummary {
  path: string;
  game: string;
  modified_ms: number;
  size_bytes: number;
  duration_secs: number;
  tracks: string[];
  thumbnail: string | null;
}

export interface StorageStatus {
  clips_dir: string;
  free_bytes: number;
  total_bytes: number;
}

export interface ActiveAlert {
  severity: Severity;
  code: string;
  message: string;
}

export type TrackState = "waiting" | "capturing" | "silent" | "unavailable";

export interface CaptureInfo {
  source: string;
  width: number;
  height: number;
  fps: number;
}

export interface TrackInfo {
  title: string;
  state: TrackState | null;
  source: string | null;
}

export interface AppStatus {
  state: EngineState;
  buffered_secs: number;
  buffer_bytes: number;
  capture: CaptureInfo | null;
  tracks: TrackInfo[];
  alerts: ActiveAlert[];
}

export type EngineEvent =
  | { type: "state_changed"; state: EngineState }
  | {
      type: "capture_started";
      source: string;
      width: number;
      height: number;
      fps: number;
      tracks: string[];
    }
  | { type: "track_changed"; track: number; title: string; state: TrackState; source: string | null }
  | { type: "game_named"; exe_name: string; name: string }
  | {
      type: "clip_saved";
      path: string;
      duration_secs: number;
      save_latency_ms: number;
      thumbnail: string | null;
    }
  | { type: "audio_levels"; levels: number[] }
  | { type: "replay_stats"; buffered_secs: number; bytes: number }
  | { type: "alert"; severity: Severity; code: string; message: string };
