import { invoke } from "@tauri-apps/api/core";

export function engineVersion(): Promise<string> {
  return invoke<string>("engine_version");
}

export function defaultConfig(): Promise<EngineConfig> {
  return invoke<EngineConfig>("default_config");
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
    record_microphone: boolean;
    microphone_device_id: string | null;
    microphone_gain_db: number;
    app_tracks: AppTrack[];
    max_app_tracks: number;
    auto_game_track: boolean;
  };
  replay: {
    arm_on_startup: boolean;
    duration_secs: number;
    storage: ReplayStorage;
  };
}

export type EngineState = "idle" | "replay_armed" | "recording";
export type Severity = "info" | "warning" | "error";

export type EngineEvent =
  | { type: "state_changed"; state: EngineState }
  | { type: "clip_saved"; path: string; duration_secs: number; save_latency_ms: number }
  | { type: "audio_levels"; levels: [string, number][] }
  | { type: "replay_stats"; buffered_secs: number; bytes: number }
  | { type: "alert"; severity: Severity; code: string; message: string };
