import type { RunSettings, StartConversionRequest, ValidatedInput } from "./contracts";

export function conversionRequest(
  input: ValidatedInput,
  outputPath: string | null,
  settings: RunSettings,
): StartConversionRequest {
  return {
    inputPath: input.path,
    outputPath,
    settings: { ...settings, overwrite: false },
  };
}
