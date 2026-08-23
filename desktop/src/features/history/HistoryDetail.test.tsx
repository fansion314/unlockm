import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, expect, test, vi } from "vite-plus/test";
import i18n from "../../i18n";
import type { HistoryItem } from "../../lib/contracts";
import { Artwork, HistoryDetail, VisibleArtwork } from "./HistoryDetail";
import { PlaybackProvider } from "../playback/PlaybackProvider";

const { api, convertFileSrc } = vi.hoisted(() => ({
  api: {
    getArtwork: vi.fn(),
    getArtworkMime: vi.fn(),
    queryRunItems: vi.fn(),
    listRunItemFacets: vi.fn(),
    revealRunOutput: vi.fn(),
    revealRunItem: vi.fn(),
    openRunItem: vi.fn(),
  },
  convertFileSrc: vi.fn((path: string, protocol: string) => `${protocol}://localhost/${path}`),
}));

vi.mock("../../lib/ipc", () => ({ desktopApi: api }));
vi.mock("@tauri-apps/api/core", () => ({ convertFileSrc }));

const run = {
  id: 9,
  inputKind: "directory" as const,
  inputPath: "/input/album",
  outputPath: "/output",
  settings: { updateMetadata: false, copyPassthrough: true, overwrite: false },
  status: "completed" as const,
  createdAtMs: 0,
  updatedAtMs: 65_000,
  finishedAtMs: 65_000,
  totalItems: 4,
  completedItems: 4,
  converted: 4,
  copied: 0,
  skipped: 0,
  conflicted: 0,
  failed: 0,
  musicCount: 4,
  representativeArtworkId: null,
  terminalErrorCode: null,
  terminalErrorDetail: null,
};

let pauseSpy: ReturnType<typeof vi.spyOn>;
let loadSpy: ReturnType<typeof vi.spyOn>;

function item(id: number, title = `Track ${id}`): HistoryItem {
  return {
    id,
    runId: 9,
    ordinal: id - 1,
    itemKind: "music" as const,
    sourcePath: `/input/${id}.ncm`,
    relativePath: `${id}.ncm`,
    destinationPath: `/output/${id}.mp3`,
    action: "convert",
    outcome: "converted",
    logicalBytes: 10,
    outputBytes: 1_572_864,
    outputFormat: "mp3",
    errorCode: null,
    errorDetail: null,
    warnings: [],
    title,
    artist: id % 2 ? "Artist A" : "Artist B",
    album: id % 2 ? "Album A" : "Album B",
    durationMs: 65_000,
    artworkId: null,
  };
}

function page(items = [item(1)], overrides = {}) {
  return {
    items,
    page: 1,
    pageSize: 25,
    pageCount: 1,
    filteredTotal: items.length,
    companionCount: 2,
    musicTotal: 4,
    ...overrides,
  };
}

function renderDetail(onError = vi.fn(), onRename = vi.fn(async () => undefined)) {
  return render(
    <PlaybackProvider>
      <HistoryDetail
        layoutId="run-card-9"
        run={run}
        onBack={() => undefined}
        onRename={onRename}
        onError={onError}
      />
    </PlaybackProvider>,
  );
}

beforeEach(() => {
  api.queryRunItems.mockResolvedValue(page());
  api.listRunItemFacets.mockImplementation(async (_runId, facet) => ({
    values: facet === "artist" ? ["Artist A", "Artist B"] : ["Album A", "Album B"],
    nextCursor: null,
  }));
  api.revealRunOutput.mockResolvedValue(true);
  api.revealRunItem.mockResolvedValue(true);
  api.openRunItem.mockResolvedValue(true);
  vi.spyOn(HTMLMediaElement.prototype, "play").mockResolvedValue(undefined);
  pauseSpy = vi.spyOn(HTMLMediaElement.prototype, "pause").mockImplementation(() => undefined);
  loadSpy = vi.spyOn(HTMLMediaElement.prototype, "load").mockImplementation(() => undefined);
});

afterEach(async () => {
  cleanup();
  vi.restoreAllMocks();
  for (const method of Object.values(api)) method.mockReset();
  convertFileSrc.mockClear();
  vi.unstubAllGlobals();
  await i18n.changeLanguage("en-US");
});

test("late artwork resolution after unmount revokes its object URL", async () => {
  let resolveBytes: ((bytes: ArrayBuffer) => void) | undefined;
  api.getArtworkMime.mockResolvedValue({ id: 1, mimeType: "image/png" });
  api.getArtwork.mockReturnValue(
    new Promise<ArrayBuffer>((resolve) => {
      resolveBytes = resolve;
    }),
  );
  const create = vi.spyOn(URL, "createObjectURL").mockReturnValue("blob:cover");
  const revoke = vi.spyOn(URL, "revokeObjectURL");
  render(<Artwork artworkId={1} title="A" />);
  cleanup();
  resolveBytes?.(new ArrayBuffer(4));
  await Promise.resolve();
  await Promise.resolve();
  expect(create).toHaveBeenCalled();
  expect(revoke).toHaveBeenCalledWith("blob:cover");
});

