export function formatDuration(milliseconds: number | null, locale: string): string {
  if (milliseconds === null) {
    return "—";
  }
  const seconds = Math.floor(milliseconds / 1_000);
  const minutes = Math.floor(seconds / 60);
  const remainder = String(seconds % 60).padStart(2, "0");
  return new Intl.NumberFormat(locale, { useGrouping: false }).format(minutes) + `:${remainder}`;
}

export function formatTimestamp(milliseconds: number, locale: string): string {
  return new Intl.DateTimeFormat(locale, {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(new Date(milliseconds));
}
