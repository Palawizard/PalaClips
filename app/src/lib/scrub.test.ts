import { describe, expect, test } from "vitest";
import { SNAP_PX, scrubTime } from "./scrub.ts";

describe("scrubbing", () => {
  const width = 1000;
  const span = 100;

  test("a_click_on_a_track_seeks_to_that_moment", () => {
    expect(scrubTime(250, width, span, [])).toBe(25);
  });

  test("the_playhead_snaps_to_the_cut_when_it_passes_close", () => {
    const edges = [40, 80];
    expect(scrubTime(400 + SNAP_PX, width, span, edges)).toBe(40);
    expect(scrubTime(800 - SNAP_PX, width, span, edges)).toBe(80);
    expect(scrubTime(400 + SNAP_PX + 1, width, span, edges)).toBeCloseTo(40.9);
  });

  test("dragging_past_the_ends_stays_inside_the_clip", () => {
    expect(scrubTime(-50, width, span, [])).toBe(0);
    expect(scrubTime(1200, width, span, [])).toBe(100);
    expect(scrubTime(10, 0, span, [])).toBe(0);
  });
});
