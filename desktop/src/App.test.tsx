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
      themeId: "aurora-light" as const,
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
  document.documentElement.dataset.theme = "aurora-light";
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

test("theme switching preserves the current detail route and scroll state", async () => {
  const user = userEvent.setup();
  const scrollTo = vi.fn();
  vi.stubGlobal("scrollTo", scrollTo);
  api.bootstrap.mockResolvedValue(bootstrap({ history: { runs: [partialRun], nextCursor: null } }));
  api.queryRunItems.mockResolvedValue(emptyItemPage());
  api.listRunItemFacets.mockResolvedValue({ values: [], nextCursor: null });
  api.setPreferences.mockImplementation(async (value) => value);
  render(<App />);
  await user.click(await screen.findByRole("button", { name: /Open details/ }));
  await screen.findByRole("button", { name: "Home" });
  scrollTo.mockClear();

  await user.click(screen.getByRole("button", { name: "Choose appearance theme" }));
  await user.click(screen.getByRole("menuitemradio", { name: /Dracula/ }));

  expect(document.documentElement.dataset.theme).toBe("dracula");
  expect(screen.getByRole("button", { name: "Home" })).toBeTruthy();
  expect(screen.getByText("Completed with issues")).toBeTruthy();
  expect(scrollTo).not.toHaveBeenCalled();
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
  await waitFor(() => expect(resolveBootstrap).toBeTypeOf("function"));
  resolveBootstrap?.(bootstrap());
  expect(await screen.findByText("Your finished conversions will live here.")).toBeTruthy();
  await waitFor(() => expect(document.querySelectorAll(".run-skeleton")).toHaveLength(0));
});

