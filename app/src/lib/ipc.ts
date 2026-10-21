import { Channel, invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { Language, LanguageSetting, Localized } from "./i18n.ts";

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

export interface LanguageView {
  setting: LanguageSetting;
  resolved: Language;
}

export function getLanguage(): Promise<LanguageView> {
  return invoke<LanguageView>("get_language");
}

export function setLanguageSetting(language: LanguageSetting): Promise<Language> {
  return invoke<Language>("set_language", { language });
}

export type HotkeyAction = "save_replay" | "toggle_replay" | "toggle_recording";

export interface Hotkeys {
  save_replay: string | null;
  toggle_replay: string | null;
  toggle_recording: string | null;
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

export function recordStart(): Promise<void> {
  return invoke("record_start");
}

export function recordStop(): Promise<void> {
  return invoke("record_stop");
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

export interface UpdateInfo {
  current: string;
  available: { version: string; notes: string | null } | null;
}

export function getUpdate(): Promise<UpdateInfo> {
  return invoke("get_update");
}

export function checkUpdate(): Promise<UpdateInfo> {
  return invoke("check_update");
}

export function installUpdate(): Promise<void> {
  return invoke("install_update");
}

export interface OnboardingChecks {
  windows_build: number | null;
  windows_supported: boolean;
  webview: string | null;
  clips_dir_writable: boolean;
}

export function onboardingNeeded(): Promise<boolean> {
  return invoke("onboarding_needed");
}

export function onboardingChecks(): Promise<OnboardingChecks> {
  return invoke("onboarding_checks");
}

export function finishOnboarding(): Promise<void> {
  return invoke("finish_onboarding");
}

export function exportDiagnostic(): Promise<string | null> {
  return invoke("export_diagnostic");
}

export function openDiagnostic(): Promise<void> {
  return invoke("open_diagnostic");
}

export function openClipsDir(): Promise<void> {
  return invoke("open_clips_dir");
}

export function onEngineEvent(handler: (event: EngineEvent) => void): Promise<UnlistenFn> {
  return listen<EngineEvent>(ENGINE_EVENT, (message) => handler(message.payload));
}

export const LIBRARY_EVENT = "library-changed";

export function onLibraryChanged(handler: () => void): Promise<UnlistenFn> {
  return listen(LIBRARY_EVENT, handler);
}

export type ClipKind = "replay" | "recording";
export type ClipSort = "newest" | "oldest" | "largest" | "longest" | "name";

export interface Clip {
  path: string;
  name: string;
  game: string;
  kind: ClipKind;
  modified_ms: number;
  size_bytes: number;
  duration_secs: number;
  fps: number;
  tracks: string[];
  thumbnail: string | null;
  favorite: boolean;
}

export interface ClipQuery {
  game?: string | null;
  kind?: ClipKind | null;
  favorites?: boolean;
  since_ms?: number | null;
  search?: string | null;
  sort?: ClipSort;
  limit?: number | null;
}

export function listClips(query: ClipQuery): Promise<Clip[]> {
  return invoke<Clip[]>("list_clips", { query });
}

export function clipGames(): Promise<string[]> {
  return invoke<string[]>("clip_games");
}

export function setClipFavorite(path: string, favorite: boolean): Promise<void> {
  return invoke("set_clip_favorite", { path, favorite });
}

export function renameClip(path: string, name: string): Promise<string> {
  return invoke<string>("rename_clip", { path, name });
}

export function trashClip(path: string): Promise<void> {
  return invoke("trash_clip", { path });
}

export function revealClip(path: string): Promise<void> {
  return invoke("reveal_clip", { path });
}

export interface QuotaPreview {
  total_bytes: number;
  deleted: Clip[];
}

export function quotaPreview(quotaGb: number): Promise<QuotaPreview> {
  return invoke<QuotaPreview>("quota_preview", { quotaGb });
}

export const MEDIA_SCHEME = "palaclips";

export interface TrackFile {
  title: string;
  path: string;
}

export function preparePlayer(path: string): Promise<TrackFile[]> {
  return invoke<TrackFile[]>("prepare_player", { path });
}

export function trackPeaks(path: string): Promise<ArrayBuffer> {
  return invoke<ArrayBuffer>("track_peaks", { path });
}

export function copyClip(path: string): Promise<void> {
  return invoke("copy_clip", { path });
}

export interface ExportInput {
  path: string;
  name: string;
  start_secs: number;
  end_secs: number;
  precise: boolean;
  tracks: number[];
  gains: number[];
  mix: boolean;
  mix_title: string;
}

export interface ExportFinished {
  id: number;
  path: string | null;
  error: Localized | null;
}

export function startExport(exportInput: ExportInput): Promise<number> {
  return invoke<number>("start_export", { export: exportInput });
}

export function cancelExport(id: number): Promise<void> {
  return invoke("cancel_export", { id });
}

export function onExportProgress(handler: (id: number, progress: number) => void): Promise<UnlistenFn> {
  return listen<{ id: number; progress: number }>("export-progress", (message) =>
    handler(message.payload.id, message.payload.progress),
  );
}

export function onExportFinished(handler: (finished: ExportFinished) => void): Promise<UnlistenFn> {
  return listen<ExportFinished>("export-finished", (message) => handler(message.payload));
}

export type VideoCodec = "h264" | "hevc" | "av1";
export type VideoQuality = "quality" | "balanced" | "light" | "custom";
export type ReplayStorage = "ram" | "disk";

export type RecordSplit =
  | { kind: "never" }
  | { kind: "duration"; minutes: number }
  | { kind: "size"; megabytes: number };

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
      app_name: string;
      title: string;
      process_id: number;
      width: number;
      height: number;
      minimized: boolean;
    };

export interface AudioDevice {
  id: string;
  name: string;
  default: boolean;
}

export function listAudioDevices(input: boolean): Promise<AudioDevice[]> {
  return invoke<AudioDevice[]>("list_audio_devices", { input });
}

export interface Measurement {
  pairs: number;
  late_ms: number;
  spread_ms: number;
}

export interface Calibration {
  tracks: { kind: TrackKind; title: string; measurement: Measurement | null }[];
  mix_ms: number | null;
  microphone_ms: number | null;
  game_ms: number | null;
  apps: { exe_name: string; offset_ms: number }[];
}

export function calibrateAudio(): Promise<Calibration> {
  return invoke<Calibration>("calibrate_audio");
}

export interface AppSession {
  exe_name: string;
  app_name: string;
  process_id: number;
  peak: number;
}

export function listAppSessions(): Promise<AppSession[]> {
  return invoke<AppSession[]>("list_app_sessions");
}

export interface PickedApp {
  exe_name: string;
  app_name: string;
}

export function pickAppExe(): Promise<PickedApp | null> {
  return invoke<PickedApp | null>("pick_app_exe");
}

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

export function setPerformancePanel(open: boolean): Promise<void> {
  return invoke("set_performance_panel", { open });
}

export type GpuPriority = "game" | "capture";

export type OutputResolution ={ kind: "native" } | { kind: "height"; height: number };

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
    gpu_priority: GpuPriority;
  };
  audio: {
    record_full_mix: boolean;
    full_mix_label: string | null;
    output_device_id: string | null;
    full_mix_sync_offset_ms: number;
    desktop_in_full_mix: boolean;
    record_microphone: boolean;
    microphone_label: string | null;
    microphone_device_id: string | null;
    microphone_gain_db: number;
    microphone_sync_offset_ms: number;
    microphone_in_full_mix: boolean;
    app_tracks: AppTrack[];
    max_app_tracks: number;
    auto_game_track: boolean;
    game_label: string | null;
    game_sync_offset_ms: number;
    game_exe: string | null;
  };
  replay: {
    arm_on_startup: boolean;
    duration_secs: number;
    storage: ReplayStorage;
  };
  record: {
    include_replay: boolean;
    split: RecordSplit;
  };
  storage: {
    clips_dir: string | null;
    game_names: Record<string, string>;
    quota_gb: number | null;
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

export interface ActiveAlert extends Localized {
  severity: Severity;
  message: string;
}

export type TrackState = "waiting" | "capturing" | "silent" | "unavailable";

export interface CaptureInfo {
  source: Localized;
  width: number;
  height: number;
  fps: number;
}

export type TrackKind = "mix" | "microphone" | "application" | "game";

export interface TrackDescriptor {
  title: string;
  kind: TrackKind;
}

export interface TrackInfo {
  title: string;
  kind: TrackKind;
  state: TrackState | null;
  source: string | null;
}

export interface AppStatus {
  state: EngineState;
  buffered_secs: number;
  buffer_bytes: number;
  recording_secs: number;
  recording_bytes: number;
  capture: CaptureInfo | null;
  tracks: TrackInfo[];
  alerts: ActiveAlert[];
}

export interface CapturePerformance {
  encoder: string;
  width: number;
  height: number;
  fps: number;
  repeated_percent: number;
  late_total: number;
  late_last_minute: number;
  lag_max_ms: number;
}

export interface PerformanceStats {
  cpu_percent: number | null;
  cpu_max_percent: number | null;
  engine_bytes: number | null;
  buffer_bytes: number;
  buffered_secs: number;
  capture: CapturePerformance | null;
}

export type EngineEvent =
  | { type: "state_changed"; state: EngineState }
  | {
      type: "capture_started";
      source: Localized;
      width: number;
      height: number;
      fps: number;
      tracks: TrackDescriptor[];
    }
  | { type: "tracks_renamed"; tracks: TrackDescriptor[] }
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
  | ({ type: "performance_stats" } & PerformanceStats)
  | { type: "recording_stats"; elapsed_secs: number; bytes: number }
  | ({ type: "alert"; severity: Severity; message: string } & Localized)
  | { type: "alert_cleared"; code: string };
