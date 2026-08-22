import type { Preferences, RunSettings } from "./contracts";

export function preferenceSnapshot(
  locale: Preferences["locale"],
  theme: Preferences["theme"],
  defaultSettings: RunSettings,
): Preferences {
  return { locale, theme, defaultSettings: { ...defaultSettings, overwrite: false } };
}

export async function persistPreferences(
  setPreferences: (preferences: Preferences) => Promise<Preferences>,
  locale: Preferences["locale"],
  theme: Preferences["theme"],
  defaultSettings: RunSettings,
): Promise<Preferences> {
  return setPreferences(preferenceSnapshot(locale, theme, defaultSettings));
}
