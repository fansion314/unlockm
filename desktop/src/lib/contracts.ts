export type RunSettings = {
  updateMetadata: boolean;
  copyPassthrough: boolean;
  overwrite: boolean;
};

export type Preferences = {
  locale: "en-US" | "zh-CN" | "ja-JP";
  theme: "light" | "dark";
  palette?: "blue" | "emerald" | "violet" | "amber";
  playerVolume?: number;
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
  displayName?: string;
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
  musicCount: number;
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
  supportedLocales: ["zh-CN", "en-US", "ja-JP"];
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
  outputBytes: number | null;
  outputFormat: string | null;
  bitrateKbps?: number | null;
  mediaProbeVersion?: number;
  metadataEditPending?: boolean;
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

export type MetadataPatch = {
  title?: string | null;
  artist?: string | null;
  album?: string | null;
  cover?: Uint8Array;
  existingArtworkId?: number;
};

export type ArtworkCandidate = {
  artworkId: number;
  album: string | null;
  artist: string | null;
  title: string | null;
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
  outputBytes: number | null;
  outputFormat: string | null;
  errorCode: string | null;
  errorDetail: string | null;
  warnings: ReportWarning[];
  title: string | null;
  artist: string | null;
  album: string | null;
  durationMs: number | null;
  artworkId: number | null;
  bitrateKbps?: number | null;
  mediaProbeVersion?: number;
  metadataEditPending?: boolean;
};

export type ItemPage = {
  items: HistoryItem[];
  page: number;
  pageSize: number;
  pageCount: number;
  filteredTotal: number;
  companionCount: number;
  musicTotal: number;
  repairPending?: boolean;
};

export type ItemSort = "ordinal" | "title" | "artist" | "album" | "size";
export type SortDirection = "asc" | "desc";
export type OutcomeFilter =
  | "converted"
  | "copied"
  | "skipped"
  | "conflict"
  | "failed"
  | "cancelled";

export type ItemQuery = {
  page: number;
  pageSize: number;
  search: string | null;
  artist: string | null;
  album: string | null;
  outcome: OutcomeFilter | null;
  sort: ItemSort;
  direction: SortDirection;
};

export type FacetKind = "artist" | "album";

export type FacetPage = {
  values: string[];
  nextCursor: string | null;
};
