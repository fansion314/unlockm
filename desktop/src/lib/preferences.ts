import type { Preferences, RunSettings } from "./contracts";

export function preferenceSnapshot(
  locale: Preferences["locale"],
  defaultSettings: RunSettings,
): Preferences {
  return { locale, defaultSettings: { ...defaultSettings } };
}

export async function persistPreferences(
  setPreferences: (preferences: Preferences) => Promise<Preferences>,
  locale: Preferences["locale"],
  defaultSettings: RunSettings,
): Promise<Preferences> {
  return setPreferences(preferenceSnapshot(locale, defaultSettings));
}
