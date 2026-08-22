import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, test, vi } from "vite-plus/test";
import "../../i18n";
import { Composer } from "./Composer";
import { singleDroppedPath } from "./drop";

const { openMock, isTauriMock, onDragDropEventMock } = vi.hoisted(() => ({
  openMock: vi.fn(),
  isTauriMock: vi.fn(() => false),
  onDragDropEventMock: vi.fn(),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({ open: openMock }));
vi.mock("@tauri-apps/api/core", () => ({ isTauri: isTauriMock }));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ onDragDropEvent: onDragDropEventMock }),
}));

afterEach(() => {
  cleanup();
  openMock.mockReset();
  isTauriMock.mockReset();
  isTauriMock.mockReturnValue(false);
  onDragDropEventMock.mockReset();
});

const settings = { updateMetadata: false, copyPassthrough: true, overwrite: false };

function props(overrides = {}) {
  return {
    busy: false,
    outputPath: "/output",
    settings,
    onSelectPath: vi.fn(async () => undefined),
    onOutputPath: vi.fn(),
    onSettings: vi.fn(),
    onConvert: vi.fn(),
    onError: vi.fn(),
    ...overrides,
  };
}

test("drop validation accepts exactly one root", () => {
  expect(singleDroppedPath([])).toBeNull();
  expect(singleDroppedPath(["/one", "/two"])).toBeNull();
  expect(singleDroppedPath(["/one"])).toBe("/one");
});

test("native drag listener unregisters on unmount", async () => {
  const unlisten = vi.fn();
  isTauriMock.mockReturnValue(true);
  onDragDropEventMock.mockResolvedValue(unlisten);
  render(<Composer {...props()} selected={null} />);
  await vi.waitFor(() => expect(onDragDropEventMock).toHaveBeenCalled());
  cleanup();
  expect(unlisten).toHaveBeenCalled();
});

test("a stale native drop callback cannot update an unmounted composer", async () => {
  const unlisten = vi.fn();
  const callbacks = props();
  let listener: ((event: unknown) => void) | undefined;
  isTauriMock.mockReturnValue(true);
  onDragDropEventMock.mockImplementation(async (callback) => {
    listener = callback;
    return unlisten;
  });
  render(<Composer {...callbacks} selected={null} />);
  await vi.waitFor(() => expect(listener).toBeDefined());
  cleanup();
  listener?.({ payload: { type: "drop", paths: ["/late.ncm"] } });
  expect(callbacks.onSelectPath).not.toHaveBeenCalled();
  expect(unlisten).toHaveBeenCalled();
});

test("file folder and output pickers use matching one-root semantics", async () => {
  const user = userEvent.setup();
  const callbacks = props();
  openMock
    .mockResolvedValueOnce("/file.ncm")
    .mockResolvedValueOnce("/folder")
    .mockResolvedValueOnce("/output-folder");
  render(<Composer {...callbacks} selected={null} />);
  await user.click(screen.getByRole("button", { name: "Choose file" }));
  await user.click(screen.getByRole("button", { name: "Choose folder" }));
  await user.click(screen.getByRole("button", { name: "Choose output" }));
  expect(openMock).toHaveBeenNthCalledWith(1, { directory: false, multiple: false });
  expect(openMock).toHaveBeenNthCalledWith(2, { directory: true, multiple: false });
  expect(openMock).toHaveBeenNthCalledWith(3, { directory: true, multiple: false });
  expect(callbacks.onSelectPath).toHaveBeenNthCalledWith(1, "/file.ncm");
  expect(callbacks.onSelectPath).toHaveBeenNthCalledWith(2, "/folder");
  expect(callbacks.onOutputPath).toHaveBeenCalledWith("/output-folder");
});

test("keyboard activation, validation errors, busy state, and directory-only copy setting are accessible", async () => {
  const user = userEvent.setup();
  const callbacks = props();
  openMock.mockRejectedValueOnce(new Error("dialog denied"));
  const { rerender } = render(
    <Composer
      {...callbacks}
      selected={{ path: "/track.ncm", kind: "file", defaultOutputPath: "/" }}
    />,
  );
  const file = screen.getByRole("button", { name: "Choose file" });
  file.focus();
  await user.keyboard("{Enter}");
  expect(callbacks.onError).toHaveBeenCalledWith(expect.stringContaining("dialog denied"));
  expect(screen.queryByText("Copy companion files")).toBeNull();
  rerender(
    <Composer
      {...callbacks}
      selected={{ path: "/music", kind: "directory", defaultOutputPath: "/music" }}
    />,
  );
  expect(screen.getByText("Copy companion files")).toBeTruthy();
  rerender(
    <Composer
      {...callbacks}
      busy
      selected={{ path: "/music", kind: "directory", defaultOutputPath: "/music" }}
    />,
  );
  expect((screen.getByRole("button", { name: "Choose file" }) as HTMLButtonElement).disabled).toBe(
    true,
  );
  expect(
    (screen.getByRole("button", { name: "Conversion in progress" }) as HTMLButtonElement).disabled,
  ).toBe(true);
  expect(screen.queryByText("Atomically replace existing files")).toBeNull();
});
