import { expect, test, vi } from "vite-plus/test";
import { persistPreferences, preferenceSnapshot } from "./preferences";

test("quick settings are snapshotted and persisted through the narrow preference command", async () => {
  const settings = { updateMetadata: true, copyPassthrough: false, overwrite: true };
  const snapshot = preferenceSnapshot("zh-CN", settings);
  settings.overwrite = false;
  expect(snapshot.defaultSettings.overwrite).toBe(true);
  const setPreferences = vi.fn(async (value) => value);
  await persistPreferences(setPreferences, "zh-CN", snapshot.defaultSettings);
  expect(setPreferences).toHaveBeenCalledWith({
    locale: "zh-CN",
    defaultSettings: snapshot.defaultSettings,
  });
});
