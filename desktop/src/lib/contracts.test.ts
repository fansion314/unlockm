import { expect, test } from "vite-plus/test";
import bootstrapFixture from "../contracts/bootstrap.fixture.json";
import type { BootstrapResponse } from "./contracts";

test("Rust bootstrap fixture preserves the camelCase IPC contract", () => {
  const bootstrap = bootstrapFixture as unknown as BootstrapResponse;
  expect(bootstrap.productName).toBe("UnlockM");
  expect(bootstrap.history.nextCursor).toBeNull();
  expect(bootstrap.preferences.themeId).toBe("aurora-light");
  expect(bootstrap.preferences.defaultSettings.copyPassthrough).toBe(true);
  expect(bootstrap.preferences.playerVolume).toBe(0.6);
});
