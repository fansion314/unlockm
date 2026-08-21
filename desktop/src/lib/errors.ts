export type DesktopError = { code: string; detail: string };

export function normalizeDesktopError(error: unknown): DesktopError {
  if (typeof error === "object" && error !== null && "code" in error && "detail" in error) {
    const payload = error as { code: unknown; detail: unknown };
    if (typeof payload.code === "string" && typeof payload.detail === "string") {
      return { code: payload.code, detail: payload.detail };
    }
  }
  return { code: "unknown", detail: String(error) };
}

export function errorMessageKey(code: string): string {
  return `errors.${code}`;
}