test("fresh Aurora theme, full preset selection, and language persist one preference snapshot", async () => {
  const user = userEvent.setup();
  api.bootstrap.mockResolvedValue(bootstrap());
  api.setPreferences.mockImplementation(async (value) => value);
  render(<App />);
  await screen.findByText("Your finished conversions will live here.");
  expect(document.documentElement.dataset.theme).toBe("aurora-light");

  await user.click(screen.getByRole("button", { name: "Choose appearance theme" }));
  await user.click(screen.getByRole("menuitemradio", { name: /Tokyo Night/ }));
  expect(document.documentElement.dataset.theme).toBe("tokyo-night");
  expect(document.documentElement.style.colorScheme).toBe("dark");
  expect(api.setPreferences).toHaveBeenLastCalledWith({
    locale: "en-US",
    themeId: "tokyo-night",
    playerVolume: 0.6,
    defaultSettings: { updateMetadata: false, copyPassthrough: true, overwrite: false },
  });

  await user.click(screen.getByRole("button", { name: "Choose language" }));
  await user.click(screen.getByRole("menuitemradio", { name: "简体中文" }));
  await waitFor(() => expect(screen.getByText("音乐解锁")).toBeTruthy());
  expect(api.setPreferences).toHaveBeenLastCalledWith({
    locale: "zh-CN",
    themeId: "tokyo-night",
    playerVolume: 0.6,
    defaultSettings: { updateMetadata: false, copyPassthrough: true, overwrite: false },
  });

  await user.click(screen.getByRole("button", { name: "选择语言" }));
  await user.click(screen.getByRole("menuitemradio", { name: "日本語" }));
  await waitFor(() => expect(screen.getByText("音楽ロック解除")).toBeTruthy());
  expect(api.setPreferences).toHaveBeenLastCalledWith({
    locale: "ja-JP",
    themeId: "tokyo-night",
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

test("theme menu exposes nine previews, persists selection, and supports two-dimensional keys", async () => {
  const user = userEvent.setup();
  api.bootstrap.mockResolvedValue(bootstrap());
  api.setPreferences.mockImplementation(async (value) => value);
  render(<App />);
  await screen.findByText("Your finished conversions will live here.");
  const trigger = screen.getByRole("button", { name: "Choose appearance theme" });
  expect(screen.queryByRole("menu", { name: "Choose appearance theme" })).toBeNull();
  await user.click(trigger);
  expect(screen.getAllByRole("menuitemradio")).toHaveLength(9);
  const themeOptions = screen.getAllByRole("menuitemradio");
  const checkSlots = themeOptions.map((option) => option.querySelector(".theme-check-slot"));
  expect(checkSlots).toHaveLength(9);
  expect(checkSlots.every(Boolean)).toBe(true);
  expect(checkSlots.map((slot) => slot?.getAttribute("data-selected"))).toEqual([
    "true",
    "false",
    "false",
    "false",
    "false",
    "false",
    "false",
    "false",
    "false",
  ]);
  await waitFor(() =>
    expect(document.activeElement).toBe(
      screen.getByRole("menuitemradio", { name: /Aurora Light/ }),
    ),
  );
  await user.keyboard("{ArrowUp}");
  expect(document.activeElement).toBe(
    screen.getByRole("menuitemradio", { name: /Rosé Pine Dawn/ }),
  );
  await user.keyboard("{ArrowDown}");
  expect(document.activeElement).toBe(screen.getByRole("menuitemradio", { name: /Aurora Light/ }));
  await user.keyboard("{ArrowDown}");
  expect(document.activeElement).toBe(screen.getByRole("menuitemradio", { name: /Tokyo Night/ }));
  await user.keyboard("{ArrowRight}");
  expect(document.activeElement).toBe(
    screen.getByRole("menuitemradio", { name: /Catppuccin Mocha/ }),
  );
  await user.keyboard("{End}");
  expect(document.activeElement).toBe(
    screen.getByRole("menuitemradio", { name: /Rosé Pine Dawn/ }),
  );
  await user.keyboard("{Home}");
  expect(document.activeElement).toBe(screen.getByRole("menuitemradio", { name: /Aurora Light/ }));
  await user.keyboard("{ArrowRight}{ArrowUp}");
  expect(document.activeElement).toBe(screen.getByRole("menuitemradio", { name: /Gruvbox Dark/ }));
  await user.keyboard("{ArrowDown}");
  expect(document.activeElement).toBe(screen.getByRole("menuitemradio", { name: /Aurora Dark/ }));
  await user.keyboard("{Escape}");
  await waitFor(() => expect(document.activeElement).toBe(trigger));
  expect(screen.queryByRole("menu", { name: "Choose appearance theme" })).toBeNull();

  await user.click(trigger);
  fireEvent.pointerDown(document.body);
  expect(screen.queryByRole("menu", { name: "Choose appearance theme" })).toBeNull();
  await user.click(trigger);
  await user.click(screen.getByRole("menuitemradio", { name: /Catppuccin Mocha/ }));
  expect(document.documentElement.dataset.theme).toBe("catppuccin-mocha");
  expect(api.setPreferences).toHaveBeenLastCalledWith({
    locale: "en-US",
    themeId: "catppuccin-mocha",
    playerVolume: 0.6,
    defaultSettings: { updateMetadata: false, copyPassthrough: true, overwrite: false },
  });
});

test("theme persistence failure keeps the optimistic session theme and reports the error", async () => {
  const user = userEvent.setup();
  api.bootstrap.mockResolvedValue(bootstrap());
  api.setPreferences.mockRejectedValue({
    code: "history_error",
    detail: "preference write failed",
  });
  render(<App />);
  await screen.findByText("Your finished conversions will live here.");

  await user.click(screen.getByRole("button", { name: "Choose appearance theme" }));
  await user.click(screen.getByRole("menuitemradio", { name: /Nord/ }));

  expect(document.documentElement.dataset.theme).toBe("nord");
  expect(screen.queryByRole("menu", { name: "Choose appearance theme" })).toBeNull();
  expect((await screen.findByRole("alert")).textContent).toContain(
    "Local history is temporarily unavailable.",
  );
  expect(screen.getByText("preference write failed")).toBeTruthy();

  cleanup();
  api.bootstrap.mockResolvedValue(bootstrap());
  render(<App />);
  await screen.findByText("Your finished conversions will live here.");
  expect(document.documentElement.dataset.theme).toBe("aurora-light");
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
