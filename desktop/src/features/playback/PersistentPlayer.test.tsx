import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, expect, test, vi } from "vite-plus/test";
import "../../i18n";
import type { HistoryItem } from "../../lib/contracts";

const api = vi.hoisted(() => ({ getArtwork: vi.fn(), getArtworkMime: vi.fn() }));
const playback = vi.hoisted(() => ({
  activeItemId: 1,
  track: null as HistoryItem | null,
  playing: false,
  currentTime: 0,
  duration: 30,
  volume: 0.6,
  muted: false,
  error: null as string | null,
  toggle: vi.fn(async () => undefined),
  seek: vi.fn(),
  toggleMute: vi.fn(),
  setVolume: vi.fn(),
}));

vi.mock("../../lib/ipc", () => ({ desktopApi: api }));
vi.mock("./PlaybackProvider", () => ({ usePlayback: () => playback }));

import { PersistentPlayer } from "./PersistentPlayer";

const baseTrack: HistoryItem = {
  id: 1,
  runId: 1,
  ordinal: 0,
  itemKind: "music",
  sourcePath: "source.mp3",
  relativePath: "source.mp3",
  destinationPath: "output.mp3",
  action: "copy",
  outcome: "copied",
  logicalBytes: 1,
  outputBytes: 1,
  outputFormat: "mp3",
  errorCode: null,
  errorDetail: null,
  warnings: [],
  title: "Now playing",
  artist: "Artist",
  album: "Album",
  durationMs: 30_000,
  artworkId: 1,
};

beforeEach(() => {
  playback.track = { ...baseTrack };
  playback.playing = false;
  api.getArtworkMime.mockResolvedValue({ mimeType: "image/png" });
  api.getArtwork.mockResolvedValue(new ArrayBuffer(4));
});

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  api.getArtwork.mockReset();
  api.getArtworkMime.mockReset();
  playback.toggle.mockReset();
  playback.seek.mockReset();
  playback.toggleMute.mockReset();
  playback.setVolume.mockReset();
});

test("artwork changes immediately clear old URLs and late requests cannot repaint them", async () => {
  vi.spyOn(URL, "createObjectURL").mockReturnValue("blob:old");
  const revoke = vi.spyOn(URL, "revokeObjectURL");
  const view = render(<PersistentPlayer />);
  await waitFor(() =>
    expect(document.querySelector("img.player-cover")?.getAttribute("src")).toBe("blob:old"),
  );
  playback.track = { ...baseTrack, artworkId: null, title: "No cover" };
  view.rerender(<PersistentPlayer />);
  expect(document.querySelector("img.player-cover")).toBeNull();
  expect(document.querySelector(".player-cover-placeholder")?.textContent).toBe("N");
  expect(revoke).toHaveBeenCalledWith("blob:old");

  let resolveBytes: ((bytes: ArrayBuffer) => void) | undefined;
  api.getArtwork.mockReturnValue(
    new Promise<ArrayBuffer>((resolve) => {
      resolveBytes = resolve;
    }),
  );
  playback.track = { ...baseTrack, artworkId: 2, title: "Late" };
  view.rerender(<PersistentPlayer />);
  playback.track = { ...baseTrack, artworkId: null, title: "Current" };
  view.rerender(<PersistentPlayer />);
  resolveBytes?.(new ArrayBuffer(4));
  await Promise.resolve();
  await Promise.resolve();
  expect(document.querySelector("img.player-cover")).toBeNull();
});

test("cover rotation, layout order, and collapsed focus contract follow playback state", async () => {
  vi.spyOn(URL, "createObjectURL").mockReturnValue("blob:cover");
  playback.playing = true;
  const user = userEvent.setup();
  const view = render(<PersistentPlayer />);
  await waitFor(() =>
    expect(document.querySelector(".player-cover")?.classList.contains("is-playing")).toBe(true),
  );
  const cover = document.querySelector("img.player-cover") as HTMLImageElement;
  expect(cover.style.animationPlayState).toBe("running");
  const progress = document.querySelector(".player-progress")!;
  expect(progress.firstElementChild?.classList.contains("play-button")).toBe(true);
  expect(progress.querySelector('input[type="range"]')).toBeTruthy();
  await user.click(screen.getByRole("button", { name: "Collapse player" }));
  const player = document.querySelector(".persistent-player")!;
  expect(player.getAttribute("data-collapsed")).toBe("true");
  expect(document.querySelector(".player-expanded")?.getAttribute("aria-hidden")).toBe("true");
  expect(
    document.querySelector(".player-expanded")?.classList.contains("is-collapsed-content"),
  ).toBe(true);
  expect((document.querySelector(".collapsed-play") as HTMLButtonElement).disabled).toBe(false);
  await user.click(screen.getByRole("button", { name: "Expand player" }));
  expect((document.querySelector(".collapsed-play") as HTMLButtonElement).disabled).toBe(true);
  expect(
    document.querySelector(".player-expanded")?.classList.contains("is-collapsed-content"),
  ).toBe(false);
  playback.playing = false;
  view.rerender(<PersistentPlayer />);
  expect(document.querySelector(".player-cover")?.classList.contains("is-playing")).toBe(false);
  expect(
    (document.querySelector("img.player-cover") as HTMLImageElement).style.animationPlayState,
  ).toBe("paused");
  expect(
    (document.querySelector("img.player-cover") as HTMLImageElement).classList.contains(
      "player-cover",
    ),
  ).toBe(true);
});
