export interface PendingChange {
  page: string;
  restarts: boolean;
  apply: () => Promise<boolean>;
  discard: () => void;
}

export type LeaveChoice = "apply" | "discard" | "stay";

export interface LeaveRequest {
  changes: PendingChange[];
  action: () => void;
}

export interface LeaveGuard {
  set: (id: string, change: PendingChange | null) => void;
  pending: () => boolean;
  request: () => LeaveRequest | null;
  leave: (action: () => void) => void;
  choose: (choice: LeaveChoice) => Promise<void>;
  subscribe: (listener: () => void) => () => void;
}

export function createLeaveGuard(): LeaveGuard {
  const changes = new Map<string, PendingChange>();
  const listeners = new Set<() => void>();
  let request: LeaveRequest | null = null;
  let choosing = false;

  const show = (next: LeaveRequest | null) => {
    request = next;
    for (const listener of listeners) listener();
  };

  return {
    set: (id, change) => {
      if (change) changes.set(id, change);
      else changes.delete(id);
    },
    pending: () => changes.size > 0,
    request: () => request,
    leave: (action) => {
      if (changes.size === 0) action();
      else if (!request) show({ changes: [...changes.values()], action });
    },
    choose: async (choice) => {
      const current = request;
      if (!current || choosing) return;
      if (choice === "stay") return show(null);
      if (choice === "apply") {
        choosing = true;
        let applied = false;
        try {
          applied = (await Promise.all(current.changes.map((change) => change.apply()))).every(Boolean);
        } catch {
          applied = false;
        } finally {
          choosing = false;
        }
        if (!applied) return show(null);
      } else {
        for (const change of current.changes) change.discard();
      }
      for (const [id, change] of changes) if (current.changes.includes(change)) changes.delete(id);
      show(null);
      current.action();
    },
    subscribe: (listener) => {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
  };
}
