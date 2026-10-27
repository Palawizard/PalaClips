import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";
import { SETTLE_MS, type ScrubMedia, ScrubSession } from "./scrubSession.ts";

function fakeMedia(playing = false) {
  const log = { painted: [] as number[], seeks: [] as number[], held: 0, released: [] as boolean[] };
  const media: ScrubMedia = {
    playing: () => playing,
    hold: () => void log.held++,
    paint: (seconds) => void log.painted.push(seconds),
    seek: (seconds) => {
      log.seeks.push(seconds);
      return true;
    },
    release: (resume) => void log.released.push(resume),
  };
  return { media, log };
}

describe("scrub session", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  test("the_cursor_follows_every_move_without_loading_the_video", () => {
    const { media, log } = fakeMedia();
    const scrub = new ScrubSession(media);
    scrub.start(1);
    for (const seconds of [2, 3, 4, 5]) {
      vi.advanceTimersByTime(SETTLE_MS / 4);
      scrub.move(seconds);
    }
    expect(log.painted).toEqual([1, 2, 3, 4, 5]);
    expect(log.seeks).toEqual([]);
  });

  test("a_still_pointer_loads_the_frame_after_the_settle_delay", () => {
    const { media, log } = fakeMedia();
    const scrub = new ScrubSession(media);
    scrub.start(1);
    scrub.move(7);
    vi.advanceTimersByTime(SETTLE_MS - 1);
    expect(log.seeks).toEqual([]);
    vi.advanceTimersByTime(1);
    expect(log.seeks).toEqual([7]);
  });

  test("only_one_load_runs_and_the_latest_position_wins", () => {
    const { media, log } = fakeMedia();
    const scrub = new ScrubSession(media);
    scrub.start(1);
    vi.advanceTimersByTime(SETTLE_MS);
    for (const seconds of [2, 3, 4]) {
      scrub.move(seconds);
      vi.advanceTimersByTime(SETTLE_MS);
    }
    expect(log.seeks).toEqual([1]);
    scrub.loaded();
    expect(log.seeks).toEqual([1, 4]);
    scrub.loaded();
    expect(log.seeks).toEqual([1, 4]);
  });

  test("release_loads_the_exact_position_then_hands_back_the_video", () => {
    const { media, log } = fakeMedia(true);
    const scrub = new ScrubSession(media);
    scrub.start(1);
    scrub.move(9.5);
    scrub.end();
    expect(log.seeks).toEqual([9.5]);
    expect(log.released).toEqual([]);
    expect(scrub.scrubbing).toBe(true);
    vi.advanceTimersByTime(SETTLE_MS * 2);
    expect(log.seeks).toEqual([9.5]);
    scrub.loaded();
    expect(log.released).toEqual([true]);
    expect(scrub.scrubbing).toBe(false);
    expect(log.held).toBe(1);
  });

  test("release_waits_for_the_load_in_flight_before_the_final_one", () => {
    const { media, log } = fakeMedia();
    const scrub = new ScrubSession(media);
    scrub.start(1);
    vi.advanceTimersByTime(SETTLE_MS);
    scrub.move(6);
    scrub.end();
    expect(log.seeks).toEqual([1]);
    scrub.loaded();
    expect(log.seeks).toEqual([1, 6]);
    expect(log.released).toEqual([]);
    scrub.loaded();
    expect(log.released).toEqual([false]);
  });

  test("grabbing_again_before_the_final_load_keeps_one_hold_and_the_first_play_state", () => {
    const { media, log } = fakeMedia(true);
    const scrub = new ScrubSession(media);
    scrub.start(1);
    scrub.end();
    scrub.start(3);
    scrub.loaded();
    expect(log.released).toEqual([]);
    scrub.end();
    scrub.loaded();
    expect(log.held).toBe(1);
    expect(log.released).toEqual([true]);
  });

  test("a_video_that_cannot_seek_yet_is_released_at_once", () => {
    const { media, log } = fakeMedia();
    media.seek = (seconds) => {
      log.seeks.push(seconds);
      return false;
    };
    const scrub = new ScrubSession(media);
    scrub.start(2);
    scrub.end();
    expect(log.released).toEqual([false]);
  });
});
