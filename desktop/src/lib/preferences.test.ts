import { expect, test, vi } from "vite-plus/test";
import { persistPreferences, preferenceSnapshot } from "./preferences";

test("quick settings are snapshotted and persisted through the narrow preference command", async () => {
  const settings = { updateMetadata: true, copyPassthrough: false, overwrite: true };
  const snapshot = preferenceSnapshot("zh-CN", "dark", settings);
  settings.overwrite = false;
  expect(snapshot.defaultSettings.overwrite).toBe(false);
  const setPreferences = vi.fn(async (value) => value);
  await persistPreferences(setPreferences, "zh-CN", "dark", snapshot.defaultSettings);
  expect(setPreferences).toHaveBeenCalledWith({
    locale: "zh-CN",
    theme: "dark",
    defaultSettings: snapshot.defaultSettings,
  });
});
