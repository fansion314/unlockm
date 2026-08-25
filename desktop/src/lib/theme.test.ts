import { afterEach, expect, test, vi } from "vite-plus/test";
import { applyTheme, cachedThemeId } from "./theme";

afterEach(() => {
  vi.restoreAllMocks();
  window.localStorage.clear();
  document.documentElement.removeAttribute("data-theme");
  document.documentElement.style.removeProperty("color-scheme");
});

test("theme cache defaults to Aurora Light and applies a complete preset", () => {
  expect(cachedThemeId()).toBe("aurora-light");
  applyTheme("tokyo-night");
  expect(document.documentElement.dataset.theme).toBe("tokyo-night");
  expect(document.documentElement.style.colorScheme).toBe("dark");
  expect(document.documentElement.classList.contains("is-theme-switching")).toBe(false);
  expect(cachedThemeId()).toBe("tokyo-night");
});

test("a valid canonical cache wins and legacy keys are removed after apply", () => {
  window.localStorage.setItem("unlockm-theme-id", "rose-pine-dawn");
  window.localStorage.setItem("unlockm-theme", "dark");
  window.localStorage.setItem("unlockm-palette", "amber");
  expect(cachedThemeId()).toBe("rose-pine-dawn");
  applyTheme(cachedThemeId());
  expect(window.localStorage.getItem("unlockm-theme-id")).toBe("rose-pine-dawn");
  expect(window.localStorage.getItem("unlockm-theme")).toBeNull();
  expect(window.localStorage.getItem("unlockm-palette")).toBeNull();
});

test.each([
  ["light", "blue", "aurora-light"],
  ["light", "emerald", "aurora-light"],
  ["light", "violet", "rose-pine-dawn"],
  ["light", "amber", "gruvbox-light"],
  ["dark", "blue", "aurora-dark"],
  ["dark", "emerald", "nord"],
  ["dark", "violet", "catppuccin-mocha"],
  ["dark", "amber", "gruvbox-dark"],
])("migrates legacy %s and %s cache to %s", (theme, palette, expected) => {
  window.localStorage.setItem("unlockm-theme", theme);
  window.localStorage.setItem("unlockm-palette", palette);
  expect(cachedThemeId()).toBe(expected);
});

test("unknown and unavailable cache safely fall back to Aurora Light", () => {
  window.localStorage.setItem("unlockm-theme-id", "not-a-theme");
  expect(cachedThemeId()).toBe("aurora-light");
  vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
    throw new Error("storage unavailable");
  });
  expect(cachedThemeId()).toBe("aurora-light");
  expect(() => applyTheme("aurora-dark")).not.toThrow();
});
