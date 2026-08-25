import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, expect, test, vi } from "vite-plus/test";
import i18n from "../../i18n";
import type { HistoryItem } from "../../lib/contracts";
import { Artwork, HistoryDetail, VisibleArtwork } from "./HistoryDetail";
import { clampTitleAlbumRatio, paginationItems, visibleItemNumber } from "./layout";
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

test("detail artwork clears the old cover when its artwork id becomes null", async () => {
  api.getArtworkMime.mockResolvedValue({ id: 1, mimeType: "image/png" });
  api.getArtwork.mockResolvedValue(new ArrayBuffer(4));
  vi.spyOn(URL, "createObjectURL").mockReturnValue("blob:detail-cover");
  const revoke = vi.spyOn(URL, "revokeObjectURL");
  const view = render(<Artwork artworkId={1} title="Old" />);
  await waitFor(() =>
    expect(document.querySelector("img.cover")?.getAttribute("src")).toBe("blob:detail-cover"),
  );
  view.rerender(<Artwork artworkId={null} title="New" />);
  expect(document.querySelector("img.cover")).toBeNull();
  expect(document.querySelector(".cover-placeholder")?.textContent).toBe("N");
  expect(revoke).toHaveBeenCalledWith("blob:detail-cover");
});

test("detail artwork remounts before returning to a previously revoked id", async () => {
  let resolveReturnedBytes: ((bytes: ArrayBuffer) => void) | undefined;
  api.getArtworkMime.mockResolvedValue({ id: 1, mimeType: "image/png" });
  api.getArtwork.mockResolvedValueOnce(new ArrayBuffer(4));
  api.getArtwork.mockResolvedValueOnce(new ArrayBuffer(4));
  api.getArtwork.mockReturnValueOnce(
    new Promise<ArrayBuffer>((resolve) => {
      resolveReturnedBytes = resolve;
    }),
  );
  vi.spyOn(URL, "createObjectURL")
    .mockReturnValueOnce("blob:detail-a")
    .mockReturnValueOnce("blob:detail-b")
    .mockReturnValueOnce("blob:detail-a-returned");
  const view = render(<Artwork artworkId={1} title="A" />);
  await waitFor(() =>
    expect(document.querySelector("img.cover")?.getAttribute("src")).toBe("blob:detail-a"),
  );
  view.rerender(<Artwork artworkId={2} title="B" />);
  await waitFor(() =>
    expect(document.querySelector("img.cover")?.getAttribute("src")).toBe("blob:detail-b"),
  );
  view.rerender(<Artwork artworkId={1} title="A returned" />);
  expect(document.querySelector("img.cover")).toBeNull();
  resolveReturnedBytes?.(new ArrayBuffer(4));
  await waitFor(() =>
    expect(document.querySelector("img.cover")?.getAttribute("src")).toBe("blob:detail-a-returned"),
  );
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

test("detail back navigation remains an identifiable sticky pill", async () => {
  renderDetail();
  await screen.findByText("Track 1");
  expect(screen.getByRole("button", { name: "Home" }).classList.contains("detail-back-pill")).toBe(
    true,
  );
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

test("compact filters reserve clear space only after a value is present in every locale", async () => {
  const user = userEvent.setup();
  renderDetail();
  await screen.findByText("Track 1");
  const search = screen.getByRole("searchbox", { name: "Search songs" });
  const artist = screen.getByPlaceholderText("All artists");
  expect(search.closest(".search-control")?.getAttribute("data-has-clear")).toBe("false");
  expect(artist.parentElement?.getAttribute("data-has-clear")).toBe("false");
  await user.type(search, "late");
  expect(search.closest(".search-control")?.getAttribute("data-has-clear")).toBe("true");

  for (const [language, searchPlaceholder, artistPlaceholder, albumPlaceholder] of [
    ["zh-CN", "搜索歌曲", "全部歌手", "全部专辑"],
    ["ja-JP", "曲を検索", "すべてのアーティスト", "すべてのアルバム"],
  ]) {
    await i18n.changeLanguage(language);
    expect(screen.getByPlaceholderText(searchPlaceholder)).toBeTruthy();
    expect(screen.getByPlaceholderText(artistPlaceholder)).toBeTruthy();
    expect(screen.getByPlaceholderText(albumPlaceholder)).toBeTruthy();
  }
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

test("media-library row keeps artist filtering and row playback separate from actions", async () => {
  const user = userEvent.setup();
  renderDetail();
  await screen.findByText("Track 1");
  const row = document.querySelector(".music-item")!;
  expect(row.querySelector(".track-index")?.textContent).toContain("01");
  await user.click(row);
  await waitFor(() => expect(convertFileSrc).toHaveBeenCalledWith("1", "unlockm-audio"));
  await user.click(screen.getByRole("button", { name: "Artist A" }));
  await waitFor(() =>
    expect(api.queryRunItems).toHaveBeenLastCalledWith(
      9,
      expect.objectContaining({ artist: "Artist A", page: 1 }),
    ),
  );
  await user.click(screen.getByRole("button", { name: "Reveal Track 1 in its folder" }));
  expect(api.revealRunItem).toHaveBeenCalledWith(1);
});

test("visible row numbers start at one for every result set and continue across pages", async () => {
  const user = userEvent.setup();
  api.queryRunItems.mockImplementation(async (_runId, query) =>
    page([item(query.page === 1 ? 57 : 88)], {
      page: query.page,
      pageCount: 3,
      filteredTotal: 75,
      musicTotal: 75,
    }),
  );
  renderDetail();
  await screen.findByText("Track 57");
  expect(document.querySelector(".track-number")?.textContent).toBe("01");

  await user.selectOptions(screen.getByLabelText("Sort"), "title:asc");
  await waitFor(() => expect(screen.getByText("Track 57")).toBeTruthy());
  expect(document.querySelector(".track-number")?.textContent).toBe("01");

  await user.click(screen.getByRole("button", { name: "2" }));
  await waitFor(() => expect(screen.getByText("Track 88")).toBeTruthy());
  expect(document.querySelector(".track-number")?.textContent).toBe("26");
  expect(visibleItemNumber(2, 10, 0)).toBe(11);
});

test("pagination keeps first, last, and nearby pages with accessible gaps", () => {
  expect(paginationItems(10, 2_000)).toEqual([
    1,
    2,
    "ellipsis",
    7,
    8,
    9,
    10,
    11,
    12,
    13,
    "ellipsis",
    1_999,
    2_000,
  ]);
  expect(paginationItems(2, 6)).toEqual([1, 2, 3, 4, 5, 6]);
  expect(paginationItems(1, 1)).toEqual([]);
});

test("pagination renders edge pages, nearby pages, and inert ellipses", async () => {
  api.queryRunItems.mockImplementation(async (_runId, query) =>
    page([item(query.page)], {
      page: query.page === 1 ? 10 : query.page,
      pageCount: 2_000,
      filteredTotal: 50_000,
      musicTotal: 50_000,
    }),
  );
  renderDetail();
  const pagination = await screen.findByRole("navigation", { name: "Song pages" });
  await waitFor(() =>
    expect(
      within(pagination).getByRole("button", { name: "10" }).getAttribute("aria-current"),
    ).toBe("page"),
  );
  expect(within(pagination).getByRole("button", { name: "1" })).toBeTruthy();
  expect(within(pagination).getByRole("button", { name: "2" })).toBeTruthy();
  expect(within(pagination).getByRole("button", { name: "1999" })).toBeTruthy();
  expect(within(pagination).getByRole("button", { name: "2000" })).toBeTruthy();
  expect(within(pagination).queryByRole("button", { name: "3" })).toBeNull();
  expect(pagination.querySelectorAll(".pagination-ellipsis")).toHaveLength(2);
});

test("title and album separator clamps pointer-independent keyboard adjustments", async () => {
  const user = userEvent.setup();
  renderDetail();
  await screen.findByText("Track 1");
  const separator = screen.getByRole("separator", { name: "Adjust title and album columns" });
  expect(separator.getAttribute("aria-valuenow")).toBe("60");
  await user.click(separator);
  await user.keyboard("{ArrowRight}");
  expect(separator.getAttribute("aria-valuenow")).toBe("65");
  await user.keyboard("{End}");
  expect(separator.getAttribute("aria-valuenow")).toBe("75");
  await user.keyboard("{Home}");
  expect(separator.getAttribute("aria-valuenow")).toBe("35");
  const row = document.querySelector<HTMLElement>(".music-item")!;
  const header = document.querySelector<HTMLElement>(".song-list-header")!;
  const songList = document.querySelector<HTMLElement>(".song-list")!;
  expect(separator.closest(".song-list-header")).toBe(header);
  expect(row.querySelector("[role='separator'], .title-album-separator")).toBeNull();
  expect(songList.style.getPropertyValue("--title-track")).toBe("35fr");
  expect(songList.style.getPropertyValue("--album-track")).toBe("65fr");
  expect(row.style.getPropertyValue("--title-track")).toBe("");
  expect(header.style.getPropertyValue("--album-track")).toBe("");
  expect(clampTitleAlbumRatio(-1)).toBe(35);
  expect(clampTitleAlbumRatio(101)).toBe(75);
});

test("title and album separator captures and releases pointer drags including cancellation", async () => {
  renderDetail();
  await screen.findByText("Track 1");
  const separator = screen.getByRole("separator", { name: "Adjust title and album columns" });
  const header = document.querySelector<HTMLElement>(".song-list-header")!;
  const songList = document.querySelector<HTMLElement>(".song-list")!;
  const titleTrack = header.querySelector<HTMLElement>(".song-title-track")!;
  const albumTrack = header.querySelector<HTMLElement>(".song-album-track")!;
  Object.assign(titleTrack, { getBoundingClientRect: () => ({ left: 100 }) });
  Object.assign(albumTrack, { getBoundingClientRect: () => ({ right: 500 }) });
  const setPointerCapture = vi.fn();
  const releasePointerCapture = vi.fn();
  const hasPointerCapture = vi.fn(() => true);
  Object.assign(separator, { setPointerCapture, releasePointerCapture, hasPointerCapture });

  fireEvent.pointerDown(separator, { clientX: 260, pointerId: 12 });
  expect(setPointerCapture).toHaveBeenCalledWith(12);
  await waitFor(() => expect(separator.getAttribute("aria-valuenow")).toBe("40"));
  expect(songList.style.getPropertyValue("--title-track")).toBe("40fr");
  expect(songList.style.getPropertyValue("--album-track")).toBe("60fr");
  fireEvent.pointerMove(window, { clientX: 380, pointerId: 12 });
  await waitFor(() => expect(separator.getAttribute("aria-valuenow")).toBe("70"));
  expect(songList.style.getPropertyValue("--title-track")).toBe("70fr");
  expect(songList.style.getPropertyValue("--album-track")).toBe("30fr");
  fireEvent.pointerUp(window, { pointerId: 12 });
  expect(releasePointerCapture).toHaveBeenCalledWith(12);

  fireEvent.pointerDown(separator, { clientX: 260, pointerId: 13 });
  fireEvent.pointerCancel(window, { pointerId: 13 });
  expect(releasePointerCapture).toHaveBeenLastCalledWith(13);
});

test("narrow song layout removes the title and album separator from the tab order", async () => {
  const mediaQuery = {
    matches: false,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
  };
  vi.stubGlobal(
    "matchMedia",
    vi.fn(() => mediaQuery),
  );
  renderDetail();
  await screen.findByText("Track 1");
  expect(screen.queryByRole("separator", { name: "Adjust title and album columns" })).toBeNull();
  expect(document.querySelector(".title-album-separator[tabindex]")).toBeNull();
});

test("all rows share stable tail tracks while long artist and album text stay intrinsic", async () => {
  const long = item(2, "An exceptionally long title that must never move the fixed file facts");
  long.artist = "An exceptionally long artist name that is clipped within the title column";
  long.album = "An exceptionally long album name that is clipped within the album column";
  long.outputFormat = "flac";
  long.outputBytes = 1_234_567_890;
  api.queryRunItems.mockResolvedValue(page([item(1), long]));
  renderDetail();
  await screen.findByText(long.title!);
  const rows = Array.from(document.querySelectorAll<HTMLElement>(".music-item"));
  const header = document.querySelector<HTMLElement>(".song-list-header")!;
  const songList = document.querySelector<HTMLElement>(".song-list")!;
  expect(rows).toHaveLength(2);
  expect(songList.style.getPropertyValue("--title-track")).toBe("60fr");
  expect(songList.style.getPropertyValue("--album-track")).toBe("40fr");
  expect(rows.every((row) => row.style.getPropertyValue("--title-track") === "")).toBe(true);
  expect(header.style.getPropertyValue("--album-track")).toBe("");
  expect(document.querySelectorAll("[role='separator']")).toHaveLength(1);
  expect(document.querySelectorAll(".music-item .title-album-spacer[tabindex]")).toHaveLength(0);
  const artist = screen.getByRole("button", { name: long.artist });
  const album = screen.getByRole("button", { name: long.album });
  expect(artist.getAttribute("title")).toBe(long.artist);
  expect(album.getAttribute("title")).toBe(long.album);
});

test("column header exposes localized compact media labels while rows keep the spacer inert", async () => {
  renderDetail();
  await screen.findByText("Track 1");
  const header = document.querySelector<HTMLElement>(".song-list-header")!;
  expect(header.textContent).toContain("Song");
  expect(header.textContent).toContain("Album");
  expect(header.textContent).toContain("Duration");
  expect(header.textContent).toContain("Format");
  expect(header.textContent).toContain("Size & bitrate");
  expect(header.textContent).toContain("Actions");
  expect(header.textContent).toContain("1 song total");
  const summary = header.querySelector(".song-list-count");
  expect(summary?.textContent).toBe("1 song total");
  expect(summary?.textContent).not.toContain("companion");
  await i18n.changeLanguage("zh-CN");
  await waitFor(() => expect(summary?.textContent).toBe("共 1 首"));
  const durationCells = document.querySelectorAll(".duration-cell");
  expect(durationCells).toHaveLength(2);
  expect(durationCells[1]?.textContent).toBe("1:05");
  const spacer = document.querySelector(".music-item .title-album-spacer");
  expect(spacer?.getAttribute("aria-hidden")).toBe("true");
  expect(spacer?.getAttribute("role")).toBeNull();
});

test("artist and album buttons filter independently without starting row playback", async () => {
  const user = userEvent.setup();
  renderDetail();
  await screen.findByText("Track 1");

  await user.click(screen.getByRole("button", { name: "Artist A" }));
  await waitFor(() =>
    expect(api.queryRunItems).toHaveBeenLastCalledWith(
      9,
      expect.objectContaining({ artist: "Artist A", page: 1 }),
    ),
  );
  expect(convertFileSrc).not.toHaveBeenCalled();

  await user.click(screen.getByRole("button", { name: "Album A" }));
  await waitFor(() =>
    expect(api.queryRunItems).toHaveBeenLastCalledWith(
      9,
      expect.objectContaining({ album: "Album A", page: 1 }),
    ),
  );
  expect(convertFileSrc).not.toHaveBeenCalled();
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
