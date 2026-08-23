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
