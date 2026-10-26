import { useEffect, useRef } from "react";
import { type LibraryChange, onLibraryChanged } from "./ipc.ts";

export const REFRESH_DELAY_MS = 200;

type Subscribe = (handler: (change: LibraryChange) => void) => Promise<() => void>;

export function refreshOnLibraryChange(
  refresh: (change: LibraryChange) => void,
  subscribe: Subscribe = onLibraryChanged,
  delayMs = REFRESH_DELAY_MS,
): () => void {
  let timer: ReturnType<typeof setTimeout> | undefined;
  let removed: string[] = [];
  const unlisten = subscribe((change) => {
    removed.push(...change.removed);
    clearTimeout(timer);
    timer = setTimeout(() => {
      const batch = { removed };
      removed = [];
      refresh(batch);
    }, delayMs);
  });
  return () => {
    clearTimeout(timer);
    unlisten.then((stop) => stop()).catch(() => undefined);
  };
}

export function useLibraryChanges(refresh: (change: LibraryChange) => void) {
  const latest = useRef(refresh);
  latest.current = refresh;
  useEffect(() => refreshOnLibraryChange((change) => latest.current(change)), []);
}
