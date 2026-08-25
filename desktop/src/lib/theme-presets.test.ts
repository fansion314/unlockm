import { expect, test } from "vite-plus/test";
import {
  defaultThemeId,
  isThemeId,
  legacyThemeId,
  themeIds,
  themePresets,
  themePreset,
} from "./theme-presets";

test("theme registry exposes nine stable complete presets", () => {
  expect(themeIds).toHaveLength(9);
  expect(new Set(themeIds).size).toBe(themeIds.length);
  expect(themePresets.map(({ id }) => id)).toEqual(themeIds);
  expect(themePreset(defaultThemeId).scheme).toBe("light");

  for (const preset of themePresets) {
    expect(preset.labelKey).toBe(`app.themes.${preset.id}.name`);
    expect(preset.descriptionKey).toBe(`app.themes.${preset.id}.description`);
    expect(preset.preview).toHaveLength(4);
    expect(new Set(preset.preview).size).toBe(4);
    expect(preset.preview.every((color) => /^#[0-9a-f]{6}$/i.test(color))).toBe(true);
    expect(isThemeId(preset.id)).toBe(true);
  }
  expect(isThemeId("blue")).toBe(false);
});

test("legacy light, dark, and palette combinations map deterministically", () => {
  expect(legacyThemeId("light", "blue")).toBe("aurora-light");
  expect(legacyThemeId("light", "emerald")).toBe("aurora-light");
  expect(legacyThemeId("light", "violet")).toBe("rose-pine-dawn");
  expect(legacyThemeId("light", "amber")).toBe("gruvbox-light");
  expect(legacyThemeId("dark", "blue")).toBe("aurora-dark");
  expect(legacyThemeId("dark", "emerald")).toBe("nord");
  expect(legacyThemeId("dark", "violet")).toBe("catppuccin-mocha");
  expect(legacyThemeId("dark", "amber")).toBe("gruvbox-dark");
  expect(legacyThemeId(null, null)).toBe(defaultThemeId);
});
