export type RunSettings = {
  updateMetadata: boolean;
  copyPassthrough: boolean;
  overwrite: boolean;
};

export type Preferences = {
  locale: "en-US" | "zh-CN";
  defaultSettings: RunSettings;
};

export type RunStatus =
  | "planning"
  | "running"
  | "completed"
  | "partial"
  | "failed"
  | "cancelled"
  | "interrupted";

export type RunSummary = {
  id: number;
  inputKind: "file" | "directory";
  inputPath: string;
  outputPath: string;
  settings: RunSettings;
  status: RunStatus;
  createdAtMs: number;
  updatedAtMs: number;
  finishedAtMs: number | null;
  totalItems: number;
  completedItems: number;
  converted: number;
  copied: number;
  skipped: number;
  conflicted: number;
  failed: number;
  representativeArtworkId: number | null;
  terminalErrorCode: string | null;
  terminalErrorDetail: string | null;
};

export type RunPage = {
  runs: RunSummary[];
  nextCursor: number | null;
};

export type BootstrapResponse = {
  productName: "UnlockM";
  supportedLocales: ["zh-CN", "en-US"];
  preferences: Preferences;
  history: RunPage;
  activeRun: { runId: number } | null;
};

export type InputKind = "file" | "directory";
export type ItemKind = "music" | "companion";
export type ItemAction = "convert" | "copy";
export type JobOutcome = "converted" | "copied" | "skipped" | "conflict" | "failed" | "cancelled";

export type ReportWarning = {
  code: string;
  detail: string;
};

export type MusicSnapshot = {
  title: string | null;
  artist: string | null;
  album: string | null;
  durationMs: number | null;
  artwork: null;
};

export type JobReport = {
  ordinal: number;
  source: string;
  relativePath: string;
  destination: string | null;
  itemKind: ItemKind;
  action: ItemAction | null;
  logicalBytes: number;
  outcome: JobOutcome;
  error: { code: string; detail: string } | null;
  warnings: ReportWarning[];
  music: MusicSnapshot | null;
  timings: Record<string, { secs: number; nanos: number }>;
};

export type ConversionEvent =
  | { type: "planningStarted"; runId: number; total: number }
  | {
      type: "itemPlanning";
      runId: number;
      ordinal: number;
      source: string;
      relativePath: string;
      itemKind: ItemKind;
    }
  | {
      type: "itemProcessing";
      runId: number;
      ordinal: number;
      source: string;
      relativePath: string;
      destination: string;
      itemKind: ItemKind;
      action: ItemAction;
    }
  | { type: "itemTerminal"; runId: number; report: JobReport }
  | {
      type: "completed";
      runId: number;
      summary: {
        converted: number;
        copied: number;
        skipped: number;
        conflicted: number;
        failed: number;
      };
      cancelled: boolean;
    };

export type ValidatedInput = {
  path: string;
  kind: InputKind;
  defaultOutputPath: string;
};

export type StartConversionRequest = {
  inputPath: string;
  outputPath: string | null;
  settings: RunSettings;
};

export type HistoryItem = {
  id: number;
  runId: number;
  ordinal: number;
  itemKind: ItemKind;
  sourcePath: string;
  relativePath: string;
  destinationPath: string | null;
  action: string | null;
  outcome: string;
  logicalBytes: number;
  errorCode: string | null;
  errorDetail: string | null;
  warnings: ReportWarning[];
  title: string | null;
  artist: string | null;
  album: string | null;
  durationMs: number | null;
  artworkId: number | null;
};

export type ItemPage = {
  items: HistoryItem[];
  nextCursor: number | null;
  companionCount: number;
  musicTotal: number;
};
