import { getCurrentWindow } from "@tauri-apps/api/window";
import { type ReactNode, createContext, useContext, useEffect, useRef, useState, useSyncExternalStore } from "react";
import { type Key, useT } from "../lib/i18n.ts";
import { type LeaveGuard, type PendingChange, createLeaveGuard } from "../lib/leaveGuard.ts";

const GuardContext = createContext<LeaveGuard | null>(null);

export function useLeaveGuard(): LeaveGuard {
  const guard = useContext(GuardContext);
  if (!guard) throw new Error("useLeaveGuard outside LeaveGuardProvider");
  return guard;
}

export function usePendingChange(id: string, change: PendingChange | null) {
  const guard = useContext(GuardContext);
  const latest = useRef(change);
  latest.current = change;
  const active = change !== null;
  const restarts = change?.restarts ?? false;
  const page = change?.page ?? "";
  useEffect(() => {
    if (!guard) return;
    guard.set(
      id,
      active
        ? {
            page,
            restarts,
            apply: () => latest.current?.apply() ?? Promise.resolve(true),
            discard: () => latest.current?.discard(),
          }
        : null,
    );
    return () => guard.set(id, null);
  }, [guard, id, active, restarts, page]);
}

export function LeaveGuardProvider({ children }: { children: ReactNode }) {
  const [guard] = useState(createLeaveGuard);

  useEffect(() => {
    const window = getCurrentWindow();
    const unlisten = window.onCloseRequested((event) => {
      if (!guard.pending()) return;
      event.preventDefault();
      guard.leave(() => void window.destroy());
    });
    return () => {
      unlisten.then((stop) => stop());
    };
  }, [guard]);

  return (
    <GuardContext.Provider value={guard}>
      {children}
      <LeaveDialog guard={guard} />
    </GuardContext.Provider>
  );
}

function LeaveDialog({ guard }: { guard: LeaveGuard }) {
  const t = useT();
  const request = useSyncExternalStore(guard.subscribe, guard.request);
  const dialog = useRef<HTMLDialogElement>(null);
  const applyButton = useRef<HTMLButtonElement>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    const element = dialog.current;
    if (!element) return;
    if (request && !element.open) {
      element.showModal();
      applyButton.current?.focus();
    } else if (!request && element.open) element.close();
  }, [request]);

  const choose = (choice: "apply" | "discard" | "stay") => {
    setBusy(choice === "apply");
    guard.choose(choice).finally(() => setBusy(false));
  };

  const pages = request?.changes.map((change) => t(change.page as Key)) ?? [];
  const restarts = request?.changes.some((change) => change.restarts) ?? false;

  return (
    <dialog
      ref={dialog}
      aria-labelledby="leave-title"
      aria-describedby="leave-body"
      onCancel={(event) => {
        event.preventDefault();
        choose("stay");
      }}
      className="leave-dialog m-auto w-[min(440px,calc(100vw-48px))] rounded-md border border-line-strong bg-surface-2 p-0 text-ink"
    >
      <div className="px-5 pt-5 pb-4">
        <h2 id="leave-title" className="text-[17px] font-semibold">
          {t("leave.title")}
        </h2>
        <div id="leave-body" className="mt-2 text-[14px] text-ink-2">
          <p>{t("leave.pages", { pages: pages.join(", ") })}</p>
          {restarts && <p className="mt-1">{t("settings.restartWarning")}</p>}
        </div>
      </div>
      <div className="flex flex-wrap justify-end gap-2 border-t border-line px-5 py-3">
        <button
          type="button"
          onClick={() => choose("stay")}
          className="pressable h-9 rounded-md px-3.5 font-medium text-ink-2 hover:bg-surface-hover hover:text-ink"
        >
          {t("leave.stay")}
        </button>
        <button
          type="button"
          onClick={() => choose("discard")}
          disabled={busy}
          className="pressable h-9 rounded-md border border-line-strong px-3.5 font-medium text-ink hover:bg-surface-hover disabled:text-ink-3"
        >
          {t("leave.discard")}
        </button>
        <button
          ref={applyButton}
          type="button"
          onClick={() => choose("apply")}
          disabled={busy}
          className="pressable h-9 rounded-md bg-ink px-3.5 font-semibold text-ground hover:bg-ink/90 disabled:cursor-wait disabled:opacity-70"
        >
          {t("settings.apply")}
        </button>
      </div>
    </dialog>
  );
}