test("detail renders compact output facts without raw diagnostics", async () => {
  const failed = {
    ...item(1, "Broken track"),
    destinationPath: null,
    outcome: "failed",
    outputBytes: null,
    outputFormat: null,
    errorDetail: "private diagnostic path",
  };
  api.queryRunItems.mockResolvedValue(page([failed]));
  renderDetail();

  expect(await screen.findByText("Broken track")).toBeTruthy();
  expect(screen.getByText("Unknown format")).toBeTruthy();
  expect(document.querySelector(".outcome-failed")?.textContent).toBe("Failed");
  expect(screen.queryByText("private diagnostic path")).toBeNull();
  expect(document.querySelector(".music-item details")).toBeNull();
});

test("detail renames inline, cancels with Escape, and keeps editing after a failure", async () => {
  const user = userEvent.setup();
  const rename = vi.fn(async () => undefined);
  renderDetail(vi.fn(), rename);
  await screen.findByText("Track 1");
  await user.click(screen.getByRole("button", { name: "Rename conversion" }));
  const input = screen.getByRole("textbox", { name: "Rename conversion" }) as HTMLInputElement;
  expect(input.value).toBe("album");
  expect(screen.getByRole("button", { name: "Cancel" })).toBeTruthy();
  expect(screen.queryByRole("button", { name: "Rename conversion" })).toBeNull();
  await user.clear(input);
  await user.type(input, "Downloads");
  await user.keyboard("{Enter}");
  await waitFor(() => expect(rename).toHaveBeenCalledWith(9, "Downloads"));

  await user.click(screen.getByRole("button", { name: "Rename conversion" }));
  await waitFor(() =>
    expect(document.activeElement).toBe(screen.getByRole("textbox", { name: "Rename conversion" })),
  );
  await user.keyboard("{Escape}");
  expect(screen.queryByRole("textbox", { name: "Rename conversion" })).toBeNull();

  rename.mockRejectedValueOnce(new Error("sqlite busy"));
  await user.click(screen.getByRole("button", { name: "Rename conversion" }));
  await user.click(screen.getByRole("button", { name: "Save" }));
  expect((await screen.findByRole("alert")).textContent).toContain(
    "Could not rename this conversion",
  );
  expect(screen.getByRole("textbox", { name: "Rename conversion" })).toBeTruthy();
});

test("search, facets, sorting, and numbered pages produce server queries", async () => {
  const user = userEvent.setup();
  api.queryRunItems.mockResolvedValue(
    page([item(1)], { pageCount: 3, filteredTotal: 60, musicTotal: 60 }),
  );
  renderDetail();
  await screen.findByText("Track 1");
  expect(api.queryRunItems).toHaveBeenCalledWith(
    9,
    expect.objectContaining({ page: 1, pageSize: 25, sort: "ordinal", direction: "asc" }),
  );

  await user.type(screen.getByRole("searchbox", { name: "Search songs" }), "late target");
  await waitFor(
    () =>
      expect(api.queryRunItems).toHaveBeenCalledWith(
        9,
        expect.objectContaining({ search: "late target", page: 1 }),
      ),
    { timeout: 1_000 },
  );
  await user.type(screen.getByPlaceholderText("All artists"), "Artist B");
  await waitFor(() =>
    expect(api.queryRunItems).toHaveBeenCalledWith(
      9,
      expect.objectContaining({ artist: "Artist B", page: 1 }),
    ),
  );
  await user.selectOptions(screen.getByLabelText("Sort"), "size:desc");
  expect(api.queryRunItems).toHaveBeenCalledWith(
    9,
    expect.objectContaining({ sort: "size", direction: "desc", page: 1 }),
  );
  await user.click(screen.getByRole("button", { name: "2" }));
  expect(api.queryRunItems).toHaveBeenCalledWith(9, expect.objectContaining({ page: 2 }));
});

test("each filter clears independently without resetting the selected sort", async () => {
  const user = userEvent.setup();
  renderDetail();
  await screen.findByText("Track 1");
  await user.selectOptions(screen.getByLabelText("Sort"), "size:desc");
  await user.type(screen.getByPlaceholderText("All artists"), "Artist B");
  await user.click(screen.getByRole("button", { name: "Clear artist filter" }));
  await waitFor(() =>
    expect(api.queryRunItems).toHaveBeenLastCalledWith(
      9,
      expect.objectContaining({ artist: null, sort: "size", direction: "desc", page: 1 }),
    ),
  );
});

