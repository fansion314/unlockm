import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { expect, test } from "vite-plus/test";
import { themePresets } from "./theme-presets";

const cssFiles = [
  "src/styles/tokens.css",
  "src/styles/themes.css",
  "src/index.css",
  "src/App.css",
  "src/styles/history.css",
  "src/styles/player.css",
] as const;
const cssByFile = new Map(
  cssFiles.map((file) => [file, readFileSync(resolve(process.cwd(), file), "utf8")]),
);
const themeCss = cssByFile.get("src/styles/themes.css")!;

const requiredThemeTokens = [
  "color-accent",
  "color-accent-hover",
  "color-accent-secondary",
  "color-accent-tertiary",
  "color-accent-contrast",
  "color-danger",
  "color-success",
  "color-warning",
  "color-focus",
  "surface-app",
  "surface-app-alt",
  "surface-header",
  "surface-card",
  "surface-card-tinted",
  "surface-raised",
  "surface-soft",
  "surface-control",
  "surface-hover",
  "surface-selected",
  "surface-active",
  "surface-danger",
  "surface-success",
  "surface-skeleton",
  "surface-skeleton-highlight",
  "text-primary",
  "text-secondary",
  "text-muted",
  "text-disabled",
  "text-on-accent",
  "text-on-strong-surface",
  "border-subtle",
  "border-control",
  "border-strong",
  "border-accent",
  "ambient-primary",
  "ambient-secondary",
  "ambient-tertiary",
  "gradient-app",
  "gradient-hero",
  "gradient-action",
  "overlay-backdrop",
  "shadow-float",
  "shadow-menu",
  "shadow-colored",
] as const;

function themeBlock(themeId: string): string {
  const selector = `:root[data-theme="${themeId}"]`;
  const selectorStart = themeCss.indexOf(selector);
  if (selectorStart < 0) return "";
  const blockStart = themeCss.indexOf("{", selectorStart);
  const blockEnd = themeCss.indexOf("\n}", blockStart);
  return blockStart < 0 || blockEnd < 0 ? "" : themeCss.slice(blockStart + 1, blockEnd);
}

function hexToken(block: string, token: string): string {
  const value = block.match(new RegExp(`--${token}:\\s*(#[0-9a-f]{6})\\s*;`, "i"))?.[1];
  if (!value) throw new Error(`missing solid color --${token}`);
  return value;
}

function relativeLuminance(hex: string): number {
  const channels = [1, 3, 5].map(
    (offset) => Number.parseInt(hex.slice(offset, offset + 2), 16) / 255,
  );
  const [red, green, blue] = channels.map((channel) =>
    channel <= 0.04045 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4,
  );
  return red * 0.2126 + green * 0.7152 + blue * 0.0722;
}

function contrastRatio(foreground: string, background: string): number {
  const lighter = Math.max(relativeLuminance(foreground), relativeLuminance(background));
  const darker = Math.min(relativeLuminance(foreground), relativeLuminance(background));
  return (lighter + 0.05) / (darker + 0.05);
}

function contrastForTokens(
  block: string,
  foregroundToken: string,
  backgroundToken: string,
): number {
  return contrastRatio(hexToken(block, foregroundToken), hexToken(block, backgroundToken));
}

test("every theme owns a complete semantic token block", () => {
  for (const preset of themePresets) {
    const block = themeBlock(preset.id);
    expect(block, `${preset.id} selector must exist`).not.toBe("");
    for (const token of requiredThemeTokens) {
      expect(block, `${preset.id} must define --${token}`).toContain(`--${token}:`);
    }
    for (const previewColor of preset.preview) {
      expect(block.toLowerCase(), `${preset.id} preview must come from its theme block`).toContain(
        previewColor,
      );
    }
  }
});

test("component styles do not retain palette-specific selectors", () => {
  expect(themeCss).not.toContain("data-palette");
});

test("component styles keep theme colors and semantic variables inside the contract", () => {
  const componentCss = [
    cssByFile.get("src/index.css"),
    cssByFile.get("src/App.css"),
    cssByFile.get("src/styles/history.css"),
    cssByFile.get("src/styles/player.css"),
  ].join("\n");
  expect(componentCss.match(/#[0-9a-f]{3,8}|rgb\(/gi) ?? []).toEqual([]);
  expect(componentCss).not.toMatch(/transition\s*:\s*all/i);
  expect(componentCss).not.toMatch(/transition\s*:[^;}]*\bbackground\s/gi);
  expect(componentCss).not.toMatch(/data-(?:theme|palette)/i);
  expect(componentCss).toContain("@media (prefers-reduced-motion: reduce)");
  expect(componentCss).toContain("@media (prefers-reduced-transparency: reduce)");
  expect(componentCss).toContain("@media (forced-colors: active)");

  const allCss = [...cssByFile.values()].join("\n");
  const declared = new Set([...allCss.matchAll(/(--[a-z0-9-]+)\s*:/gi)].map((match) => match[1]));
  const runtimeTokens = new Set(["--title-track", "--album-track"]);
  const used = new Set(
    [...componentCss.matchAll(/var\((--[a-z0-9-]+)/gi)].map((match) => match[1]),
  );
  expect([...used].filter((token) => !declared.has(token) && !runtimeTokens.has(token))).toEqual(
    [],
  );
});

test("every preset meets text, action, state, and focus contrast gates", () => {
  const failures: string[] = [];
  for (const preset of themePresets) {
    const block = themeBlock(preset.id);
    const check = (foreground: string, background: string, minimum: number) => {
      const ratio = contrastForTokens(block, foreground, background);
      if (ratio < minimum) {
        failures.push(
          `${preset.id}: --${foreground} on --${background} = ${ratio.toFixed(2)} < ${minimum}`,
        );
      }
    };
    for (const surface of ["surface-app", "surface-card", "surface-control"] as const) {
      check("text-primary", surface, 4.5);
      check("text-secondary", surface, 4.5);
    }
    check("text-muted", "surface-card", 4.5);
    check("text-on-accent", "color-accent", 4.5);
    check("color-danger", "surface-danger", 4.5);
    check("color-success", "surface-success", 4.5);
    check("color-warning", "surface-card", 4.5);
    check("color-focus", "surface-app", 3);
    check("border-strong", "surface-control", 3);

    const actionValue = block.match(/--gradient-action:\s*([^;]+);/i)?.[1] ?? "";
    const actionStops = actionValue.match(/#[0-9a-f]{6}/gi) ?? [];
    expect(
      actionStops.length,
      `${preset.id}: action gradient needs multiple stops`,
    ).toBeGreaterThan(1);
    for (const stop of actionStops) {
      const ratio = contrastRatio(hexToken(block, "text-on-accent"), stop);
      if (ratio < 4.5) {
        failures.push(
          `${preset.id}: --text-on-accent on action stop ${stop} = ${ratio.toFixed(2)} < 4.5`,
        );
      }
    }
  }
  expect(failures).toEqual([]);
});
