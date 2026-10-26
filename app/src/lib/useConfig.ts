import { useSyncExternalStore } from "react";
import { type ConfigPatch, createConfigStore } from "./config.ts";
import { type EngineConfig, getConfig, onConfigChanged, setConfig } from "./ipc.ts";

const store = createConfigStore<EngineConfig>({
  load: getConfig,
  subscribe: (handler) => {
    void onConfigChanged(handler);
  },
});

export function useLiveConfig(): EngineConfig | null {
  return useSyncExternalStore(store.listen, store.current);
}

export const configReady = (): Promise<EngineConfig> => store.ready();

export function saveConfig(patch: ConfigPatch): Promise<EngineConfig> {
  return setConfig(patch).then((config) => {
    store.publish(config);
    return config;
  });
}
