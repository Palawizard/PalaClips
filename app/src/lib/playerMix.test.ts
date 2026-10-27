import { describe, expect, test } from "vitest";
import type { TrackKind } from "./ipc.ts";
import { FULL_MIX, audibleTracks, changeTrack, heardMix, openingMix } from "./playerMix.ts";

const separate: TrackKind[] = ["mix", "microphone", "game", "application"];
const mutes = (kinds: TrackKind[], setting = true) => heardMix(kinds, openingMix(kinds.length, null), setting).map((track) => track.mute);

describe("player mix", () => {
  test("the_full_mix_starts_muted_when_the_clip_has_separate_tracks", () => {
    expect(mutes(separate)).toEqual([true, false, false, false]);
  });

  test("a_clip_with_only_the_mix_or_the_mix_and_mic_is_not_muted", () => {
    expect(mutes(["mix"])).toEqual([false]);
    expect(mutes(["mix", "microphone"])).toEqual([false, false]);
  });

  test("the_setting_off_plays_every_track", () => {
    expect(mutes(separate, false)).toEqual([false, false, false, false]);
  });

  test("turning_the_full_mix_back_on_sticks_for_the_clip", () => {
    const opened = openingMix(separate.length, null);
    const heard = heardMix(separate, opened, true)[0] ?? FULL_MIX;
    const chosen = changeTrack(separate, opened, true, 0, { ...heard, mute: false });
    expect(chosen.full_mix_chosen).toBe(true);
    expect(heardMix(separate, openingMix(separate.length, chosen), true)[0]?.mute).toBe(false);
  });

  test("other_changes_keep_the_full_mix_muted_by_the_setting_only", () => {
    const opened = openingMix(separate.length, null);
    const louder = changeTrack(separate, opened, true, 0, { volume: 0.5, mute: true, solo: false });
    expect(louder.full_mix_chosen).toBe(false);
    expect(louder.tracks[0]).toEqual({ volume: 0.5, mute: false, solo: false });
    const soloed = changeTrack(separate, louder, true, 2, { ...FULL_MIX, solo: true });
    expect(heardMix(separate, soloed, true).map((track) => track.mute)).toEqual([true, false, false, false]);
  });

  test("a_clip_reopens_with_its_saved_state_unless_its_tracks_changed", () => {
    const stored = { tracks: [FULL_MIX, { ...FULL_MIX, volume: 0.3 }, FULL_MIX, FULL_MIX], full_mix_chosen: true };
    expect(openingMix(separate.length, stored)).toBe(stored);
    expect(openingMix(separate.length, { ...stored, tracks: stored.tracks.slice(1) }).full_mix_chosen).toBe(false);
  });

  test("the_export_keeps_track_one_when_only_the_setting_muted_it", () => {
    const opened = openingMix(separate.length, null);
    expect(audibleTracks(opened.tracks)).toEqual([true, true, true, true]);
    const muted = changeTrack(separate, { ...opened, full_mix_chosen: false }, false, 0, { ...FULL_MIX, mute: true });
    expect(audibleTracks(muted.tracks)).toEqual([false, true, true, true]);
  });
});
