import { expect, test } from "vite-plus/test";
import { appReducer, initialAppModel } from "./conversion-state";

test("ignores stale and duplicate terminal conversion events", () => {
  const planned = appReducer(initialAppModel, {
    type: "conversionEvent",
    event: { type: "planningStarted", runId: 7, total: 2 },
  });
  const terminal = {
    type: "itemTerminal" as const,
    runId: 7,
    report: {
      ordinal: 0,
      source: "/input/a.ncm",
      relativePath: "a.ncm",
      destination: "/output/a.flac",
      itemKind: "music" as const,
      action: "convert" as const,
      logicalBytes: 1,
      outcome: "converted" as const,
      error: null,
      warnings: [],
      music: null,
      timings: {},
    },
  };
  const once = appReducer(planned, { type: "conversionEvent", event: terminal });
  const twice = appReducer(once, { type: "conversionEvent", event: terminal });
  const stale = appReducer(twice, {
    type: "conversionEvent",
    event: {
      type: "itemProcessing",
      runId: 8,
      ordinal: 0,
      source: "/other.ncm",
      relativePath: "other.ncm",
      destination: "/other.flac",
      itemKind: "music",
      action: "convert",
    },
  });

  expect(once.active?.completed).toBe(1);
  expect(twice.active?.completed).toBe(1);
  expect(stale.active?.runId).toBe(7);
});

test("keeps live conversion reports bounded for large directories", () => {
  let state = appReducer(initialAppModel, {
    type: "conversionEvent",
    event: { type: "planningStarted", runId: 11, total: 10_000 },
  });
  for (let ordinal = 0; ordinal < 10_000; ordinal += 1) {
    state = appReducer(state, {
      type: "conversionEvent",
      event: {
        type: "itemTerminal",
        runId: 11,
        report: {
          ordinal,
          source: `/input/${ordinal}.ncm`,
          relativePath: `${ordinal}.ncm`,
          destination: `/output/${ordinal}.flac`,
          itemKind: "music",
          action: "convert",
          logicalBytes: 1,
          outcome: "converted",
          error: null,
          warnings: [],
          music: null,
          timings: {},
        },
      },
    });
  }
  expect(state.active?.completed).toBe(10_000);
  expect(state.active?.recentReports).toHaveLength(8);
  expect(state.active?.recentReports[0]?.ordinal).toBe(9_992);
});

test("late busy rejection cannot clear the current conversion request", () => {
  const started = appReducer(initialAppModel, { type: "conversionStarted", requestId: 101 });
  const planned = appReducer(started, {
    type: "conversionEvent",
    event: { type: "planningStarted", runId: 42, total: 3 },
  });
  const staleFailure = appReducer(planned, {
    type: "conversionFailed",
    requestId: 100,
    message: "Another conversion is already active.",
    detail: "busy",
  });
  expect(staleFailure.active?.runId).toBe(42);
  expect(staleFailure.error).toBeNull();
});

test("late bootstrap preserves a locally owned conversion request", () => {
  const started = appReducer(initialAppModel, { type: "conversionStarted", requestId: 101 });
  const planned = appReducer(started, {
    type: "conversionEvent",
    event: { type: "planningStarted", runId: 42, total: 3 },
  });
  const booted = appReducer(planned, {
    type: "booted",
    bootstrap: {
      productName: "UnlockM",
      supportedLocales: ["zh-CN", "en-US"],
      preferences: {
        locale: "en-US",
        defaultSettings: { updateMetadata: false, copyPassthrough: true, overwrite: false },
      },
      history: { runs: [], nextCursor: null },
      activeRun: { runId: 999 },
    },
  });
  expect(booted.active?.requestId).toBe(101);
  expect(booted.active?.runId).toBe(42);
});

test("reconciles a reload-recovered active run once bootstrap shows it terminal", () => {
  const recovered = appReducer(initialAppModel, {
    type: "booted",
    bootstrap: {
      productName: "UnlockM",
      supportedLocales: ["zh-CN", "en-US"],
      preferences: {
        locale: "en-US",
        defaultSettings: { updateMetadata: false, copyPassthrough: true, overwrite: false },
      },
      history: { runs: [], nextCursor: null },
      activeRun: { runId: 77 },
    },
  });
  const reconciled = appReducer(recovered, {
    type: "booted",
    bootstrap: {
      productName: "UnlockM",
      supportedLocales: ["zh-CN", "en-US"],
      preferences: {
        locale: "en-US",
        defaultSettings: { updateMetadata: false, copyPassthrough: true, overwrite: false },
      },
      history: { runs: [], nextCursor: null },
      activeRun: null,
    },
  });
  expect(reconciled.active).toBeNull();
});
