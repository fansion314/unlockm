export function formatDuration(milliseconds: number | null, locale: string): string {
  if (milliseconds === null) {
    return "-";
  }
  const seconds = Math.floor(milliseconds / 1_000);
  const minutes = Math.floor(seconds / 60);
  const remainder = String(seconds % 60).padStart(2, "0");
  return new Intl.NumberFormat(locale, { useGrouping: false }).format(minutes) + `:${remainder}`;
}

export function formatBytes(bytes: number | null, locale: string): string {
  if (bytes === null) return "-";
  const units = ["B", "KB", "MB", "GB"];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${new Intl.NumberFormat(locale, {
    maximumFractionDigits: unit === 0 ? 0 : 1,
  }).format(value)} ${units[unit]}`;
}

export function formatTimestamp(milliseconds: number, locale: string): string {
  return new Intl.DateTimeFormat(locale, {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(new Date(milliseconds));
}
