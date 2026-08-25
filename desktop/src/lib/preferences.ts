import type { Preferences, RunSettings } from "./contracts";
import type { ThemeId } from "./theme-presets";

export function preferenceSnapshot(
  locale: Preferences["locale"],
  themeId: ThemeId,
  defaultSettings: RunSettings,
  playerVolume = 0.6,
): Preferences {
  return {
    locale,
    themeId,
    playerVolume: Math.max(0, Math.min(1, Number.isFinite(playerVolume) ? playerVolume : 0.6)),
    defaultSettings: { ...defaultSettings, overwrite: false },
  };
}

export async function persistPreferences(
  setPreferences: (preferences: Preferences) => Promise<Preferences>,
  locale: Preferences["locale"],
  themeId: ThemeId,
  defaultSettings: RunSettings,
  playerVolume = 0.6,
): Promise<Preferences> {
  return setPreferences(preferenceSnapshot(locale, themeId, defaultSettings, playerVolume));
}
