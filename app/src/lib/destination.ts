import type { View } from "../components/Rail.tsx";

export interface ClipRequest {
  path: string;
  returnTo: View;
  key: number;
}

export function clipRequest(clip: string | undefined, from: View, previous: ClipRequest | null): ClipRequest | null {
  return clip ? { path: clip, returnTo: from, key: (previous?.key ?? 0) + 1 } : null;
}

export function playerExit(request: ClipRequest | null): View {
  return request?.returnTo ?? "gallery";
}
