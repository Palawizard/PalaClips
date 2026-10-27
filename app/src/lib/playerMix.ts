import type { ClipMix, TrackKind } from "./ipc.ts";
import type { TrackMix } from "./playerAudio.ts";

export const FULL_MIX: TrackMix = { volume: 1, mute: false, solo: false };

export function openingMix(count: number, stored: ClipMix | null): ClipMix {
  return stored && stored.tracks.length === count
    ? stored
    : { tracks: Array.from({ length: count }, () => FULL_MIX), full_mix_chosen: false };
}

export function mutesFullMix(kinds: TrackKind[], state: ClipMix, setting: boolean): boolean {
  return (
    setting &&
    !state.full_mix_chosen &&
    kinds.includes("mix") &&
    kinds.some((kind) => kind === "application" || kind === "game")
  );
}

export function heardMix(kinds: TrackKind[], state: ClipMix, setting: boolean): TrackMix[] {
  const muted = mutesFullMix(kinds, state, setting);
  return state.tracks.map((track, index) => (muted && kinds[index] === "mix" ? { ...track, mute: true } : track));
}

export function changeTrack(kinds: TrackKind[], state: ClipMix, setting: boolean, index: number, next: TrackMix): ClipMix {
  const heard = heardMix(kinds, state, setting)[index];
  const current = state.tracks[index];
  if (!heard || !current) return state;
  const choosesFullMix = kinds[index] === "mix" && next.mute !== heard.mute;
  const kept = choosesFullMix || kinds[index] !== "mix" ? next : { ...next, mute: current.mute };
  return {
    tracks: state.tracks.map((track, place) => (place === index ? kept : track)),
    full_mix_chosen: state.full_mix_chosen || choosesFullMix,
  };
}

export function audibleTracks(mix: TrackMix[]): boolean[] {
  const solo = mix.some((track) => track.solo);
  return mix.map((track) => !track.mute && track.volume > 0 && (!solo || track.solo));
}
