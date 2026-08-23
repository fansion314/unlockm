/** Converts a native monitor factor into a deliberately small density
 * adjustment. WebView already performs DPI scaling, so this is never CSS zoom. */
export function displayDensity(platform: string, nativeFactor: number): number {
  if (!Number.isFinite(nativeFactor)) return 1;
  const factor = Math.max(0.5, Math.min(4, nativeFactor));
  if (platform === "darwin") return 1;
  if (platform === "win32") return Math.max(0.96, Math.min(1.08, 1 + (factor - 1) * 0.04));
  return 1;
}

export function applyDisplayDensity(platform: string, nativeFactor: number): void {
  document.documentElement.style.setProperty(
    "--native-density",
    String(displayDensity(platform, nativeFactor)),
  );
}

export async function listenDisplayScale(): Promise<() => void> {
  try {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    const current = getCurrentWindow();
    const os = /Mac/i.test(navigator.platform)
      ? "darwin"
      : /Win/i.test(navigator.platform)
        ? "win32"
        : "other";
    applyDisplayDensity(os, await current.scaleFactor());
    return await current.onScaleChanged(({ payload }) =>
      applyDisplayDensity(os, payload.scaleFactor),
    );
  } catch {
    return () => undefined;
  }
}
