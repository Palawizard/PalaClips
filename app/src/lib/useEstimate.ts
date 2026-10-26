import { useEffect, useState } from "react";
import { type EngineConfig, type ReplayEstimate, estimateReplay } from "./ipc.ts";

export function useEstimate(config: EngineConfig | null): ReplayEstimate | null {
  const [estimate, setEstimate] = useState<ReplayEstimate | null>(null);
  useEffect(() => {
    if (!config) return;
    let current = true;
    estimateReplay(config).then(
      (value) => current && setEstimate(value),
      () => current && setEstimate(null),
    );
    return () => {
      current = false;
    };
  }, [config]);
  return estimate;
}
