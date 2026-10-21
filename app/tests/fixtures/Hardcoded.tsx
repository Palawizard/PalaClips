const LABELS = { idle: "Désarmé" };
const OPTIONS = [{ value: "dark", label: "Dark" }];

export function Hardcoded({ armed }: { armed: boolean }) {
  return (
    <section aria-label="Replay status" className="flex items-center">
      <p>Replay armed</p>
      <p>{armed ? "Disarm" : LABELS.idle}</p>
      <p>{OPTIONS.length}</p>
    </section>
  );
}
