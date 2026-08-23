import type { Preferences, RunSettings } from "./contracts";

export function preferenceSnapshot(
  locale: Preferences["locale"],
  theme: Preferences["theme"],
  defaultSettings: RunSettings,
  palette: NonNullable<Preferences["palette"]> = "blue",
  playerVolume = 0.6,
): Preferences {
  return {
    locale,
    theme,
    palette,
    playerVolume: Math.max(0, Math.min(1, Number.isFinite(playerVolume) ? playerVolume : 0.6)),
    defaultSettings: { ...defaultSettings, overwrite: false },
  };
}

export async function persistPreferences(
  setPreferences: (preferences: Preferences) => Promise<Preferences>,
  locale: Preferences["locale"],
  theme: Preferences["theme"],
  defaultSettings: RunSettings,
  palette: NonNullable<Preferences["palette"]> = "blue",
  playerVolume = 0.6,
): Promise<Preferences> {
  return setPreferences(preferenceSnapshot(locale, theme, defaultSettings, palette, playerVolume));
}
