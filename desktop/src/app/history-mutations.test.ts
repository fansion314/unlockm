import { expect, test, vi } from "vite-plus/test";
import { clearHistoryRecords, deleteHistoryRun, rerunHistoryRun } from "./history-mutations";

test("releases matching run before deleting its record", async () => {
  const order: string[] = [];
  const controller = {
    releaseForRun: vi.fn((id: number) => {
      order.push(`release:${id}`);
      return null;
    }),
    releaseAll: vi.fn(() => null),
  };
  await deleteHistoryRun(controller, 7, async (id) => {
    order.push(`delete:${id}`);
  });
  expect(order).toEqual(["release:7", "delete:7"]);
});

test("releases all media before clearing history", async () => {
  const order: string[] = [];
  const controller = {
    releaseForRun: vi.fn(() => null),
    releaseAll: vi.fn(() => {
      order.push("release");
      return null;
    }),
  };
  await clearHistoryRecords(controller, async () => {
    order.push("clear");
  });
  expect(order).toEqual(["release", "clear"]);
});

test("releases the matching run before same-ID rerun and does not restore on failure", async () => {
  const order: string[] = [];
  const controller = {
    releaseForRun: vi.fn((id: number) => {
      order.push(`release:${id}`);
      return null;
    }),
    releaseAll: vi.fn(() => null),
  };
  await expect(
    rerunHistoryRun(controller, 7, async () => {
      order.push("rerun");
      throw new Error("scan failed");
    }),
  ).rejects.toThrow("scan failed");
  expect(order).toEqual(["release:7", "rerun"]);
  expect(controller.releaseAll).not.toHaveBeenCalled();
});
