import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, test, vi } from "vite-plus/test";
import i18n from "./i18n";

const { api } = vi.hoisted(() => ({
  api: {
    bootstrap: vi.fn(),
    validateInput: vi.fn(),
    listRuns: vi.fn(),
    getRunItems: vi.fn(),
    startConversion: vi.fn(),
    cancelConversion: vi.fn(),
    getPreferences: vi.fn(),
    setPreferences: vi.fn(),
    deleteRun: vi.fn(),
    clearHistory: vi.fn(),
    getArtwork: vi.fn(),
    getArtworkMime: vi.fn(),
  },
}));

vi.mock("./lib/ipc", () => ({ desktopApi: api }));

import App from "./App";

const longPath = `/input/${"very-long-segment-".repeat(10)}album.ncm`;
const partialRun = {
  id: 4,
  inputKind: "file" as const,
  inputPath: longPath,
  outputPath: "/output",
  settings: { updateMetadata: false, copyPassthrough: true, overwrite: false },
  status: "partial" as const,
  createdAtMs: 0,
  updatedAtMs: 0,
  finishedAtMs: 0,
  totalItems: 2,
  completedItems: 2,
  converted: 1,
  copied: 0,
  skipped: 0,
  conflicted: 0,
  failed: 1,
  representativeArtworkId: null,
  terminalErrorCode: null,
  terminalErrorDetail: null,
};

function bootstrap(overrides = {}) {
  return {
    productName: "UnlockM" as const,
    supportedLocales: ["zh-CN", "en-US"] as ["zh-CN", "en-US"],
    preferences: {
      locale: "en-US" as const,
      defaultSettings: { updateMetadata: false, copyPassthrough: true, overwrite: false },
    },
    history: { runs: [], nextCursor: null },
    activeRun: null,
    ...overrides,
  };
}

afterEach(async () => {
  cleanup();
  vi.restoreAllMocks();
  await i18n.changeLanguage("en-US");
  for (const method of Object.values(api)) method.mockReset();
});

test("long partial row", async () => {
  api.bootstrap.mockResolvedValue(bootstrap({ history: { runs: [partialRun], nextCursor: null } }));
  render(<App />);
  expect(await screen.findByText("Completed with issues")).toBeTruthy();
  expect(screen.getByText(longPath.split(/[\\/]/).pop() ?? "")).toBeTruthy();
  expect(screen.getByText("2/2")).toBeTruthy();
  expect(screen.getByText(/Jan 1, 1970/)).toBeTruthy();
});

test("bootstrap history reserves stable rows while local history is loading", async () => {
  let resolveBootstrap: ((value: ReturnType<typeof bootstrap>) => void) | undefined;
  api.bootstrap.mockImplementation(
    () =>
      new Promise((resolve) => {
        resolveBootstrap = resolve;
      }),
  );
  render(<App />);
  expect(document.querySelectorAll(".run-skeleton")).toHaveLength(3);
  resolveBootstrap?.(bootstrap());
  expect(await screen.findByText("Your finished conversions will live here.")).toBeTruthy();
  await waitFor(() => expect(document.querySelectorAll(".run-skeleton")).toHaveLength(0));
});

test("bootstrap error separates product message from technical detail", async () => {
  api.bootstrap.mockRejectedValue({ code: "history_error", detail: "sqlite busy timeout" });
  render(<App />);
  expect(await screen.findByText("Local history is temporarily unavailable.")).toBeTruthy();
  const details = document.querySelector(".error-banner details") as HTMLDetailsElement;
  expect(details.open).toBe(false);
  expect(screen.getByText("sqlite busy timeout")).toBeTruthy();
});

test("locale switch persists local preference defaults", async () => {
  const user = userEvent.setup();
  api.bootstrap.mockResolvedValue(bootstrap());
  api.setPreferences.mockImplementation(async (value) => value);
  render(<App />);
  await screen.findByText("Your finished conversions will live here.");
  await user.click(screen.getByRole("button", { name: "中文" }));
  await waitFor(() => expect(api.setPreferences).toHaveBeenCalled());
  expect(api.setPreferences).toHaveBeenCalledWith({
    locale: "zh-CN",
    defaultSettings: { updateMetadata: false, copyPassthrough: true, overwrite: false },
  });
});

test("keyboard activation of the dedicated detail button navigates only to detail", async () => {
  const user = userEvent.setup();
  api.bootstrap.mockResolvedValue(bootstrap({ history: { runs: [partialRun], nextCursor: null } }));
  api.getRunItems.mockResolvedValue({
    items: [],
    nextCursor: null,
    companionCount: 0,
    musicTotal: 0,
  });
  render(<App />);
  await screen.findByText("Completed with issues");
  const openDetails = screen.getByRole("button", {
    name: `Open details for ${longPath.split(/[\\/]/).pop()}`,
  });
  openDetails.focus();
  await user.keyboard("{Enter}");
  expect(await screen.findByRole("button", { name: /Back to conversions/ })).toBeTruthy();
  expect(api.deleteRun).not.toHaveBeenCalled();
});

test("the detail-control accessible name follows the current locale", async () => {
  api.bootstrap.mockResolvedValue(bootstrap({ history: { runs: [partialRun], nextCursor: null } }));
  render(<App />);
  const filename = longPath.split(/[\\/]/).pop();
  await screen.findByRole("button", { name: `Open details for ${filename}` });
  await i18n.changeLanguage("zh-CN");
  expect(await screen.findByRole("button", { name: `打开 ${filename} 的详情` })).toBeTruthy();
});

test("keyboard activation of the sibling delete control does not navigate to detail", async () => {
  const user = userEvent.setup();
  const confirm = vi.spyOn(window, "confirm").mockReturnValue(true);
  api.bootstrap.mockResolvedValue(bootstrap({ history: { runs: [partialRun], nextCursor: null } }));
  api.deleteRun.mockResolvedValue(undefined);
  render(<App />);
  await screen.findByText("Completed with issues");
  const openDetails = screen.getByRole("button", {
    name: `Open details for ${longPath.split(/[\\/]/).pop()}`,
  });
  const remove = screen.getByRole("button", { name: "Remove record" });
  openDetails.focus();
  await user.tab();
  expect(document.activeElement).toBe(remove);
  await user.keyboard("{Enter}");
  expect(confirm).toHaveBeenCalledTimes(1);
  await waitFor(() => expect(api.deleteRun).toHaveBeenCalledWith(partialRun.id));
  expect(screen.queryByRole("button", { name: /Back to conversions/ })).toBeNull();
});

test("loading another history page reserves a stable history-row skeleton", async () => {
  const user = userEvent.setup();
  let resolvePage:
    | ((page: { runs: (typeof partialRun)[]; nextCursor: number | null }) => void)
    | undefined;
  api.bootstrap.mockResolvedValue(bootstrap({ history: { runs: [partialRun], nextCursor: 3 } }));
  api.listRuns.mockImplementation(
    () =>
      new Promise((resolve) => {
        resolvePage = resolve;
      }),
  );
  render(<App />);
  await screen.findByText("Completed with issues");
  await user.click(screen.getByRole("button", { name: "Load more" }));
  await waitFor(() => expect(document.querySelector(".history-pagination-skeleton")).toBeTruthy());
  resolvePage?.({ runs: [], nextCursor: null });
  await waitFor(() => expect(document.querySelector(".history-pagination-skeleton")).toBeNull());
});
