import type {
  BootstrapResponse,
  ConversionEvent,
  JobReport,
  RunPage,
  RunSummary,
} from "../lib/contracts";

export type ActiveConversion = {
  runId: number | null;
  requestId: number | null;
  total: number;
  completed: number;
  recentReports: JobReport[];
  phase: "planning" | "running" | "terminal";
};

export type AppModel = {
  boot: "loading" | "ready" | "error";
  bootstrap: BootstrapResponse | null;
  active: ActiveConversion | null;
  latestTerminal: RunSummary | null;
  error: { message: string; detail: string | null } | null;
};

export const initialAppModel: AppModel = {
  boot: "loading",
  bootstrap: null,
  active: null,
  latestTerminal: null,
  error: null,
};

export type AppAction =
  | { type: "booted"; bootstrap: BootstrapResponse }
  | { type: "bootFailed"; message: string; detail: string | null }
  | { type: "uiError"; message: string; detail: string | null }
  | { type: "runsAppended"; page: RunPage }
  | { type: "conversionStarted"; requestId: number }
  | { type: "conversionEvent"; event: ConversionEvent }
  | { type: "conversionResolved"; requestId: number; run: RunSummary }
  | { type: "conversionFailed"; requestId: number; message: string; detail: string | null };

export function appReducer(state: AppModel, action: AppAction): AppModel {
  switch (action.type) {
    case "booted":
      // A local request owns the active slot until its matching promise/event
      // resolves. A late bootstrap response must refresh history without
      // replacing that request ID or dropping its IPC lifecycle.
      if (state.active?.requestId !== null && state.active?.requestId !== undefined) {
        return { ...state, boot: "ready", bootstrap: action.bootstrap, error: null };
      }
      return {
        ...state,
        boot: "ready",
        bootstrap: action.bootstrap,
        active: action.bootstrap.activeRun
          ? {
              runId: action.bootstrap.activeRun.runId,
              requestId: null,
              total: 0,
              completed: 0,
              recentReports: [],
              phase: "running",
            }
          : null,
        error: null,
      };
    case "bootFailed":
      return { ...state, boot: "error", error: { message: action.message, detail: action.detail } };
    case "uiError":
      return { ...state, error: { message: action.message, detail: action.detail } };
    case "runsAppended":
      if (!state.bootstrap) {
        return state;
      }
      return {
        ...state,
        bootstrap: {
          ...state.bootstrap,
          history: {
            runs: mergeRuns(state.bootstrap.history.runs, action.page.runs),
            nextCursor: action.page.nextCursor,
          },
        },
      };
    case "conversionStarted":
      return {
        ...state,
        active: {
          runId: null,
          requestId: action.requestId,
          total: 0,
          completed: 0,
          recentReports: [],
          phase: "planning",
        },
        error: null,
      };
    case "conversionEvent":
      return reduceConversionEvent(state, action.event);
    case "conversionResolved":
      return state.active?.requestId === action.requestId
        ? { ...state, active: null, latestTerminal: action.run, error: null }
        : state;
    case "conversionFailed":
      return state.active?.requestId === action.requestId
        ? { ...state, active: null, error: { message: action.message, detail: action.detail } }
        : state;
  }
}

function reduceConversionEvent(state: AppModel, event: ConversionEvent): AppModel {
  if (event.type === "planningStarted") {
    if (state.active && state.active.runId !== null && state.active.runId !== event.runId) {
      return state;
    }
    return {
      ...state,
      active: {
        runId: event.runId,
        requestId: state.active?.requestId ?? null,
        total: event.total,
        completed: 0,
        recentReports: [],
        phase: "planning",
      },
    };
  }
  if (!state.active || state.active.runId !== event.runId) {
    return state;
  }
  if (event.type === "itemPlanning" || event.type === "itemProcessing") {
    return { ...state, active: { ...state.active, phase: "running" } };
  }
  if (event.type === "itemTerminal") {
    if (state.active.recentReports.some((report) => report.ordinal === event.report.ordinal)) {
      return state;
    }
    const recentReports = [...state.active.recentReports, event.report].slice(-8);
    return {
      ...state,
      active: {
        ...state.active,
        completed: state.active.completed + 1,
        recentReports,
        phase: "running",
      },
    };
  }
  return { ...state, active: { ...state.active, phase: "terminal" } };
}

function mergeRuns(existing: RunSummary[], incoming: RunSummary[]): RunSummary[] {
  const byId = new Map(existing.map((run) => [run.id, run]));
  for (const run of incoming) {
    byId.set(run.id, run);
  }
  return [...byId.values()].sort((left, right) => right.id - left.id);
}
