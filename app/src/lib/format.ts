const decimal = new Intl.NumberFormat("fr-FR", { maximumFractionDigits: 1 });
const whole = new Intl.NumberFormat("fr-FR", { maximumFractionDigits: 0 });

export function clock(totalSeconds: number): string {
  const seconds = Math.max(0, Math.floor(totalSeconds));
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const rest = seconds % 60;
  const pad = (value: number) => value.toString().padStart(2, "0");
  return hours > 0 ? `${hours}:${pad(minutes)}:${pad(rest)}` : `${pad(minutes)}:${pad(rest)}`;
}

export function bytes(value: number): string {
  if (value >= 1e9) return `${decimal.format(value / 1e9)} Go`;
  if (value >= 1e6) return `${whole.format(value / 1e6)} Mo`;
  return `${whole.format(value / 1e3)} Ko`;
}

export function decibels(linear: number): number {
  return linear > 0 ? 20 * Math.log10(linear) : Number.NEGATIVE_INFINITY;
}

export function decibelLabel(db: number): string {
  return Number.isFinite(db) && db > -60 ? `${decimal.format(db)} dB` : "−∞";
}

export function relativeTime(epochMs: number, now = Date.now()): string {
  const minutes = Math.round((now - epochMs) / 60_000);
  if (minutes < 1) return "à l'instant";
  if (minutes < 60) return `il y a ${minutes} min`;
  const hours = Math.round(minutes / 60);
  if (hours < 24) return `il y a ${hours} h`;
  return new Date(epochMs).toLocaleDateString("fr-FR", { day: "numeric", month: "short" });
}
