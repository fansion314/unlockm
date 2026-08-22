import { expect, test } from "vite-plus/test";
import { conversionRequest } from "./conversion-request";

test("conversion request snapshots the selected root output and quick settings", () => {
  const settings = { updateMetadata: true, copyPassthrough: false, overwrite: true };
  const request = conversionRequest(
    { path: "/input/album", kind: "directory", defaultOutputPath: "/input/album" },
    "/chosen/output",
    settings,
  );
  settings.overwrite = false;
  expect(request).toEqual({
    inputPath: "/input/album",
    outputPath: "/chosen/output",
    settings: { updateMetadata: true, copyPassthrough: false, overwrite: false },
  });
});
