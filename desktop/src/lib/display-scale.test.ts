import { expect, test } from "vite-plus/test";
import { displayDensity } from "./display-scale";

test("native scale makes only a bounded Windows density adjustment", () => {
  expect(displayDensity("darwin", 2)).toBe(1);
  expect(displayDensity("win32", 1)).toBe(1);
  expect(displayDensity("win32", 4)).toBe(1.08);
  expect(displayDensity("win32", 0.1)).toBe(0.98);
  expect(displayDensity("other", 2)).toBe(1);
});
