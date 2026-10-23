import { describe, expect, test, vi } from "vitest";
import { type PendingChange, createLeaveGuard } from "./leaveGuard.ts";

function change(applied = true): PendingChange {
  return {
    page: "nav.capture",
    restarts: true,
    apply: vi.fn(() => Promise.resolve(applied)),
    discard: vi.fn(),
  };
}

describe("leave guard", () => {
  test("leaving_without_changes_runs_the_action_without_asking", () => {
    const guard = createLeaveGuard();
    const action = vi.fn();
    guard.leave(action);
    expect(action).toHaveBeenCalledOnce();
    expect(guard.request()).toBeNull();
  });

  test("navigating_away_from_a_pending_change_asks_first", () => {
    const guard = createLeaveGuard();
    const pending = change();
    guard.set("capture", pending);
    const action = vi.fn();
    guard.leave(action);
    expect(action).not.toHaveBeenCalled();
    expect(guard.request()?.changes).toEqual([pending]);
  });

  test("apply_saves_then_navigates", async () => {
    const guard = createLeaveGuard();
    const pending = change();
    guard.set("capture", pending);
    const action = vi.fn();
    guard.leave(action);
    await guard.choose("apply");
    expect(pending.apply).toHaveBeenCalledOnce();
    expect(action).toHaveBeenCalledOnce();
    expect(guard.request()).toBeNull();
    expect(guard.pending()).toBe(false);
  });

  test("a_failed_apply_stays_on_the_page", async () => {
    const guard = createLeaveGuard();
    guard.set("capture", change(false));
    const action = vi.fn();
    guard.leave(action);
    await guard.choose("apply");
    expect(action).not.toHaveBeenCalled();
    expect(guard.request()).toBeNull();
    expect(guard.pending()).toBe(true);
  });

  test("discard_drops_the_draft_then_navigates", async () => {
    const guard = createLeaveGuard();
    const pending = change();
    guard.set("capture", pending);
    const action = vi.fn();
    guard.leave(action);
    await guard.choose("discard");
    expect(pending.discard).toHaveBeenCalledOnce();
    expect(pending.apply).not.toHaveBeenCalled();
    expect(action).toHaveBeenCalledOnce();
  });

  test("stay_keeps_the_draft_and_does_nothing", async () => {
    const guard = createLeaveGuard();
    const pending = change();
    guard.set("capture", pending);
    const action = vi.fn();
    guard.leave(action);
    await guard.choose("stay");
    expect(action).not.toHaveBeenCalled();
    expect(pending.discard).not.toHaveBeenCalled();
    expect(guard.request()).toBeNull();
    expect(guard.pending()).toBe(true);
  });

  test("closing_the_window_with_a_pending_change_asks_and_closes_after_the_choice", async () => {
    const guard = createLeaveGuard();
    guard.set("replay", change());
    const close = vi.fn();
    expect(guard.pending()).toBe(true);
    guard.leave(close);
    expect(close).not.toHaveBeenCalled();
    await guard.choose("discard");
    expect(close).toHaveBeenCalledOnce();
  });

  test("a_cleared_change_no_longer_blocks", () => {
    const guard = createLeaveGuard();
    guard.set("capture", change());
    guard.set("capture", null);
    const action = vi.fn();
    guard.leave(action);
    expect(action).toHaveBeenCalledOnce();
  });

  test("a_second_leave_while_asking_keeps_the_first_request", () => {
    const guard = createLeaveGuard();
    guard.set("capture", change());
    const first = vi.fn();
    guard.leave(first);
    guard.leave(vi.fn());
    expect(guard.request()?.action).toBe(first);
  });

  test("subscribers_hear_when_the_dialog_opens_and_closes", async () => {
    const guard = createLeaveGuard();
    const listener = vi.fn();
    guard.subscribe(listener);
    guard.set("capture", change());
    guard.leave(vi.fn());
    await guard.choose("stay");
    expect(listener).toHaveBeenCalledTimes(2);
  });
});