test("copied outcome is available as a server-side result filter", async () => {
  const user = userEvent.setup();
  renderDetail();
  await screen.findByText("Track 1");
  await user.selectOptions(screen.getByLabelText("Result"), "copied");
  await waitFor(() =>
    expect(api.queryRunItems).toHaveBeenCalledWith(
      9,
      expect.objectContaining({ outcome: "copied" }),
    ),
  );
});

test("row controls delegate each selected track to the shared player", async () => {
  const user = userEvent.setup();
  api.queryRunItems.mockResolvedValue(page([item(1), item(2)]));
  renderDetail();
  const playOne = await screen.findByRole("button", { name: "Play Track 1" });
  await user.click(playOne);
  await waitFor(() => expect(convertFileSrc).toHaveBeenCalledWith("1", "unlockm-audio"));
  await user.click(screen.getByRole("button", { name: "Play Track 2" }));
  await waitFor(() => expect(convertFileSrc).toHaveBeenCalledWith("2", "unlockm-audio"));
  expect(pauseSpy).toHaveBeenCalled();
  expect(screen.queryByRole("slider", { name: /Seek through/ })).toBeNull();
});

test("normal player cleanup releases the source without forcing an error-prone reload", async () => {
  const user = userEvent.setup();
  renderDetail();
  await user.click(await screen.findByRole("button", { name: "Play Track 1" }));
  await waitFor(() => expect(loadSpy).toHaveBeenCalledTimes(1));
  cleanup();
  expect(pauseSpy).toHaveBeenCalled();
  expect(loadSpy).toHaveBeenCalledTimes(1);
});

test("legacy repair refreshes once until repairPending clears", async () => {
  api.queryRunItems
    .mockResolvedValueOnce(page([item(1)], { repairPending: true }))
    .mockResolvedValueOnce(page([item(1)], { repairPending: false }));
  renderDetail();
  await waitFor(() => expect(api.queryRunItems).toHaveBeenCalledTimes(2), { timeout: 1_000 });
  await new Promise((resolve) => window.setTimeout(resolve, 350));
  expect(api.queryRunItems).toHaveBeenCalledTimes(2);
  cleanup();
});

test("file reveal and system-player buttons call separate ID-only commands", async () => {
  const user = userEvent.setup();
  renderDetail();
  await screen.findByText("Track 1");
  await user.click(screen.getByRole("button", { name: "Reveal Track 1 in its folder" }));
  await user.click(screen.getByRole("button", { name: "Open Track 1 in the system player" }));
  expect(api.revealRunItem).toHaveBeenCalledWith(1);
  expect(api.openRunItem).toHaveBeenCalledWith(1);
});

test("detail can retry a failed query", async () => {
  const user = userEvent.setup();
  api.queryRunItems
    .mockRejectedValueOnce(new Error("database unavailable"))
    .mockResolvedValueOnce(page([]));
  renderDetail();
  expect(await screen.findByText("Could not load these details.")).toBeTruthy();
  await user.click(screen.getByRole("button", { name: "Try again" }));
  await waitFor(() => expect(api.queryRunItems).toHaveBeenCalledTimes(2));
});

test("home artwork waits for visibility and bounds artwork requests", async () => {
  type ObserverCallback = (entries: IntersectionObserverEntry[]) => void;
  class FakeIntersectionObserver {
    static instances: FakeIntersectionObserver[] = [];
    callback: ObserverCallback;
    constructor(callback: ObserverCallback) {
      this.callback = callback;
      FakeIntersectionObserver.instances.push(this);
    }
    disconnect() {}
    observe() {}
    triggerVisible() {
      this.callback([{ isIntersecting: true } as IntersectionObserverEntry]);
    }
  }
  vi.stubGlobal("IntersectionObserver", FakeIntersectionObserver);
  api.getArtworkMime.mockResolvedValue({ id: 1, mimeType: "image/png" });
  api.getArtwork.mockResolvedValue(new ArrayBuffer(4));
  vi.spyOn(URL, "createObjectURL").mockReturnValue("blob:cover");
  render(
    <>
      {Array.from({ length: 30 }, (_, index) => (
        <VisibleArtwork artworkId={index + 1} key={index} title={`Track ${index}`} />
      ))}
    </>,
  );
  expect(FakeIntersectionObserver.instances).toHaveLength(30);
  expect(api.getArtwork).not.toHaveBeenCalled();
  for (const observer of FakeIntersectionObserver.instances.slice(0, 4)) observer.triggerVisible();
  await waitFor(() => expect(api.getArtwork).toHaveBeenCalledTimes(4));
});
