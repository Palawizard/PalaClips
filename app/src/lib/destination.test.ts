import { describe, expect, test } from "vitest";
import { clipRequest, playerExit } from "./destination.ts";

describe("clip destination", () => {
  test("opening_a_clip_from_status_returns_to_status", () => {
    const request = clipRequest("C:\clips\Fortnite\a.mp4", "status", null);
    expect(request).toEqual({ path: "C:\clips\Fortnite\a.mp4", returnTo: "status", key: 1 });
    expect(playerExit(request)).toBe("status");
  });

  test("a_destination_without_a_clip_opens_no_player", () => {
    expect(clipRequest(undefined, "status", null)).toBeNull();
    expect(playerExit(null)).toBe("gallery");
  });

  test("opening_the_same_clip_again_is_a_new_request", () => {
    const first = clipRequest("a.mp4", "status", null);
    const second = clipRequest("a.mp4", "status", first);
    expect(second?.key).toBe(2);
  });
});
