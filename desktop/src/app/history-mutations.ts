import type { PlaybackController } from "../features/playback/PlaybackProvider";

export async function deleteHistoryRun(
  controller: PlaybackController | null,
  runId: number,
  remove: (runId: number) => Promise<unknown>,
) {
  controller?.releaseForRun(runId);
  return remove(runId);
}

export async function clearHistoryRecords(
  controller: PlaybackController | null,
  clear: () => Promise<unknown>,
) {
  controller?.releaseAll();
  return clear();
}

/** A same-ID rerun replaces item records, so its old audio identity must be
 * detached before Rust resets the run. Failed reruns intentionally do not
 * restore that stale item ID. */
export async function rerunHistoryRun<T>(
  controller: PlaybackController | null,
  runId: number,
  rerun: () => Promise<T>,
) {
  controller?.releaseForRun(runId);
  return rerun();
}
