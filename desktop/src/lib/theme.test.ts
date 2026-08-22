import { afterEach, expect, test } from "vite-plus/test";
import { applyTheme, cachedTheme } from "./theme";

afterEach(() => {
  window.localStorage.clear();
  document.documentElement.removeAttribute("data-theme");
  document.documentElement.style.removeProperty("color-scheme");
});

test("theme cache defaults to light and applies a persisted explicit choice", () => {
  expect(cachedTheme()).toBe("light");
  applyTheme("dark");
  expect(document.documentElement.dataset.theme).toBe("dark");
  expect(document.documentElement.style.colorScheme).toBe("dark");
  expect(cachedTheme()).toBe("dark");
});
