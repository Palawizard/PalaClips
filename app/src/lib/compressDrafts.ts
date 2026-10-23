import { useSyncExternalStore } from "react";
import { errorText } from "./i18n.ts";
import {
  type CompressCodec,
  type CompressPlan,
  type CompressPlanView,
  type CompressSource,
  compressPlan,
  compressPreview,
  compressProbe,
} from "./ipc.ts";

export type Preset = "discord" | "nitroBasic" | "nitro" | "other";

export const PRESET_MB: Record<Exclude<Preset, "other">, number> = { discord: 20, nitroBasic: 50, nitro: 500 };
const BYTES_PER_MB = 1_000_000;

export interface CompressOptions {
  preset: Preset;
  freeMb: number;
  codec: CompressCodec;
  allTracks: boolean;
}

export interface Draft {
  key: number;
  path: string;
  name: string;
  startSecs: number;
  endSecs: number | null;
  thumbnail: string | null;
  source: CompressSource | null;
  view: CompressPlanView | null;
  error: string | null;
  revision: number;
}

export interface DraftRequest {
  path: string;
  startSecs?: number;
  endSecs?: number | null;
}

interface State {
  options: CompressOptions;
  drafts: Draft[];
}

let state: State = {
  options: { preset: "discord", freeMb: 100, codec: "h264", allTracks: false },
  drafts: [],
};
let nextKey = 1;
const thumbnails = new Map<string, string>();

export function thumbnailOf(path: string): string | null {
  return thumbnails.get(path) ?? null;
}
const listeners = new Set<() => void>();

function publish(next: State) {
  state = next;
  for (const listener of listeners) listener();
}

function revise(key: number): number {
  const draft = state.drafts.find((item) => item.key === key);
  const revision = (draft?.revision ?? 0) + 1;
  patchDraft(key, { revision });
  return revision;
}

function patchCurrent(key: number, revision: number, patch: Partial<Draft>) {
  if (state.drafts.find((item) => item.key === key)?.revision === revision) patchDraft(key, patch);
}

function patchDraft(key: number, patch: Partial<Draft>) {
  publish({ ...state, drafts: state.drafts.map((draft) => (draft.key === key ? { ...draft, ...patch } : draft)) });
}

export function targetBytes(options: CompressOptions): number {
  const megabytes = options.preset === "other" ? options.freeMb : PRESET_MB[options.preset];
  return Math.round(megabytes * BYTES_PER_MB);
}

export function planInput(draft: Draft, options: CompressOptions) {
  return {
    path: draft.path,
    target_bytes: targetBytes(options),
    codec: options.codec,
    all_tracks: options.allTracks,
    start_secs: draft.startSecs,
    end_secs: draft.endSecs,
  };
}

function replan(draft: Draft) {
  if (!draft.source) return;
  const revision = revise(draft.key);
  compressPlan(planInput(draft, state.options)).then(
    (view) => patchCurrent(draft.key, revision, { view, error: null }),
    (failure) => patchCurrent(draft.key, revision, { view: null, error: errorText(failure) }),
  );
}

function fileName(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}

export function addDrafts(requests: DraftRequest[]) {
  const added = requests.map((request) => ({
    key: nextKey++,
    path: request.path,
    name: fileName(request.path),
    startSecs: request.startSecs ?? 0,
    endSecs: request.endSecs ?? null,
    thumbnail: null,
    source: null,
    view: null,
    error: null,
    revision: 0,
  }));
  publish({ ...state, drafts: [...state.drafts, ...added] });
  for (const draft of added) {
    compressProbe(draft.path).then(
      ({ source, thumbnail }) => {
        if (thumbnail) thumbnails.set(draft.path, thumbnail);
        patchDraft(draft.key, { source, thumbnail });
        replan({ ...draft, source });
      },
      (failure) => patchDraft(draft.key, { error: errorText(failure) }),
    );
  }
}

export function removeDraft(key: number) {
  publish({ ...state, drafts: state.drafts.filter((draft) => draft.key !== key) });
}

export function clearDrafts(keys: number[]) {
  publish({ ...state, drafts: state.drafts.filter((draft) => !keys.includes(draft.key)) });
}

export function setOptions(patch: Partial<CompressOptions>) {
  publish({ ...state, options: { ...state.options, ...patch } });
  for (const draft of state.drafts) replan(draft);
}

export function editPlan(key: number, plan: CompressPlan) {
  const draft = state.drafts.find((item) => item.key === key);
  if (!draft?.view) return;
  const duration = draft.view.duration_secs;
  patchDraft(key, { view: { ...draft.view, plan } });
  const revision = revise(key);
  compressPreview(plan, state.options.codec, duration).then(
    (view) => patchCurrent(key, revision, { view, error: null }),
    (failure) => patchCurrent(key, revision, { error: errorText(failure) }),
  );
}

function subscribe(listener: () => void) {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

export function useCompressDrafts(): State {
  return useSyncExternalStore(subscribe, () => state);
}
