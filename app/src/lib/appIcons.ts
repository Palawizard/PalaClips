import { appIcon } from "./ipc.ts";

const icons = new Map<string, Promise<string | null>>();

export function iconUrl(exe: string, processId: number): Promise<string | null> {
  const key = exe.toLowerCase();
  let icon = icons.get(key);
  if (!icon) {
    icon = appIcon(processId).then(
      (png) => URL.createObjectURL(new Blob([png], { type: "image/png" })),
      () => null,
    );
    icons.set(key, icon);
  }
  return icon;
}
