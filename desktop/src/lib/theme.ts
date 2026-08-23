import type { Preferences } from "./contracts";

const THEME_KEY = "unlockm-theme";
const PALETTE_KEY = "unlockm-palette";

export function cachedTheme(): Preferences["theme"] {
  try {
    return window.localStorage.getItem(THEME_KEY) === "dark" ? "dark" : "light";
  } catch {
    return "light";
  }
}

export function applyTheme(
  theme: Preferences["theme"],
  palette: NonNullable<Preferences["palette"]> = cachedPalette(),
): void {
  document.documentElement.dataset.theme = theme;
  document.documentElement.dataset.palette = palette;
  document.documentElement.style.colorScheme = theme;
  try {
    window.localStorage.setItem(THEME_KEY, theme);
    window.localStorage.setItem(PALETTE_KEY, palette);
  } catch {
    // The Rust preference remains authoritative when localStorage is unavailable.
  }
}

export function cachedPalette(): NonNullable<Preferences["palette"]> {
  try {
    const value = window.localStorage.getItem(PALETTE_KEY);
    return value === "emerald" || value === "violet" || value === "amber" ? value : "blue";
  } catch {
    return "blue";
  }
}
