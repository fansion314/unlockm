export const themeIds = [
  "aurora-light",
  "aurora-dark",
  "tokyo-night",
  "catppuccin-mocha",
  "dracula",
  "nord",
  "gruvbox-light",
  "gruvbox-dark",
  "rose-pine-dawn",
] as const;

export type ThemeId = (typeof themeIds)[number];
export type ThemeScheme = "light" | "dark";

export type ThemePreset = {
  id: ThemeId;
  scheme: ThemeScheme;
  labelKey: `app.themes.${ThemeId}.name`;
  descriptionKey: `app.themes.${ThemeId}.description`;
  preview: readonly [surface: string, primary: string, secondary: string, tertiary: string];
};

export const defaultThemeId: ThemeId = "aurora-light";

export const themePresets: readonly ThemePreset[] = [
  {
    id: "aurora-light",
    scheme: "light",
    labelKey: "app.themes.aurora-light.name",
    descriptionKey: "app.themes.aurora-light.description",
    preview: ["#f7f8ff", "#5154d8", "#087f87", "#c64f6b"],
  },
  {
    id: "aurora-dark",
    scheme: "dark",
    labelKey: "app.themes.aurora-dark.name",
    descriptionKey: "app.themes.aurora-dark.description",
    preview: ["#121424", "#a3a8ff", "#58d5c9", "#ff8fa3"],
  },
  {
    id: "tokyo-night",
    scheme: "dark",
    labelKey: "app.themes.tokyo-night.name",
    descriptionKey: "app.themes.tokyo-night.description",
    preview: ["#16161e", "#7aa2f7", "#bb9af7", "#7dcfff"],
  },
  {
    id: "catppuccin-mocha",
    scheme: "dark",
    labelKey: "app.themes.catppuccin-mocha.name",
    descriptionKey: "app.themes.catppuccin-mocha.description",
    preview: ["#1e1e2e", "#cba6f7", "#89b4fa", "#fab387"],
  },
  {
    id: "dracula",
    scheme: "dark",
    labelKey: "app.themes.dracula.name",
    descriptionKey: "app.themes.dracula.description",
    preview: ["#282a36", "#bd93f9", "#8be9fd", "#ff79c6"],
  },
  {
    id: "nord",
    scheme: "dark",
    labelKey: "app.themes.nord.name",
    descriptionKey: "app.themes.nord.description",
    preview: ["#2e3440", "#88c0d0", "#81a1c1", "#b48ead"],
  },
  {
    id: "gruvbox-light",
    scheme: "light",
    labelKey: "app.themes.gruvbox-light.name",
    descriptionKey: "app.themes.gruvbox-light.description",
    preview: ["#fbf1c7", "#076678", "#b57614", "#8f3f71"],
  },
  {
    id: "gruvbox-dark",
    scheme: "dark",
    labelKey: "app.themes.gruvbox-dark.name",
    descriptionKey: "app.themes.gruvbox-dark.description",
    preview: ["#282828", "#83a598", "#fabd2f", "#d3869b"],
  },
  {
    id: "rose-pine-dawn",
    scheme: "light",
    labelKey: "app.themes.rose-pine-dawn.name",
    descriptionKey: "app.themes.rose-pine-dawn.description",
    preview: ["#faf4ed", "#d7827e", "#56949f", "#ea9d34"],
  },
] as const;

const themePresetById = new Map(themePresets.map((preset) => [preset.id, preset]));

export function isThemeId(value: unknown): value is ThemeId {
  return typeof value === "string" && themePresetById.has(value as ThemeId);
}

export function themePreset(themeId: ThemeId): ThemePreset {
  return themePresetById.get(themeId) ?? themePresetById.get(defaultThemeId)!;
}

export function legacyThemeId(theme: string | null, palette: string | null): ThemeId {
  if (theme !== "dark") {
    if (palette === "violet") return "rose-pine-dawn";
    if (palette === "amber") return "gruvbox-light";
    return "aurora-light";
  }
  if (palette === "emerald") return "nord";
  if (palette === "violet") return "catppuccin-mocha";
  if (palette === "amber") return "gruvbox-dark";
  return "aurora-dark";
}
