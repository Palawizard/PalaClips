import { describe, expect, test, vi } from "vitest";
import { type ConfigPatch, configPatch, createConfigStore, restartsReplay, withPatch } from "./config.ts";
import type { EngineConfig } from "./ipc.ts";
import cases from "./restart-cases.json";

const base = {
  capture: { kind: "window", exe_name: "game.exe", title_hint: null },
  video: { fps: 60, resolution: { kind: "native" }, bitrate_kbps: 40000 },
  replay: { duration_secs: 60, after_secs: 0 },
  audio: { app_tracks: [{ exe_name: "discord.exe", enabled: true }], microphone_gain_db: 0 },
};

type Config = typeof base;

describe("config patches", () => {
  test("patch_holds_only_the_changed_fields", () => {
    const next = { ...base, video: { ...base.video, fps: 120 } };
    expect(configPatch(base, next)).toEqual({ video: { fps: 120 } });
  });

  test("tagged_values_and_lists_are_replaced_whole", () => {
    const next = {
      ...base,
      capture: { kind: "primary_monitor" },
      video: { ...base.video, resolution: { kind: "height", height: 1080 } },
      audio: { ...base.audio, app_tracks: [] },
    } as unknown as Config;
    expect(configPatch(base, next)).toEqual({
      capture: { kind: "primary_monitor" },
      video: { resolution: { kind: "height", height: 1080 } },
      audio: { app_tracks: [] },
    });
    expect(withPatch(base, configPatch(base, next))).toEqual(next);
  });

  test("a_page_opened_before_another_change_keeps_that_change", () => {
    const capturePage = configPatch(base, { ...base, video: { ...base.video, fps: 120 } });
    const changedElsewhere = { ...base, replay: { ...base.replay, duration_secs: 120 } };
    const draft = withPatch(changedElsewhere, capturePage);
    expect(draft.replay.duration_secs).toBe(120);
    expect(draft.video.fps).toBe(120);
  });

  test("no_change_gives_an_empty_patch", () => {
    expect(configPatch(base, structuredClone(base))).toEqual({});
  });
});

describe("config store", () => {
  test("every_reader_sees_the_config_changed_elsewhere", async () => {
    let push: (config: Config) => void = () => {};
    const store = createConfigStore<Config>({
      load: () => Promise.resolve(base),
      subscribe: (handler: (config: Config) => void) => {
        push = handler;
      },
    });
    const listener = vi.fn();
    store.listen(listener);
    await store.ready();
    expect(store.current()?.replay.duration_secs).toBe(60);
    push({ ...base, replay: { ...base.replay, duration_secs: 120 } });
    expect(store.current()?.replay.duration_secs).toBe(120);
    expect(listener).toHaveBeenLastCalledWith(expect.objectContaining({ replay: { duration_secs: 120, after_secs: 0 } }));
  });

  test("an_event_arriving_before_the_first_load_wins", async () => {
    let push: (config: Config) => void = () => {};
    let resolve: (config: Config) => void = () => {};
    const store = createConfigStore<Config>({
      load: () => new Promise((done) => (resolve = done)),
      subscribe: (handler: (config: Config) => void) => {
        push = handler;
      },
    });
    store.listen(() => {});
    push({ ...base, replay: { ...base.replay, duration_secs: 120 } });
    resolve(base);
    await store.ready();
    expect(store.current()?.replay.duration_secs).toBe(120);
  });
});

describe("restart criteria", () => {
  test("front_and_engine_agree_on_what_restarts_the_replay", () => {
    const base = cases.base as unknown as EngineConfig;
    for (const item of cases.cases) {
      const before = "before" in item ? withPatch(base, item.before as ConfigPatch) : base;
      const after = withPatch(before, item.after as ConfigPatch);
      expect(restartsReplay(before, after), item.name).toBe(item.restarts);
    }
  });
});
