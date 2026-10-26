import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";
import type { LibraryChange } from "./ipc.ts";
import { refreshOnLibraryChange } from "./library.ts";

function source() {
  const handlers: ((change: LibraryChange) => void)[] = [];
  const stop = vi.fn();
  return {
    stop,
    emit: (removed: string[] = []) => {
      for (const handler of handlers) handler({ removed });
    },
    subscribe: (handler: (change: LibraryChange) => void) => {
      handlers.push(handler);
      return Promise.resolve(stop);
    },
  };
}

describe("library changes", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  test("a_deleted_clip_refreshes_the_list_without_reopening_the_page", () => {
    const library = source();
    const refresh = vi.fn();
    refreshOnLibraryChange(refresh, library.subscribe, 200);
    library.emit(["C:\Clips\a.mp4"]);
    vi.advanceTimersByTime(200);
    expect(refresh).toHaveBeenCalledWith({ removed: ["C:\Clips\a.mp4"] });
  });

  test("changes_close_together_refresh_once_with_every_removed_clip", () => {
    const library = source();
    const refresh = vi.fn();
    refreshOnLibraryChange(refresh, library.subscribe, 200);
    library.emit(["a.mp4"]);
    vi.advanceTimersByTime(150);
    library.emit();
    library.emit(["b.mp4"]);
    vi.advanceTimersByTime(199);
    expect(refresh).not.toHaveBeenCalled();
    vi.advanceTimersByTime(1);
    expect(refresh).toHaveBeenCalledTimes(1);
    expect(refresh).toHaveBeenCalledWith({ removed: ["a.mp4", "b.mp4"] });
    library.emit();
    vi.advanceTimersByTime(200);
    expect(refresh).toHaveBeenLastCalledWith({ removed: [] });
  });

  test("leaving_the_page_drops_the_pending_refresh_and_stops_listening", async () => {
    const library = source();
    const refresh = vi.fn();
    const stop = refreshOnLibraryChange(refresh, library.subscribe, 200);
    library.emit(["a.mp4"]);
    stop();
    vi.advanceTimersByTime(500);
    expect(refresh).not.toHaveBeenCalled();
    await Promise.resolve();
    expect(library.stop).toHaveBeenCalled();
  });
});
