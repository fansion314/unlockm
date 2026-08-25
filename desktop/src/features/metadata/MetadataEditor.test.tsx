/// <reference types="node" />

import { readFileSync } from "node:fs";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vite-plus/test";
import "../../i18n";
import "../../styles/history.css";
import { applyTheme } from "../../lib/theme";
import { MetadataEditor, OnlineCover } from "./MetadataEditor";

const historyCssSource = readFileSync("src/styles/history.css", "utf8");

const api = vi.hoisted(() => ({
  listArtworkCandidates: vi.fn(),
  updateRunItemMetadata: vi.fn(),
  getArtwork: vi.fn(),
  getArtworkMime: vi.fn(),
  onlineMetadataStatus: vi.fn(),
  onlineMetadataProviderCapabilities: vi.fn(),
  startOnlineMetadataLogin: vi.fn(),
  cancelOnlineMetadataLogin: vi.fn(),
  disconnectOnlineMetadata: vi.fn(),
  searchOnlineMetadata: vi.fn(),
  getOnlineMetadataCoverPreview: vi.fn(),
  openOnlineMetadataResult: vi.fn(),
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

function onlineConnected() {
  api.onlineMetadataStatus.mockResolvedValue({
    provider: "spotify",
    state: "connected",
    retryAtMs: null,
  });
}

async function selectSpotify() {
  fireEvent.change(await screen.findByRole("combobox", { name: "Provider" }), {
    target: { value: "spotify" },
  });
}

test("online connection states expose safe actionable controls", async () => {
  api.listArtworkCandidates.mockResolvedValue([]);
  api.onlineMetadataStatus.mockResolvedValueOnce({
    provider: "spotify",
    state: "notConfigured",
    retryAtMs: null,
  });
  const { rerender } = render(<MetadataEditor item={item} onClose={vi.fn()} onSaved={vi.fn()} />);
  await selectSpotify();
  expect(await screen.findByText("Spotify search is not configured for this build.")).toBeTruthy();
  cleanup();
  api.onlineMetadataStatus.mockResolvedValueOnce({
    provider: "spotify",
    state: "credentialUnavailable",
    retryAtMs: null,
  });
  render(<MetadataEditor item={item} onClose={vi.fn()} onSaved={vi.fn()} />);
  await selectSpotify();
  expect(await screen.findByRole("alert")).toBeTruthy();
  cleanup();
  api.onlineMetadataStatus.mockResolvedValueOnce({
    provider: "spotify",
    state: "reauthorizationRequired",
    retryAtMs: null,
  });
  render(<MetadataEditor item={item} onClose={vi.fn()} onSaved={vi.fn()} />);
  await selectSpotify();
  expect(await screen.findByRole("button", { name: "Reconnect Spotify" })).toBeTruthy();
  void rerender;
});

test("opening the editor does not login or search", async () => {
  onlineConnected();
  api.listArtworkCandidates.mockResolvedValue([]);
  render(<MetadataEditor item={item} onClose={vi.fn()} onSaved={vi.fn()} />);
  await waitFor(() => expect(api.onlineMetadataStatus).toHaveBeenCalled());
  expect(api.startOnlineMetadataLogin).not.toHaveBeenCalled();
  expect(api.searchOnlineMetadata).not.toHaveBeenCalled();
});

test("MusicBrainz is the no-login default and artwork stays on demand", async () => {
  api.listArtworkCandidates.mockResolvedValue([]);
  api.onlineMetadataProviderCapabilities.mockResolvedValue([
    {
      provider: "musicBrainz",
      primaryOrder: 0,
      requiresAuthorization: false,
      artworkSource: "TheAudioDB",
      attribution: "MusicBrainz; artwork on demand from TheAudioDB",
    },
    {
      provider: "spotify",
      primaryOrder: 1,
      requiresAuthorization: true,
      artworkSource: "Spotify",
      attribution: "Spotify",
    },
  ]);
  api.searchOnlineMetadata.mockResolvedValue([
    {
      provider: "musicBrainz",
      itemId: "11111111-1111-1111-1111-111111111111",
      title: "Free result",
      artists: ["Artist"],
      album: "Album",
      durationMs: 1000,
      coverHandle: "opaque-free-cover",
    },
  ]);
  render(<MetadataEditor item={item} onClose={vi.fn()} onSaved={vi.fn()} />);
  await waitFor(() => expect(api.onlineMetadataProviderCapabilities).toHaveBeenCalled());
  expect((screen.getByRole("combobox", { name: "Provider" }) as HTMLSelectElement).value).toBe(
    "musicBrainz",
  );
  expect(await screen.findByText(/Free, no login/)).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Search MusicBrainz" }));
  await waitFor(() =>
    expect(api.searchOnlineMetadata).toHaveBeenCalledWith("musicBrainz", expect.any(String)),
  );
  expect(api.getOnlineMetadataCoverPreview).not.toHaveBeenCalled();
  fireEvent.click(await screen.findByRole("button", { name: "Use cover" }));
  await waitFor(() =>
    expect(api.getOnlineMetadataCoverPreview).toHaveBeenCalledWith(
      "musicBrainz",
      "opaque-free-cover",
    ),
  );
});

test("MusicBrainz quick-fill exposes clean artist names to the editable draft", async () => {
  api.listArtworkCandidates.mockResolvedValue([]);
  api.searchOnlineMetadata.mockResolvedValue([
    {
      provider: "musicBrainz",
      itemId: "11111111-1111-1111-1111-111111111111",
      title: "Free result",
      artists: ["A", "B"],
      album: "Album",
      durationMs: 1000,
      coverHandle: null,
    },
  ]);
  render(<MetadataEditor item={item} onClose={vi.fn()} onSaved={vi.fn()} />);
  fireEvent.click(await screen.findByRole("button", { name: "Search MusicBrainz" }));
  fireEvent.click(await screen.findByRole("button", { name: "Fill text" }));
  const inputs = screen.getAllByRole("textbox");
  expect(inputs.some((input) => (input as HTMLInputElement).value === "A, B")).toBe(true);
});

test("online candidates render as a counted list with labeled fields", async () => {
  api.listArtworkCandidates.mockResolvedValue([]);
  api.searchOnlineMetadata.mockResolvedValue([
    {
      provider: "musicBrainz",
      requiresAuthorization: false,
      artworkSource: "TheAudioDB",
      attribution: "MusicBrainz; artwork on demand from TheAudioDB",
      itemId: "11111111-1111-1111-1111-111111111111",
      title: "Labeled result",
      artists: ["First artist", "Second artist"],
      album: "Result album",
      durationMs: 262000,
      coverHandle: "opaque-cover",
    },
  ]);
  render(<MetadataEditor item={item} onClose={vi.fn()} onSaved={vi.fn()} />);
  fireEvent.click(await screen.findByRole("button", { name: "Search MusicBrainz" }));
  const result = await screen.findByRole("listitem", {
    name: "Online match for Labeled result",
  });
  expect(Array.from(result.querySelectorAll("dt"), (label) => label.textContent)).toEqual([
    "Title",
    "Artist",
    "Album",
    "Duration",
    "Metadata source",
    "Cover source",
  ]);
  expect(result.textContent).toContain("First artist, Second artist");
  expect(result.textContent).toContain("Result album");
  expect(result.textContent).toContain("4:22");
  expect(result.textContent).toContain("TheAudioDB");
  expect(screen.getByText("1 matches")).toBeTruthy();
});

test("edits metadata, chooses a local artwork and supports Escape", async () => {
  onlineConnected();
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
  expect(document.querySelector(".metadata-modal-body")).toBeTruthy();
  expect(document.querySelector(".metadata-actions")).toBeTruthy();
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

test("parent rerenders do not steal focus and Escape uses the latest close callback", () => {
  onlineConnected();
  api.listArtworkCandidates.mockResolvedValue([]);
  const firstClose = vi.fn();
  const latestClose = vi.fn();
  const { rerender } = render(
    <MetadataEditor item={item} onClose={firstClose} onSaved={vi.fn()} />,
  );
  const artistInput = screen.getByRole("textbox", { name: "Artist" });
  artistInput.focus();
  expect(document.activeElement).toBe(artistInput);

  rerender(<MetadataEditor item={item} onClose={latestClose} onSaved={vi.fn()} />);

  expect(document.activeElement).toBe(artistInput);
  fireEvent.keyDown(window, { key: "Escape" });
  expect(firstClose).not.toHaveBeenCalled();
  expect(latestClose).toHaveBeenCalledTimes(1);
});

test("modal owns a split workspace and keeps the actions outside it", () => {
  onlineConnected();
  api.listArtworkCandidates.mockResolvedValue([]);
  render(<MetadataEditor item={item} onClose={vi.fn()} onSaved={vi.fn()} />);
  const body = document.querySelector('[data-layout="split-workspace"]')!;
  const actions = document.querySelector('[data-layout="actions-row"]')!;
  const editorPane = document.querySelector(".metadata-editor-pane")!;
  const onlinePane = document.querySelector(".metadata-online")!;
  expect(body.classList.contains("metadata-modal-body")).toBe(true);
  expect(actions.classList.contains("metadata-actions")).toBe(true);
  expect(body.contains(actions)).toBe(false);
  expect(body.contains(editorPane)).toBe(true);
  expect(body.contains(onlinePane)).toBe(true);
  expect(document.querySelector(".metadata-modal")?.children).toContain(actions);
});

test("local cover grid contains intrinsic images and reserves two title lines", () => {
  expect(historyCssSource).toContain("grid-auto-rows: 150px");
  expect(historyCssSource).toContain(
    "grid-template-columns: repeat(auto-fill, minmax(112px, 1fr))",
  );
  expect(historyCssSource).toMatch(
    /\.metadata-candidate-grid button \{[^}]*grid-template-rows: 104px minmax\(0, 2\.6em\);[^}]*overflow: hidden;/s,
  );
  expect(historyCssSource).toMatch(
    /\.metadata-candidate-grid small \{[^}]*-webkit-line-clamp: 2;[^}]*overflow: hidden;/s,
  );
  expect(historyCssSource).toMatch(
    /@media \(max-width: 860px\)[\s\S]*?\.metadata-modal-body \{[^}]*display: block;[^}]*overflow: auto;/,
  );
});

test("switching themes preserves the open editor and unsaved draft", async () => {
  onlineConnected();
  api.listArtworkCandidates.mockResolvedValue([]);
  render(<MetadataEditor item={item} onClose={vi.fn()} onSaved={vi.fn()} />);
  const title = screen.getByRole("textbox", { name: "Title" });
  fireEvent.change(title, { target: { value: "Unsaved title" } });

  applyTheme("nord");

  expect(document.documentElement.dataset.theme).toBe("nord");
  expect(screen.getByRole("dialog", { name: "Edit music metadata" })).toBeTruthy();
  expect((screen.getByRole("textbox", { name: "Title" }) as HTMLInputElement).value).toBe(
    "Unsaved title",
  );
});

test("releases before metadata IPC and restores updated or original track", async () => {
  onlineConnected();
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
  onlineConnected();
  api.listArtworkCandidates.mockResolvedValue([]);
  api.updateRunItemMetadata.mockRejectedValue(new Error("locked"));
  playback.releaseForItem.mockReturnValue({ track: item, wasPlaying: true });
  render(<MetadataEditor item={item} onClose={vi.fn()} onSaved={vi.fn()} />);
  fireEvent.click(screen.getByRole("button", { name: "Save" }));
  expect(await screen.findByRole("alert")).toBeTruthy();
  expect(playback.restoreReleased).toHaveBeenCalledWith(expect.anything(), item);
});

test("uploaded cover immediately propagates the returned artwork id", async () => {
  onlineConnected();
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

test("online text apply only changes the draft until Save", async () => {
  onlineConnected();
  api.listArtworkCandidates.mockResolvedValue([]);
  api.searchOnlineMetadata.mockResolvedValue([
    {
      provider: "spotify",
      itemId: "0123456789ABCDEFGHIJKL",
      title: "Online title",
      artists: ["Online artist"],
      album: "Online album",
      durationMs: 123000,
      coverHandle: null,
    },
  ]);
  render(<MetadataEditor item={item} onClose={vi.fn()} onSaved={vi.fn()} />);
  await waitFor(() => expect(api.onlineMetadataStatus).toHaveBeenCalled());
  await selectSpotify();
  fireEvent.click(screen.getByRole("button", { name: "Search Spotify" }));
  expect(await screen.findByRole("button", { name: "Fill text" })).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Fill text" }));
  expect(api.updateRunItemMetadata).not.toHaveBeenCalled();
  expect(playback.releaseForItem).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Save" }));
  await waitFor(() =>
    expect(api.updateRunItemMetadata).toHaveBeenCalledWith(
      7,
      expect.objectContaining({
        title: "Online title",
        artist: "Online artist",
        album: "Online album",
      }),
    ),
  );
});

test("online cover apply remains a draft change until Save", async () => {
  const create = vi.spyOn(URL, "createObjectURL").mockReturnValue("blob:online-cover-draft");
  const revoke = vi.spyOn(URL, "revokeObjectURL");
  api.listArtworkCandidates.mockResolvedValue([]);
  api.searchOnlineMetadata.mockResolvedValue([
    {
      provider: "musicBrainz",
      requiresAuthorization: false,
      artworkSource: "TheAudioDB",
      attribution: "MusicBrainz; artwork on demand from TheAudioDB",
      itemId: "11111111-1111-1111-1111-111111111111",
      title: "Online title",
      artists: ["Online artist"],
      album: "Online album",
      durationMs: 123000,
      coverHandle: "opaque-cover",
    },
  ]);
  api.getOnlineMetadataCoverPreview.mockResolvedValue(new ArrayBuffer(8));
  api.updateRunItemMetadata.mockResolvedValue(item);
  playback.releaseForItem.mockReturnValue({ track: item, wasPlaying: true });
  const view = render(<MetadataEditor item={item} onClose={vi.fn()} onSaved={vi.fn()} />);
  await waitFor(() => expect(api.onlineMetadataStatus).toHaveBeenCalled());
  fireEvent.click(screen.getByRole("button", { name: "Search MusicBrainz" }));
  expect(await screen.findByRole("button", { name: "Use cover" })).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Use cover" }));
  await waitFor(() => expect(api.getOnlineMetadataCoverPreview).toHaveBeenCalled());
  expect(document.querySelector(".metadata-cover-preview")?.getAttribute("src")).toBe(
    "blob:online-cover-draft",
  );
  expect(
    screen.getAllByRole("status").some((status) => status.textContent?.includes("TheAudioDB")),
  ).toBe(true);
  expect(api.updateRunItemMetadata).not.toHaveBeenCalled();
  expect(playback.releaseForItem).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Save" }));
  await waitFor(() =>
    expect(api.updateRunItemMetadata).toHaveBeenCalledWith(
      7,
      expect.objectContaining({ cover: expect.any(Uint8Array) }),
    ),
  );
  view.unmount();
  expect(revoke).toHaveBeenCalledWith("blob:online-cover-draft");
  create.mockRestore();
  revoke.mockRestore();
});

test("online cover failure is visible above the result list and preserves the draft", async () => {
  api.listArtworkCandidates.mockResolvedValue([]);
  api.searchOnlineMetadata.mockResolvedValue([
    {
      provider: "musicBrainz",
      requiresAuthorization: false,
      artworkSource: "TheAudioDB",
      attribution: "MusicBrainz; artwork on demand from TheAudioDB",
      itemId: "11111111-1111-1111-1111-111111111111",
      title: "No cover result",
      artists: ["Artist"],
      album: "Album",
      durationMs: 1000,
      coverHandle: "missing-cover",
    },
  ]);
  api.getOnlineMetadataCoverPreview.mockRejectedValue({
    code: "provider_unavailable",
    detail: "private",
  });
  render(<MetadataEditor item={item} onClose={vi.fn()} onSaved={vi.fn()} />);
  fireEvent.click(await screen.findByRole("button", { name: "Search MusicBrainz" }));
  fireEvent.click(await screen.findByRole("button", { name: "Use cover" }));
  const alerts = await screen.findAllByRole("alert");
  expect(
    alerts.some((alert) => alert.textContent?.includes("The cover could not be fetched")),
  ).toBe(true);
  expect(document.querySelector(".metadata-cover-preview")).toBeNull();
  expect(api.updateRunItemMetadata).not.toHaveBeenCalled();
});

test("missing online artwork is distinct from a network failure", async () => {
  api.listArtworkCandidates.mockResolvedValue([]);
  api.searchOnlineMetadata.mockResolvedValue([
    {
      provider: "musicBrainz",
      requiresAuthorization: false,
      artworkSource: "Cover Art Archive / TheAudioDB",
      attribution: "MusicBrainz",
      itemId: "11111111-1111-1111-1111-111111111111",
      title: "Missing cover result",
      artists: ["Artist"],
      album: "Album",
      durationMs: 1000,
      coverHandle: "missing-cover",
    },
  ]);
  api.getOnlineMetadataCoverPreview.mockRejectedValue({
    code: "online_artwork_not_found",
    detail: "private",
  });
  render(<MetadataEditor item={item} onClose={vi.fn()} onSaved={vi.fn()} />);
  fireEvent.click(await screen.findByRole("button", { name: "Search MusicBrainz" }));
  fireEvent.click(await screen.findByRole("button", { name: "Use cover" }));
  const alerts = await screen.findAllByRole("alert");
  expect(alerts.some((alert) => alert.textContent?.includes("No cover is available"))).toBe(true);
  expect(
    alerts.some((alert) => alert.textContent?.includes("could not be fetched or validated")),
  ).toBe(false);
});

test("a stale cover response cannot alter the next provider draft or busy state", async () => {
  onlineConnected();
  api.listArtworkCandidates.mockResolvedValue([]);
  api.searchOnlineMetadata.mockResolvedValue([
    {
      provider: "spotify",
      itemId: "0123456789ABCDEFGHIJKL",
      title: "Online title",
      artists: ["Online artist"],
      album: "Online album",
      durationMs: 123000,
      coverHandle: "opaque-cover",
    },
  ]);
  let resolveCover: (value: ArrayBuffer) => void = () => undefined;
  api.getOnlineMetadataCoverPreview.mockReturnValue(
    new Promise((resolve) => {
      resolveCover = resolve;
    }),
  );
  render(<MetadataEditor item={item} onClose={vi.fn()} onSaved={vi.fn()} />);
  await selectSpotify();
  fireEvent.click(screen.getByRole("button", { name: "Search Spotify" }));
  fireEvent.click(await screen.findByRole("button", { name: "Use cover" }));
  fireEvent.change(screen.getByRole("combobox", { name: "Provider" }), {
    target: { value: "musicBrainz" },
  });
  expect(
    (screen.getByRole("button", { name: "Search MusicBrainz" }) as HTMLButtonElement).disabled,
  ).toBe(false);
  resolveCover(new ArrayBuffer(8));
  await Promise.resolve();
  expect(screen.queryByRole("status")).toBeNull();
  expect(screen.queryByRole("alert")).toBeNull();
});

test("Connect immediately shows a cancellable authorizing state", async () => {
  api.listArtworkCandidates.mockResolvedValue([]);
  api.onlineMetadataStatus.mockResolvedValue({
    provider: "spotify",
    state: "disconnected",
    retryAtMs: null,
  });
  api.startOnlineMetadataLogin.mockReturnValue(new Promise(() => undefined));
  api.cancelOnlineMetadataLogin.mockResolvedValue(true);
  render(<MetadataEditor item={item} onClose={vi.fn()} onSaved={vi.fn()} />);
  await selectSpotify();
  fireEvent.click(await screen.findByRole("button", { name: "Connect Spotify" }));
  fireEvent.click((await screen.findAllByRole("button", { name: "Cancel" }))[0]);
  await waitFor(() => expect(api.cancelOnlineMetadataLogin).toHaveBeenCalledWith("spotify"));
});

test("Cancel makes a deferred login reconnectable", async () => {
  api.listArtworkCandidates.mockResolvedValue([]);
  api.onlineMetadataStatus.mockResolvedValue({
    provider: "spotify",
    state: "disconnected",
    retryAtMs: null,
  });
  let settle: (value: {
    provider: "spotify";
    state: "disconnected";
    retryAtMs: null;
  }) => void = () => undefined;
  api.startOnlineMetadataLogin.mockReturnValue(
    new Promise((resolve) => {
      settle = resolve;
    }),
  );
  api.cancelOnlineMetadataLogin.mockResolvedValue(true);
  render(<MetadataEditor item={item} onClose={vi.fn()} onSaved={vi.fn()} />);
  await selectSpotify();
  fireEvent.click(await screen.findByRole("button", { name: "Connect Spotify" }));
  fireEvent.click((await screen.findAllByRole("button", { name: "Cancel" }))[0]);
  await waitFor(() => expect(api.cancelOnlineMetadataLogin).toHaveBeenCalledWith("spotify"));
  settle({ provider: "spotify", state: "disconnected", retryAtMs: null });
  expect(await screen.findByRole("button", { name: "Connect Spotify" })).toBeTruthy();
});

test("Fill all and Open in Spotify do not save or release playback", async () => {
  onlineConnected();
  api.listArtworkCandidates.mockResolvedValue([]);
  api.searchOnlineMetadata.mockResolvedValue([
    {
      provider: "spotify",
      itemId: "0123456789ABCDEFGHIJKL",
      title: "Online",
      artists: ["Artist"],
      album: "Album",
      durationMs: 1000,
      coverHandle: null,
    },
  ]);
  render(<MetadataEditor item={item} onClose={vi.fn()} onSaved={vi.fn()} />);
  await selectSpotify();
  fireEvent.click(await screen.findByRole("button", { name: "Search Spotify" }));
  expect(await screen.findByRole("listitem", { name: "Online match for Online" })).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Open in Spotify" }));
  await waitFor(() =>
    expect(api.openOnlineMetadataResult).toHaveBeenCalledWith("spotify", "0123456789ABCDEFGHIJKL"),
  );
  fireEvent.click(screen.getByRole("button", { name: "Fill all" }));
  expect(api.updateRunItemMetadata).not.toHaveBeenCalled();
  expect(playback.releaseForItem).not.toHaveBeenCalled();
});

test("Escape closes the dialog with online controls present", async () => {
  onlineConnected();
  api.listArtworkCandidates.mockResolvedValue([]);
  const close = vi.fn();
  render(<MetadataEditor item={item} onClose={close} onSaved={vi.fn()} />);
  await selectSpotify();
  await screen.findByRole("button", { name: "Search Spotify" });
  fireEvent.keyDown(window, { key: "Escape" });
  expect(close).toHaveBeenCalledTimes(1);
});

test("Disconnect ignores a delayed search response", async () => {
  onlineConnected();
  api.listArtworkCandidates.mockResolvedValue([]);
  let resolveSearch: (value: any[]) => void = () => undefined;
  api.searchOnlineMetadata.mockReturnValue(
    new Promise((resolve) => {
      resolveSearch = resolve;
    }),
  );
  api.disconnectOnlineMetadata.mockResolvedValue({
    provider: "spotify",
    state: "disconnected",
    retryAtMs: null,
  });
  render(<MetadataEditor item={item} onClose={vi.fn()} onSaved={vi.fn()} />);
  await selectSpotify();
  fireEvent.click(await screen.findByRole("button", { name: "Search Spotify" }));
  fireEvent.click(screen.getByRole("button", { name: "Disconnect Spotify" }));
  await waitFor(() => expect(api.disconnectOnlineMetadata).toHaveBeenCalledWith("spotify"));
  resolveSearch([
    {
      provider: "spotify",
      itemId: "0123456789ABCDEFGHIJKL",
      title: "Late result",
      artists: ["A"],
      album: "B",
      durationMs: 1,
      coverHandle: null,
    },
  ]);
  await waitFor(() => expect(screen.queryByText("Late result")).toBeNull());
  expect(await screen.findByRole("button", { name: "Connect Spotify" })).toBeTruthy();
});

test("provider switch ignores a delayed result from the prior provider", async () => {
  onlineConnected();
  api.listArtworkCandidates.mockResolvedValue([]);
  let resolveSearch: (value: any[]) => void = () => undefined;
  api.searchOnlineMetadata.mockReturnValue(
    new Promise((resolve) => {
      resolveSearch = resolve;
    }),
  );
  render(<MetadataEditor item={item} onClose={vi.fn()} onSaved={vi.fn()} />);
  await selectSpotify();
  fireEvent.click(await screen.findByRole("button", { name: "Search Spotify" }));
  fireEvent.change(screen.getByRole("combobox", { name: "Provider" }), {
    target: { value: "musicBrainz" },
  });
  expect(
    ((await screen.findByRole("button", { name: "Search MusicBrainz" })) as HTMLButtonElement)
      .disabled,
  ).toBe(false);
  resolveSearch([
    {
      provider: "spotify",
      itemId: "0123456789ABCDEFGHIJKL",
      title: "Late Spotify result",
      artists: ["A"],
      album: "B",
      durationMs: 1,
      coverHandle: null,
    },
  ]);
  await waitFor(() => expect(screen.queryByText("Late Spotify result")).toBeNull());
});

test("retryAt cooldown disables Search", async () => {
  api.listArtworkCandidates.mockResolvedValue([]);
  api.onlineMetadataStatus.mockResolvedValue({
    provider: "spotify",
    state: "connected",
    retryAtMs: Date.now() + 30_000,
  });
  render(<MetadataEditor item={item} onClose={vi.fn()} onSaved={vi.fn()} />);
  expect(
    ((await screen.findByRole("button", { name: "Search MusicBrainz" })) as HTMLButtonElement)
      .disabled,
  ).toBe(false);
  await selectSpotify();
  expect(
    (await screen.findByRole("button", { name: "Search Spotify" })).hasAttribute("disabled"),
  ).toBe(true);
});

test("MusicBrainz cooldown is visible and does not disable Spotify after switching", async () => {
  api.listArtworkCandidates.mockResolvedValue([]);
  api.onlineMetadataStatus
    .mockResolvedValueOnce({
      provider: "musicBrainz",
      state: "connected",
      retryAtMs: Date.now() + 30_000,
    })
    .mockResolvedValueOnce({ provider: "spotify", state: "connected", retryAtMs: null });
  render(<MetadataEditor item={item} onClose={vi.fn()} onSaved={vi.fn()} />);
  expect(
    ((await screen.findByRole("button", { name: "Search MusicBrainz" })) as HTMLButtonElement)
      .disabled,
  ).toBe(true);
  expect((await screen.findByRole("status")).textContent).toContain("MusicBrainz search");
  await selectSpotify();
  expect(
    ((await screen.findByRole("button", { name: "Search Spotify" })) as HTMLButtonElement).disabled,
  ).toBe(false);
});

test("MusicBrainz empty and provider errors never use Spotify wording", async () => {
  api.listArtworkCandidates.mockResolvedValue([]);
  api.searchOnlineMetadata.mockResolvedValueOnce([]).mockRejectedValueOnce({
    code: "provider_unavailable",
    detail: "private",
  });
  render(<MetadataEditor item={item} onClose={vi.fn()} onSaved={vi.fn()} />);
  fireEvent.click(await screen.findByRole("button", { name: "Search MusicBrainz" }));
  expect(await screen.findByText("No MusicBrainz recordings found.")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Search MusicBrainz" }));
  expect((await screen.findByRole("alert")).textContent).toContain(
    "MusicBrainz is temporarily unavailable.",
  );
});

test("typed provider failure is rendered as a safe alert", async () => {
  onlineConnected();
  api.listArtworkCandidates.mockResolvedValue([]);
  api.searchOnlineMetadata.mockRejectedValue({ code: "provider_unavailable", detail: "private" });
  render(<MetadataEditor item={item} onClose={vi.fn()} onSaved={vi.fn()} />);
  await selectSpotify();
  fireEvent.click(await screen.findByRole("button", { name: "Search Spotify" }));
  expect((await screen.findByRole("alert")).textContent).toContain(
    "Spotify is temporarily unavailable.",
  );
});

test("OnlineCover revokes an object URL on handle change and unmount", async () => {
  const create = vi.spyOn(URL, "createObjectURL").mockReturnValue("blob:test");
  const revoke = vi.spyOn(URL, "revokeObjectURL");
  api.getOnlineMetadataCoverPreview.mockResolvedValue(new ArrayBuffer(1));
  const candidate = {
    provider: "spotify" as const,
    requiresAuthorization: true,
    artworkSource: "Spotify",
    attribution: "Spotify",
    itemId: "0123456789ABCDEFGHIJKL",
    title: "t",
    artists: ["a"],
    album: "a",
    durationMs: 1,
    coverHandle: "one",
  };
  const view = render(<OnlineCover candidate={candidate} />);
  await waitFor(() =>
    expect(view.container.querySelector("img")?.getAttribute("src")).toBe("blob:test"),
  );
  view.rerender(<OnlineCover candidate={{ ...candidate, coverHandle: "two" }} />);
  expect(revoke).toHaveBeenCalledWith("blob:test");
  view.unmount();
  create.mockRestore();
  revoke.mockRestore();
});

test("Spotify result lazily previews only its opaque handle and revokes on provider switch", async () => {
  const create = vi.spyOn(URL, "createObjectURL").mockReturnValue("blob:spotify-cover");
  const revoke = vi.spyOn(URL, "revokeObjectURL");
  onlineConnected();
  api.listArtworkCandidates.mockResolvedValue([]);
  api.searchOnlineMetadata.mockResolvedValue([
    {
      provider: "spotify",
      itemId: "0123456789ABCDEFGHIJKL",
      title: "Spotify result",
      artists: ["Artist"],
      album: "Album",
      durationMs: 1,
      coverHandle: "opaque-spotify-cover",
    },
  ]);
  api.getOnlineMetadataCoverPreview.mockResolvedValue(new ArrayBuffer(8));
  render(<MetadataEditor item={item} onClose={vi.fn()} onSaved={vi.fn()} />);
  await selectSpotify();
  fireEvent.click(await screen.findByRole("button", { name: "Search Spotify" }));
  await waitFor(() =>
    expect(api.getOnlineMetadataCoverPreview).toHaveBeenCalledWith(
      "spotify",
      "opaque-spotify-cover",
    ),
  );
  expect(api.getOnlineMetadataCoverPreview.mock.calls[0]).toEqual([
    "spotify",
    "opaque-spotify-cover",
  ]);
  expect(document.querySelector(".metadata-online-cover")?.getAttribute("src")).toBe(
    "blob:spotify-cover",
  );
  fireEvent.change(screen.getByRole("combobox", { name: "Provider" }), {
    target: { value: "musicBrainz" },
  });
  await waitFor(() => expect(revoke).toHaveBeenCalledWith("blob:spotify-cover"));
  create.mockRestore();
  revoke.mockRestore();
});

test("OnlineCover ignores a deferred obsolete handle", async () => {
  const create = vi.spyOn(URL, "createObjectURL").mockReturnValue("blob:current");
  let oldResolve: (value: ArrayBuffer) => void = () => undefined;
  let currentResolve: (value: ArrayBuffer) => void = () => undefined;
  api.getOnlineMetadataCoverPreview.mockImplementation(
    (_provider: string, handle: string) =>
      new Promise((resolve) => {
        if (handle === "old") oldResolve = resolve;
        else currentResolve = resolve;
      }),
  );
  const base = {
    provider: "spotify" as const,
    requiresAuthorization: true,
    artworkSource: "Spotify",
    attribution: "Spotify",
    itemId: "0123456789ABCDEFGHIJKL",
    title: "t",
    artists: ["a"],
    album: "a",
    durationMs: 1,
  };
  const view = render(<OnlineCover candidate={{ ...base, coverHandle: "old" }} />);
  view.rerender(<OnlineCover candidate={{ ...base, coverHandle: "current" }} />);
  await waitFor(() => expect(api.getOnlineMetadataCoverPreview).toHaveBeenCalledTimes(2));
  oldResolve(new ArrayBuffer(1));
  await Promise.resolve();
  expect(create).not.toHaveBeenCalled();
  currentResolve(new ArrayBuffer(1));
  await waitFor(() =>
    expect(view.container.querySelector("img")?.getAttribute("src")).toBe("blob:current"),
  );
  create.mockRestore();
});

test.each([
  ["reauthorization_required", "Reconnect Spotify to continue.", "reauthorizationRequired", null],
  ["rate_limited", "Spotify search is temporarily rate limited.", "connected", Date.now() + 60_000],
  ["provider_unavailable", "Spotify is temporarily unavailable.", "connected", null],
  ["request_timeout", "Spotify did not respond in time.", "connected", null],
])("typed %s refreshes online status", async (code, message, state, retryAtMs) => {
  onlineConnected();
  api.listArtworkCandidates.mockResolvedValue([]);
  api.onlineMetadataStatus
    .mockResolvedValueOnce({ provider: "musicBrainz", state: "connected", retryAtMs: null })
    .mockResolvedValueOnce({ provider: "spotify", state: "connected", retryAtMs: null })
    .mockResolvedValueOnce({ provider: "spotify", state, retryAtMs });
  api.searchOnlineMetadata.mockRejectedValue({ code, detail: "private" });
  render(<MetadataEditor item={item} onClose={vi.fn()} onSaved={vi.fn()} />);
  await selectSpotify();
  fireEvent.click(await screen.findByRole("button", { name: "Search Spotify" }));
  expect((await screen.findByRole("alert")).textContent).toContain(message);
  if (code === "rate_limited")
    expect(
      (await screen.findByRole("button", { name: "Search Spotify" })).hasAttribute("disabled"),
    ).toBe(true);
});

test("connected online controls stay within the dialog focus contract", async () => {
  onlineConnected();
  api.listArtworkCandidates.mockResolvedValue([]);
  render(<MetadataEditor item={item} onClose={vi.fn()} onSaved={vi.fn()} />);
  await selectSpotify();
  await screen.findByRole("button", { name: "Search Spotify" });
  expect(document.querySelector(".metadata-online")).toBeTruthy();
  const save = screen.getByRole("button", { name: "Save" });
  save.focus();
  fireEvent.keyDown(window, { key: "Tab" });
  expect(document.activeElement).toBe(document.querySelector("input"));
});
