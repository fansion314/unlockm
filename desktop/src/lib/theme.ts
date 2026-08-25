import {
  defaultThemeId,
  isThemeId,
  legacyThemeId,
  type ThemeId,
  themePreset,
} from "./theme-presets";

const THEME_ID_KEY = "unlockm-theme-id";
const LEGACY_THEME_KEY = "unlockm-theme";
const LEGACY_PALETTE_KEY = "unlockm-palette";

export function cachedThemeId(): ThemeId {
  try {
    const current = window.localStorage.getItem(THEME_ID_KEY);
    if (isThemeId(current)) return current;
    const legacyTheme = window.localStorage.getItem(LEGACY_THEME_KEY);
    const legacyPalette = window.localStorage.getItem(LEGACY_PALETTE_KEY);
    if (legacyTheme !== null || legacyPalette !== null) {
      return legacyThemeId(legacyTheme, legacyPalette);
    }
  } catch {
    // Rust preferences remain authoritative when browser storage is unavailable.
  }
  return defaultThemeId;
}

export function applyTheme(themeId: ThemeId): void {
  const preset = themePreset(themeId);
  const root = document.documentElement;
  root.classList.add("is-theme-switching");
  root.dataset.theme = preset.id;
  root.style.colorScheme = preset.scheme;
  // WKWebView can otherwise retain an old transitioned color/gradient when
  // its source custom property changes. Commit the new theme with transitions
  // disabled, then restore ordinary hover/state motion without a second value change.
  void root.offsetWidth;
  root.classList.remove("is-theme-switching");
  try {
    window.localStorage.setItem(THEME_ID_KEY, preset.id);
    window.localStorage.removeItem(LEGACY_THEME_KEY);
    window.localStorage.removeItem(LEGACY_PALETTE_KEY);
  } catch {
    // Rust preferences remain authoritative when browser storage is unavailable.
  }
}
