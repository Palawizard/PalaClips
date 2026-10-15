import { useEffect, useState } from "react";
import { engineVersion } from "./lib/ipc.ts";

export default function App() {
  const [version, setVersion] = useState<string>("…");

  useEffect(() => {
    engineVersion()
      .then(setVersion)
      .catch(() => setVersion("indisponible"));
  }, []);

  return (
    <main className="grid min-h-screen place-items-center bg-neutral-950 text-neutral-100">
      <div className="text-center">
        <h1 className="text-2xl font-semibold tracking-tight">PalaClips</h1>
        <p className="mt-2 text-sm text-neutral-400">Moteur {version}</p>
      </div>
    </main>
  );
}
