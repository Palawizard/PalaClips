import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export const ENGINE_EVENT = "engine-event";

export function engineVersion(): Promise<string> {
  return invoke<string>("engine_version");
}

export function defaultConfig(): Promise<EngineConfig> {
  return invoke<EngineConfig>("default_config");
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
  };
}

export type EngineState = "idle" | "replay_armed" | "recording";
export type Severity = "info" | "warning" | "error";

export interface ActiveAlert {
  severity: Severity;
  code: string;
  message: string;
}

export interface AppStatus {
  state: EngineState;
  buffered_secs: number;
  buffer_bytes: number;
  alerts: ActiveAlert[];
}

export type EngineEvent =
  | { type: "state_changed"; state: EngineState }
  | { type: "clip_saved"; path: string; duration_secs: number; save_latency_ms: number }
  | { type: "audio_levels"; levels: [string, number][] }
  | { type: "replay_stats"; buffered_secs: number; bytes: number }
  | { type: "alert"; severity: Severity; code: string; message: string };
