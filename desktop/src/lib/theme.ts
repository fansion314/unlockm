import type { Preferences } from "./contracts";

const THEME_KEY = "unlockm-theme";

export function cachedTheme(): Preferences["theme"] {
  try {
    return window.localStorage.getItem(THEME_KEY) === "dark" ? "dark" : "light";
  } catch {
    return "light";
  }
}

export function applyTheme(theme: Preferences["theme"]): void {
  document.documentElement.dataset.theme = theme;
  document.documentElement.style.colorScheme = theme;
  try {
    window.localStorage.setItem(THEME_KEY, theme);
  } catch {
    // The Rust preference remains authoritative when localStorage is unavailable.
  }
}
