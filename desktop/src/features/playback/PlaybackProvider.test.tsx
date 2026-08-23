import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vite-plus/test";
import type { HistoryItem } from "../../lib/contracts";
import { PlaybackProvider, usePlayback } from "./PlaybackProvider";

const { convertFileSrc } = vi.hoisted(() => ({
  convertFileSrc: vi.fn((path: string, protocol: string) => `${protocol}:///${path}`),
}));
vi.mock("@tauri-apps/api/core", () => ({ convertFileSrc }));

const track: HistoryItem = {
  id: 17,
  runId: 1,
  ordinal: 0,
  itemKind: "music",
  sourcePath: "/source.mp3",
  relativePath: "source.mp3",
  destinationPath: "/output.mp3",
  action: "copy",
  outcome: "copied",
  logicalBytes: 1,
  outputBytes: 1,
  outputFormat: "mp3",
  errorCode: null,
  errorDetail: null,
  warnings: [],
  title: "Safe volume",
  artist: null,
  album: null,
  durationMs: null,
  artworkId: null,
};
let releasedSnapshot: ReturnType<ReturnType<typeof usePlayback>["releaseForItem"]> = null;

function Controls() {
  const playback = usePlayback();
  return (
    <>
      <button type="button" onClick={() => void playback.toggle(track)}>
        Play
      </button>
      <button type="button" onClick={() => playback.setVolume(0)}>
        Zero
      </button>
      <button type="button" onClick={playback.toggleMute}>
        Mute
      </button>
      <button
        type="button"
        onClick={() => {
          releasedSnapshot = playback.releaseForItem(track.id);
        }}
      >
        Release
      </button>
      <button
        type="button"
        onClick={() =>
          void playback.restoreReleased(releasedSnapshot, { ...track, title: "Updated" })
        }
      >
        Restore
      </button>
      <output>{playback.volume}</output>
    </>
  );
}

beforeEach(() => {
  vi.spyOn(HTMLMediaElement.prototype, "pause").mockImplementation(() => undefined);
  vi.spyOn(HTMLMediaElement.prototype, "load").mockImplementation(() => undefined);
});

afterEach(() => {
  cleanup();
  vi.useRealTimers();
  vi.restoreAllMocks();
  convertFileSrc.mockClear();
});

test("writes 60% before the first play", async () => {
  let volumeAtPlay = -1;
  vi.spyOn(HTMLMediaElement.prototype, "play").mockImplementation(
    function (this: HTMLMediaElement) {
      volumeAtPlay = this.volume;
      return Promise.resolve();
    },
  );
  render(
    <PlaybackProvider volume={0.6}>
      <Controls />
    </PlaybackProvider>,
  );
  fireEvent.click(screen.getByRole("button", { name: "Play" }));
  await waitFor(() => expect(convertFileSrc).toHaveBeenCalledWith("17", "unlockm-audio"));
  expect(volumeAtPlay).toBe(0.6);
});

test("persists explicit zero and mute restores the most recent audible volume", async () => {
  vi.useFakeTimers();
  const onVolumeChange = vi.fn();
  render(
    <PlaybackProvider volume={0.42} onVolumeChange={onVolumeChange}>
      <Controls />
    </PlaybackProvider>,
  );
  fireEvent.click(screen.getByRole("button", { name: "Zero" }));
  expect(screen.getByRole("status").textContent).toBe("0");
  await act(async () => {
    vi.advanceTimersByTime(220);
  });
  expect(onVolumeChange).toHaveBeenLastCalledWith(0);
  fireEvent.click(screen.getByRole("button", { name: "Mute" }));
  expect(screen.getByRole("status").textContent).toBe("0.42");
});

test("accepts a persisted volume that arrives after bootstrap", () => {
  const view = render(
    <PlaybackProvider volume={0.6}>
      <Controls />
    </PlaybackProvider>,
  );
  view.rerender(
    <PlaybackProvider volume={0.27}>
      <Controls />
    </PlaybackProvider>,
  );
  expect(screen.getByRole("status").textContent).toBe("0.27");
});

test("release detaches media before restore loads the updated item", async () => {
  const order: string[] = [];
  vi.spyOn(HTMLMediaElement.prototype, "pause").mockImplementation(() => order.push("pause"));
  vi.spyOn(HTMLMediaElement.prototype, "removeAttribute").mockImplementation(() =>
    order.push("remove"),
  );
  vi.spyOn(HTMLMediaElement.prototype, "load").mockImplementation(() => order.push("load"));
  vi.spyOn(HTMLMediaElement.prototype, "play").mockImplementation(() => {
    order.push("play");
    return Promise.resolve();
  });
  render(
    <PlaybackProvider>
      <Controls />
    </PlaybackProvider>,
  );
  fireEvent.click(screen.getByRole("button", { name: "Play" }));
  await waitFor(() => expect(order).toContain("play"));
  fireEvent.play(document.querySelector("audio")!);
  order.length = 0;
  fireEvent.click(screen.getByRole("button", { name: "Release" }));
  expect(order.slice(0, 3)).toEqual(["pause", "remove", "load"]);
  order.length = 0;
  fireEvent.click(screen.getByRole("button", { name: "Restore" }));
  await waitFor(() => expect(order).toEqual(["pause", "load", "play"]));
});
