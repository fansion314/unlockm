import { expect, test, vi } from "vite-plus/test";
import { persistPreferences, preferenceSnapshot } from "./preferences";

test("quick settings are snapshotted and persisted through the narrow preference command", async () => {
  const settings = { updateMetadata: true, copyPassthrough: false, overwrite: true };
  const snapshot = preferenceSnapshot("zh-CN", "tokyo-night", settings);
  settings.overwrite = false;
  expect(snapshot.defaultSettings.overwrite).toBe(false);
  const setPreferences = vi.fn(async (value) => value);
  await persistPreferences(setPreferences, "zh-CN", "tokyo-night", snapshot.defaultSettings);
  expect(setPreferences).toHaveBeenCalledWith({
    locale: "zh-CN",
    themeId: "tokyo-night",
    playerVolume: 0.6,
    defaultSettings: snapshot.defaultSettings,
  });
});

test("player volume is clamped without turning explicit mute into full volume", () => {
  expect(
    preferenceSnapshot(
      "en-US",
      "rose-pine-dawn",
      { updateMetadata: false, copyPassthrough: true, overwrite: false },
      0,
    ).playerVolume,
  ).toBe(0);
  expect(
    preferenceSnapshot(
      "en-US",
      "aurora-light",
      { updateMetadata: false, copyPassthrough: true, overwrite: false },
      Number.NaN,
    ).playerVolume,
  ).toBe(0.6);
});
