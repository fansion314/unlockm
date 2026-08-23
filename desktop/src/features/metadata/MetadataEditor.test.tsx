import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vite-plus/test";
import "../../i18n";
import { MetadataEditor } from "./MetadataEditor";

const api = vi.hoisted(() => ({
  listArtworkCandidates: vi.fn(),
  updateRunItemMetadata: vi.fn(),
  getArtwork: vi.fn(),
  getArtworkMime: vi.fn(),
}));
const playback = vi.hoisted(() => ({
  activeItemId: 7,
  releaseForItem: vi.fn(),
  restoreReleased: vi.fn(),
}));
vi.mock("../../lib/ipc", () => ({ desktopApi: api }));
vi.mock("../playback/PlaybackProvider", () => ({ usePlayback: () => playback }));

const item = {
  id: 7,
  runId: 1,
  ordinal: 0,
  itemKind: "music" as const,
  sourcePath: "a",
  relativePath: "a.mp3",
  destinationPath: "a.mp3",
  action: "copy",
  outcome: "copied",
  logicalBytes: 1,
  outputBytes: 1,
  outputFormat: "mp3",
  errorCode: null,
  errorDetail: null,
  warnings: [],
  title: "Title",
  artist: "Artist",
  album: "Album",
  durationMs: null,
  artworkId: null,
};

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

test("edits metadata, chooses a local artwork and supports Escape", async () => {
  api.listArtworkCandidates.mockResolvedValue([
    { artworkId: 44, album: "Album", artist: null, title: null },
  ]);
  api.getArtworkMime.mockResolvedValue({ mimeType: "image/png" });
  api.getArtwork.mockResolvedValue(new ArrayBuffer(4));
  api.updateRunItemMetadata.mockResolvedValue(item);
  playback.releaseForItem.mockReturnValue({ track: item, wasPlaying: true });
  playback.restoreReleased.mockResolvedValue(undefined);
  const close = vi.fn();
  const saved = vi.fn();
  render(<MetadataEditor item={item} onClose={close} onSaved={saved} />);
  expect(screen.getByRole("button", { name: "Cancel" })).toBeTruthy();
  expect(await screen.findByRole("button", { name: "Use local cover 44" })).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Use local cover 44" }));
  fireEvent.click(screen.getByRole("button", { name: "Save" }));
  await waitFor(() =>
    expect(api.updateRunItemMetadata).toHaveBeenCalledWith(
      7,
      expect.objectContaining({ existingArtworkId: 44 }),
    ),
  );
  fireEvent.keyDown(window, { key: "Escape" });
  expect(close).toHaveBeenCalled();
});

test("releases before metadata IPC and restores updated or original track", async () => {
  api.listArtworkCandidates.mockResolvedValue([]);
  api.updateRunItemMetadata.mockResolvedValue(item);
  playback.releaseForItem.mockReturnValue({ track: item, wasPlaying: true });
  render(<MetadataEditor item={item} onClose={vi.fn()} onSaved={vi.fn()} />);
  await waitFor(() => expect(api.listArtworkCandidates).toHaveBeenCalled());
  fireEvent.click(screen.getByRole("button", { name: "Save" }));
  await waitFor(() => expect(api.updateRunItemMetadata).toHaveBeenCalled());
  expect(playback.releaseForItem.mock.invocationCallOrder[0]).toBeLessThan(
    api.updateRunItemMetadata.mock.invocationCallOrder[0],
  );
  expect(playback.restoreReleased).toHaveBeenCalledWith(expect.anything(), item);
});

test("restores the original track when metadata IPC fails", async () => {
  api.listArtworkCandidates.mockResolvedValue([]);
  api.updateRunItemMetadata.mockRejectedValue(new Error("locked"));
  playback.releaseForItem.mockReturnValue({ track: item, wasPlaying: true });
  render(<MetadataEditor item={item} onClose={vi.fn()} onSaved={vi.fn()} />);
  fireEvent.click(screen.getByRole("button", { name: "Save" }));
  expect(await screen.findByRole("alert")).toBeTruthy();
  expect(playback.restoreReleased).toHaveBeenCalledWith(expect.anything(), item);
});

test("uploaded cover immediately propagates the returned artwork id", async () => {
  const updated = { ...item, artworkId: 99, title: "Updated" };
  api.listArtworkCandidates.mockResolvedValue([]);
  api.updateRunItemMetadata.mockResolvedValue(updated);
  playback.releaseForItem.mockReturnValue({ track: item, wasPlaying: true });
  const saved = vi.fn();
  render(<MetadataEditor item={item} onClose={vi.fn()} onSaved={saved} />);
  const file = new File([new Uint8Array([137, 80, 78, 71, 13, 10, 26, 10])], "cover.png", {
    type: "image/png",
  });
  fireEvent.change(screen.getByLabelText("Upload cover"), { target: { files: [file] } });
  fireEvent.click(screen.getByRole("button", { name: "Save" }));
  await waitFor(() => expect(saved).toHaveBeenCalledWith(updated));
  expect(playback.restoreReleased).toHaveBeenCalledWith(expect.anything(), updated);
});
