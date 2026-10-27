import { describe, expect, test } from "vitest";
import type { EngineConfig } from "./ipc.ts";
import { changedTabs, nextTab, rememberTab, rememberedTab } from "./qualityTabs.ts";

const saved = {
  replay: { duration_secs: 60, after_secs: 0, storage: "ram", arm_on_startup: true },
  video: {
    codec: "h264",
    quality: "balanced",
    bitrate_kbps: 40000,
    fps: 60,
    resolution: { kind: "native" },
    encoder: "auto",
    keyframe_interval_secs: 1,
    gpu_priority: "game",
  },
  record: { video: null },
} as unknown as EngineConfig;

describe("quality tabs", () => {
  test("only_the_tab_holding_a_change_is_marked", () => {
    expect(changedTabs(saved, saved)).toEqual([]);
    expect(changedTabs(saved, { ...saved, replay: { ...saved.replay, after_secs: 10 } })).toEqual(["replay"]);
    expect(changedTabs(saved, { ...saved, record: { ...saved.record, video: { fps: 30 } } } as EngineConfig)).toEqual([
      "recording",
    ]);
    expect(changedTabs(saved, { ...saved, video: { ...saved.video, gpu_priority: "capture" } })).toEqual(["common"]);
  });

  test("settings_outside_the_page_mark_no_tab", () => {
    expect(changedTabs(saved, { ...saved, replay: { ...saved.replay, arm_on_startup: false } })).toEqual([]);
  });

  test("arrows_wrap_and_home_end_jump", () => {
    expect(nextTab("replay", "ArrowRight")).toBe("recording");
    expect(nextTab("common", "ArrowRight")).toBe("replay");
    expect(nextTab("replay", "ArrowLeft")).toBe("common");
    expect(nextTab("recording", "Home")).toBe("replay");
    expect(nextTab("recording", "End")).toBe("common");
    expect(nextTab("recording", "Tab")).toBeNull();
  });

  test("the_chosen_tab_is_kept_for_the_session", () => {
    expect(rememberedTab()).toBe("replay");
    rememberTab("recording");
    expect(rememberedTab()).toBe("recording");
  });
});
