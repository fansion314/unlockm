import { expect, test } from "vite-plus/test";
import { errorMessageKey, normalizeDesktopError } from "./errors";

test("maps stable command codes while retaining separate technical detail", () => {
  const error = normalizeDesktopError({ code: "busy", detail: "run 17 is active" });
  expect(errorMessageKey(error.code)).toBe("errors.busy");
  expect(error.detail).toBe("run 17 is active");
  expect(normalizeDesktopError(new Error("fallback")).code).toBe("unknown");
});
