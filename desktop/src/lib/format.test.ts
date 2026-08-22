import { expect, test } from "vite-plus/test";
import { formatBytes, formatDuration, formatTimestamp } from "./format";

test("formats duration and timestamps with Intl", () => {
  expect(formatDuration(65_000, "en-US")).toBe("1:05");
  expect(formatDuration(null, "zh-CN")).toBe("-");
  expect(formatBytes(1_572_864, "en-US")).toBe("1.5 MB");
  expect(formatTimestamp(0, "en-US")).toContain("1970");
});
