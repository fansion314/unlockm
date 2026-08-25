import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, test, vi } from "vite-plus/test";
import i18n from "./i18n";

const { api } = vi.hoisted(() => ({
  api: {
    bootstrap: vi.fn(),
    validateInput: vi.fn(),
    listRuns: vi.fn(),
    queryRunItems: vi.fn(),
    listRunItemFacets: vi.fn(),
    startConversion: vi.fn(),
    cancelConversion: vi.fn(),
    getPreferences: vi.fn(),
    setPreferences: vi.fn(),
    deleteRun: vi.fn(),
    renameRun: vi.fn(),
    rerunConversion: vi.fn(),
    clearHistory: vi.fn(),
    revealRunOutput: vi.fn(),
    revealRunItem: vi.fn(),
    openRunItem: vi.fn(),
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
  musicCount: 2,
  representativeArtworkId: null,
  terminalErrorCode: null,
  terminalErrorDetail: null,
};

function emptyItemPage() {
  return {
    items: [],
    page: 1,
    pageSize: 25,
    pageCount: 0,
    filteredTotal: 0,
    companionCount: 0,
    musicTotal: 0,
  };
}

function bootstrap(overrides = {}) {
  return {
    productName: "UnlockM" as const,
    supportedLocales: ["zh-CN", "en-US", "ja-JP"] as ["zh-CN", "en-US", "ja-JP"],
    preferences: {
      locale: "en-US" as const,
      theme: "light" as const,
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
  document.documentElement.dataset.theme = "light";
  window.localStorage.clear();
  for (const method of Object.values(api)) method.mockReset();
});

test("history row exposes full record facts and output action", async () => {
  const user = userEvent.setup();
  api.bootstrap.mockResolvedValue(bootstrap({ history: { runs: [partialRun], nextCursor: null } }));
  api.revealRunOutput.mockResolvedValue(true);
  render(<App />);

  expect(await screen.findByText("Completed with issues")).toBeTruthy();
  expect(screen.getByText(longPath.split(/[\\/]/).pop() ?? "")).toBeTruthy();
  expect(screen.getByText("/output")).toBeTruthy();
  expect(screen.getByText("2")).toBeTruthy();
  await user.click(screen.getByRole("button", { name: /Reveal the output folder/ }));
  expect(api.revealRunOutput).toHaveBeenCalledWith(partialRun.id);
});

test("home/detail navigation resets scroll position", async () => {
  const user = userEvent.setup();
  const scrollTo = vi.fn();
  vi.stubGlobal("scrollTo", scrollTo);
  api.bootstrap.mockResolvedValue(bootstrap({ history: { runs: [partialRun], nextCursor: null } }));
  api.queryRunItems.mockResolvedValue(emptyItemPage());
  api.listRunItemFacets.mockResolvedValue({ values: [], nextCursor: null });
  render(<App />);
  await user.click(await screen.findByRole("button", { name: /Open details/ }));
  expect(scrollTo).toHaveBeenCalledWith(expect.objectContaining({ top: 0 }));
});

test("home omits rename while detail owns rename and rerun stays on the row", async () => {
  const user = userEvent.setup();
  api.bootstrap.mockResolvedValue(bootstrap({ history: { runs: [partialRun], nextCursor: null } }));
  api.renameRun.mockResolvedValue({ ...partialRun, displayName: "Downloads" });
  api.rerunConversion.mockResolvedValue({ ...partialRun, id: 5, displayName: "Downloads" });
  api.queryRunItems.mockResolvedValue(emptyItemPage());
  api.listRunItemFacets.mockResolvedValue({ values: [], nextCursor: null });
  render(<App />);
  await screen.findByText("Completed with issues");

  expect(screen.queryByRole("button", { name: "Rename conversion" })).toBeNull();
  await user.click(
    screen.getByRole("button", {
      name: `Open details for ${longPath.split(/[\\/]/).pop()}`,
    }),
  );
  await screen.findByRole("button", { name: "Rename conversion" });
  await user.click(screen.getByRole("button", { name: "Rename conversion" }));
  const input = screen.getByRole("textbox", { name: "Rename conversion" });
  await user.clear(input);
  await user.type(input, "Downloads");
  await user.click(screen.getByRole("button", { name: "Save" }));
  expect(api.renameRun).toHaveBeenCalledWith(partialRun.id, "Downloads");
  await user.click(screen.getByRole("button", { name: "Home" }));
  await user.click(await screen.findByRole("button", { name: "Run conversion again" }));
  expect(api.rerunConversion).toHaveBeenCalledWith(partialRun.id, expect.any(Function));
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

test("fresh light theme, dark toggle, and language menu persist one preference snapshot", async () => {
  const user = userEvent.setup();
  api.bootstrap.mockResolvedValue(bootstrap());
  api.setPreferences.mockImplementation(async (value) => value);
  render(<App />);
  await screen.findByText("Your finished conversions will live here.");
  expect(document.documentElement.dataset.theme).toBe("light");

  await user.click(screen.getByRole("button", { name: "Use dark theme" }));
  expect(document.documentElement.dataset.theme).toBe("dark");
  expect(api.setPreferences).toHaveBeenLastCalledWith({
    locale: "en-US",
    theme: "dark",
    palette: "blue",
    playerVolume: 0.6,
    defaultSettings: { updateMetadata: false, copyPassthrough: true, overwrite: false },
  });

  await user.click(screen.getByRole("button", { name: "Choose language" }));
  await user.click(screen.getByRole("menuitemradio", { name: "简体中文" }));
  await waitFor(() => expect(screen.getByText("音乐解锁")).toBeTruthy());
  expect(api.setPreferences).toHaveBeenLastCalledWith({
    locale: "zh-CN",
    theme: "dark",
    palette: "blue",
    playerVolume: 0.6,
    defaultSettings: { updateMetadata: false, copyPassthrough: true, overwrite: false },
  });

  await user.click(screen.getByRole("button", { name: "选择语言" }));
  await user.click(screen.getByRole("menuitemradio", { name: "日本語" }));
  await waitFor(() => expect(screen.getByText("音楽ロック解除")).toBeTruthy());
  expect(api.setPreferences).toHaveBeenLastCalledWith({
    locale: "ja-JP",
    theme: "dark",
    palette: "blue",
    playerVolume: 0.6,
    defaultSettings: { updateMetadata: false, copyPassthrough: true, overwrite: false },
  });
});

test("language menu uses roving arrow focus and restores the trigger on Escape", async () => {
  const user = userEvent.setup();
  api.bootstrap.mockResolvedValue(bootstrap());
  render(<App />);
  await screen.findByText("Your finished conversions will live here.");
  const trigger = screen.getByRole("button", { name: "Choose language" });
  await user.click(trigger);
  const english = screen.getByRole("menuitemradio", { name: "English" });
  const chinese = screen.getByRole("menuitemradio", { name: "简体中文" });
  const japanese = screen.getByRole("menuitemradio", { name: "日本語" });
  await waitFor(() => expect(document.activeElement).toBe(english));
  await user.keyboard("{ArrowUp}");
  expect(document.activeElement).toBe(chinese);
  await user.keyboard("{End}");
  expect(document.activeElement).toBe(japanese);
  await user.keyboard("{Escape}");
  await waitFor(() => expect(document.activeElement).toBe(trigger));
  expect(screen.queryByRole("menu")).toBeNull();
});

test("palette menu is collapsed by default, persists selection, and supports keyboard dismissal", async () => {
  const user = userEvent.setup();
  api.bootstrap.mockResolvedValue(bootstrap());
  api.setPreferences.mockImplementation(async (value) => value);
  render(<App />);
  await screen.findByText("Your finished conversions will live here.");
  const trigger = screen.getByRole("button", { name: "Choose color palette" });
  expect(screen.queryByRole("menu", { name: "Choose color palette" })).toBeNull();
  await user.click(trigger);
  expect(screen.getAllByRole("menuitemradio")).toHaveLength(4);
  const paletteOptions = screen.getAllByRole("menuitemradio");
  const checkSlots = paletteOptions.map((option) => option.querySelector(".palette-check-slot"));
  expect(checkSlots).toHaveLength(4);
  expect(checkSlots.every(Boolean)).toBe(true);
  expect(checkSlots.map((slot) => slot?.getAttribute("data-selected"))).toEqual([
    "true",
    "false",
    "false",
    "false",
  ]);
  await waitFor(() =>
    expect(document.activeElement).toBe(
      screen.getByRole("menuitemradio", { name: "Blue palette" }),
    ),
  );
  await user.keyboard("{ArrowDown}");
  expect(document.activeElement).toBe(
    screen.getByRole("menuitemradio", { name: "Emerald palette" }),
  );
  await user.keyboard("{End}");
  expect(document.activeElement).toBe(screen.getByRole("menuitemradio", { name: "Amber palette" }));
  await user.keyboard("{Home}");
  expect(document.activeElement).toBe(screen.getByRole("menuitemradio", { name: "Blue palette" }));
  await user.keyboard("{Escape}");
  await waitFor(() => expect(document.activeElement).toBe(trigger));
  expect(screen.queryByRole("menu", { name: "Choose color palette" })).toBeNull();

  await user.click(trigger);
  fireEvent.pointerDown(document.body);
  expect(screen.queryByRole("menu", { name: "Choose color palette" })).toBeNull();
  await user.click(trigger);
  await user.click(screen.getByRole("menuitemradio", { name: "Emerald palette" }));
  expect(document.documentElement.dataset.palette).toBe("emerald");
  expect(api.setPreferences).toHaveBeenLastCalledWith({
    locale: "en-US",
    theme: "light",
    palette: "emerald",
    playerVolume: 0.6,
    defaultSettings: { updateMetadata: false, copyPassthrough: true, overwrite: false },
  });
});

test("dedicated row controls keep detail, reveal, and deletion isolated", async () => {
  const user = userEvent.setup();
  const confirm = vi.spyOn(window, "confirm").mockReturnValue(true);
  api.bootstrap.mockResolvedValue(bootstrap({ history: { runs: [partialRun], nextCursor: null } }));
  api.queryRunItems.mockResolvedValue(emptyItemPage());
  api.listRunItemFacets.mockResolvedValue({ values: [], nextCursor: null });
  api.deleteRun.mockResolvedValue(undefined);
  render(<App />);
  await screen.findByText("Completed with issues");

  const detail = screen.getByRole("button", {
    name: `Open details for ${longPath.split(/[\\/]/).pop()}`,
  });
  detail.focus();
  await user.keyboard("{Enter}");
  expect(await screen.findByRole("button", { name: "Home" })).toBeTruthy();
  expect(api.deleteRun).not.toHaveBeenCalled();

  await user.click(screen.getByRole("button", { name: "Home" }));
  await user.click(await screen.findByRole("button", { name: "Remove record" }));
  expect(confirm).toHaveBeenCalledTimes(1);
  await waitFor(() => expect(api.deleteRun).toHaveBeenCalledWith(partialRun.id));
});

test("bootstrap errors keep product and technical messages separate", async () => {
  api.bootstrap.mockRejectedValue({ code: "history_error", detail: "sqlite busy timeout" });
  render(<App />);
  expect(await screen.findByText("Local history is temporarily unavailable.")).toBeTruthy();
  const details = document.querySelector(".error-banner details") as HTMLDetailsElement;
  expect(details.open).toBe(false);
  expect(screen.getByText("sqlite busy timeout")).toBeTruthy();
});

test("loading another history page reserves a row-shaped skeleton", async () => {
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
  await waitFor(() => expect(document.querySelector(".pagination-skeleton")).toBeTruthy());
  resolvePage?.({ runs: [], nextCursor: null });
  await waitFor(() => expect(document.querySelector(".pagination-skeleton")).toBeNull());
});
