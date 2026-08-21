import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, test, vi } from "vite-plus/test";
import "../../i18n";
import { Artwork, HistoryDetail, VisibleArtwork } from "./HistoryDetail";
import i18n from "../../i18n";

const { api } = vi.hoisted(() => ({
  api: { getArtwork: vi.fn(), getArtworkMime: vi.fn(), getRunItems: vi.fn() },
}));

const { virtualizerCalls } = vi.hoisted(() => ({ virtualizerCalls: [] as number[] }));

vi.mock("../../lib/ipc", () => ({ desktopApi: api }));

vi.mock("@tanstack/react-virtual", () => ({
  useVirtualizer: (options: { count: number }) => ({
    getTotalSize: () => options.count * 96,
    getVirtualItems: () => {
      virtualizerCalls.push(options.count);
      return Array.from({ length: Math.min(options.count, 12) }, (_, index) => ({
        index,
        start: index * 96,
        key: index,
        size: 96,
      }));
    },
    measureElement: () => undefined,
  }),
}));

afterEach(async () => {
  cleanup();
  vi.restoreAllMocks();
  api.getArtwork.mockReset();
  api.getArtworkMime.mockReset();
  api.getRunItems.mockReset();
  virtualizerCalls.length = 0;
  vi.unstubAllGlobals();
  await i18n.changeLanguage("en-US");
});

const run = {
  id: 9,
  inputKind: "directory" as const,
  inputPath: "/input/album",
  outputPath: "/output",
  settings: { updateMetadata: false, copyPassthrough: true, overwrite: false },
  status: "completed" as const,
  createdAtMs: 0,
  updatedAtMs: 0,
  finishedAtMs: 0,
  totalItems: 4,
  completedItems: 4,
  converted: 4,
  copied: 0,
  skipped: 0,
  conflicted: 0,
  failed: 0,
  representativeArtworkId: null,
  terminalErrorCode: null,
  terminalErrorDetail: null,
};

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

test("detail uses persisted music total and a paginated row", async () => {
  api.getRunItems.mockResolvedValue({
    items: [
      {
        id: 1,
        runId: 9,
        ordinal: 0,
        itemKind: "music",
        sourcePath: "/input/a.ncm",
        relativePath: "a.ncm",
        destinationPath: "/output/a.mp3",
        action: "convert",
        outcome: "converted",
        logicalBytes: 10,
        errorCode: null,
        errorDetail: null,
        warnings: [],
        title: "A track",
        artist: "Artist",
        album: "Album",
        durationMs: 65_000,
        artworkId: null,
      },
    ],
    nextCursor: null,
    companionCount: 2,
    musicTotal: 1,
  });
  render(<HistoryDetail layoutId="run-card-9" run={run} onBack={() => undefined} />);
  await waitFor(() => expect(screen.getByText("2 companion files")).toBeTruthy());
  expect(document.querySelector(".detail-count strong")?.textContent).toBe("1");
  expect(api.getRunItems).toHaveBeenCalledWith(9, null, 100);
});

test("missing artwork keeps placeholder", async () => {
  api.getArtworkMime.mockRejectedValue(new Error("missing"));
  render(<Artwork artworkId={22} title="Fallback" />);
  expect(screen.getByText("F")).toBeTruthy();
});

test("detail can retry a failed page load", async () => {
  const user = userEvent.setup();
  api.getRunItems
    .mockRejectedValueOnce(new Error("database unavailable"))
    .mockResolvedValueOnce({ items: [], nextCursor: null, companionCount: 0, musicTotal: 0 });
  render(<HistoryDetail layoutId="run-card-9" run={run} onBack={() => undefined} />);
  expect(await screen.findByText("Could not load these details.")).toBeTruthy();
  await user.click(screen.getByRole("button", { name: "Try again" }));
  expect(api.getRunItems).toHaveBeenNthCalledWith(1, 9, null, 100);
  expect(api.getRunItems).toHaveBeenNthCalledWith(2, 9, null, 100);
});

