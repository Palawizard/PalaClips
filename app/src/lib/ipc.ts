import { Channel, invoke } from "@tauri-apps/api/core";
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

export interface HardwareEncoder {
  name: string;
  description: string;
  codec: VideoCodec;
  failure: string | null;
}

export function listEncoders(): Promise<HardwareEncoder[]> {
  return invoke<HardwareEncoder[]>("list_encoders");
}

export interface MemoryStatus {
  total_bytes: number;
  available_bytes: number;
}

export function memoryStatus(): Promise<MemoryStatus> {
  return invoke<MemoryStatus>("memory_status");
}

export interface ReplayEstimate {
  source_width: number;
  source_height: number;
  width: number;
  height: number;
  video_kbps: number;
  audio_tracks: number;
  bytes: number;
}

export function estimateReplay(config: EngineConfig): Promise<ReplayEstimate> {
  return invoke<ReplayEstimate>("estimate_replay", { config });
}

export function resetSettings(): Promise<void> {
  return invoke("reset_settings");
}

export type Theme = "system" | "dark" | "light";

export function getTheme(): Promise<Theme> {
  return invoke<Theme>("get_theme");
}

export function setTheme(theme: Theme): Promise<void> {
  return invoke("set_theme", { theme });
}

export type HotkeyAction = "save_replay" | "toggle_replay";

export interface Hotkeys {
  save_replay: string | null;
  toggle_replay: string | null;
}

export interface HotkeysView {
  hotkeys: Hotkeys;
  unavailable: HotkeyAction[];
}

export function getHotkeys(): Promise<HotkeysView> {
  return invoke<HotkeysView>("get_hotkeys");
}

export function setHotkeys(hotkeys: Hotkeys): Promise<void> {
  return invoke("set_hotkeys", { hotkeys });
}

export function suspendHotkeys(): Promise<void> {
  return invoke("suspend_hotkeys");
}

export function resumeHotkeys(): Promise<void> {
  return invoke("resume_hotkeys");
}

export function getAutostart(): Promise<boolean> {
  return invoke<boolean>("get_autostart");
}

export function setAutostart(enabled: boolean): Promise<void> {
  return invoke("set_autostart", { enabled });
}

export interface Feedback {
  sound: boolean;
  toast: boolean;
}

export function getFeedback(): Promise<Feedback> {
  return invoke<Feedback>("get_feedback");
}

export function setFeedback(feedback: Feedback): Promise<void> {
  return invoke("set_feedback", { feedback });
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

export function pickClipsDir(): Promise<string | null> {
  return invoke<string | null>("pick_clips_dir");
}

export function openClipsDir(): Promise<void> {
  return invoke("open_clips_dir");
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

export type TargetInfo =
  | {
      kind: "monitor";
      device_name: string;
      name: string;
      width: number;
      height: number;
      refresh_rate: number;
      primary: boolean;
    }
  | {
      kind: "window";
      exe_name: string;
      title: string;
      process_id: number;
      width: number;
      height: number;
      minimized: boolean;
    };

export function listCaptureTargets(): Promise<TargetInfo[]> {
  return invoke<TargetInfo[]>("list_capture_targets");
}

export function appIcon(processId: number): Promise<ArrayBuffer> {
  return invoke<ArrayBuffer>("app_icon", { processId });
}

export async function startPreview(
  target: CaptureTarget,
  onFrame: (jpeg: ArrayBuffer) => void,
): Promise<number> {
  const channel = new Channel<ArrayBuffer>();
  channel.onmessage = onFrame;
  return invoke<number>("start_preview", { target, channel });
}

export function stopPreview(id: number): Promise<void> {
  return invoke("stop_preview", { id });
}

export function stopPreviews(): Promise<void> {
  return invoke("stop_previews");
}

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