test("initial and pagination loading reserve stable skeleton space", async () => {
  let resolveInitial: ((page: object) => void) | undefined;
  let resolveMore: ((page: object) => void) | undefined;
  api.getRunItems
    .mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          resolveInitial = resolve;
        }),
    )
    .mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          resolveMore = resolve;
        }),
    );
  const user = userEvent.setup();
  render(<HistoryDetail layoutId="run-card-9" run={run} onBack={() => undefined} />);
  expect(document.querySelectorAll(".music-skeleton")).toHaveLength(5);
  resolveInitial?.({
    items: [
      {
        id: 1,
        runId: 9,
        ordinal: 0,
        itemKind: "music",
        sourcePath: "/input/a.ncm",
        relativePath: "a.ncm",
        destinationPath: "/output/a.mp3",
        action: "convert",
        outcome: "converted",
        logicalBytes: 10,
        errorCode: null,
        errorDetail: null,
        warnings: [],
        title: "A track",
        artist: "Artist",
        album: "Album",
        durationMs: 65_000,
        artworkId: null,
      },
    ],
    nextCursor: 5,
    companionCount: 0,
    musicTotal: 8,
  });
  expect(await screen.findByRole("button", { name: "Load more" })).toBeTruthy();
  await user.click(screen.getByRole("button", { name: "Load more" }));
  await waitFor(() => expect(document.querySelector(".pagination-skeleton")).toBeTruthy());
  resolveMore?.({ items: [], nextCursor: null, companionCount: 0, musicTotal: 8 });
  await waitFor(() => expect(document.querySelector(".pagination-skeleton")).toBeNull());
});

test("item error codes use localized product copy while raw diagnostics remain collapsed", async () => {
  api.getRunItems.mockResolvedValue({
    items: [
      {
        id: 1,
        runId: 9,
        ordinal: 0,
        itemKind: "music",
        sourcePath: "/input/a.ncm",
        relativePath: "a.ncm",
        destinationPath: null,
        action: "convert",
        outcome: "failed",
        logicalBytes: 10,
        errorCode: "preflight_failed",
        errorDetail: "unsupported encrypted stream #42",
        warnings: [{ code: "artwork_snapshot_unavailable", detail: "cover too large" }],
        title: "A track",
        artist: "Artist",
        album: "Album",
        durationMs: 65_000,
        artworkId: null,
      },
    ],
    nextCursor: null,
    companionCount: 0,
    musicTotal: 1,
  });
  render(<HistoryDetail layoutId="run-card-9" run={run} onBack={() => undefined} />);
  expect(await screen.findByText("UnlockM could not read this encrypted file.")).toBeTruthy();
  const diagnostics = document.querySelector(".music-item details") as HTMLDetailsElement;
  expect(diagnostics.open).toBe(false);
  expect(diagnostics.textContent).toContain("unsupported encrypted stream #42");
  expect(diagnostics.textContent).toContain("Artwork could not be loaded.");
  await i18n.changeLanguage("zh-CN");
  expect(await screen.findByText("UnlockM 无法读取这个加密文件。")).toBeTruthy();
  expect(screen.getByText("无法加载封面。", { exact: false })).toBeTruthy();
});

test("home artwork waits for visibility and bounds requests across a long history", async () => {
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
    unobserve() {}
    takeRecords() {
      return [];
    }
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
  expect(api.getArtworkMime).toHaveBeenCalledTimes(4);
  expect(api.getArtwork).toHaveBeenCalledTimes(4);
});

test("a thousand loaded items are delegated to a bounded virtual window", async () => {
  const items = Array.from({ length: 1_000 }, (_, index) => ({
    id: index + 1,
    runId: 9,
    ordinal: index,
    itemKind: "music" as const,
    sourcePath: `/input/${index}.ncm`,
    relativePath: `${index}.ncm`,
    destinationPath: `/output/${index}.mp3`,
    action: "convert",
    outcome: "converted",
    logicalBytes: 10,
    errorCode: null,
    errorDetail: null,
    warnings: [],
    title: `Track ${index}`,
    artist: "Artist",
    album: "Album",
    durationMs: 65_000,
    artworkId: null,
  }));
  api.getRunItems.mockResolvedValue({
    items,
    nextCursor: null,
    companionCount: 0,
    musicTotal: items.length,
  });

  render(<HistoryDetail layoutId="run-card-9" run={run} onBack={() => undefined} />);

  await waitFor(() => expect(virtualizerCalls).toContain(1_000));
  expect(document.querySelectorAll(".music-item").length).toBeLessThanOrEqual(12);
  expect(screen.getByText("Track 0")).toBeTruthy();
  expect(screen.queryByText("Track 999")).toBeNull();
});
