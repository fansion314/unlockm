import { Channel, invoke } from "@tauri-apps/api/core";
import type {
  BootstrapResponse,
  ConversionEvent,
  FacetKind,
  FacetPage,
  ItemQuery,
  ItemPage,
  Preferences,
  MetadataPatch,
  ArtworkCandidate,
  RunPage,
  RunSummary,
  StartConversionRequest,
  ValidatedInput,
} from "./contracts";

export type DesktopApi = {
  bootstrap(): Promise<BootstrapResponse>;
  validateInput(path: string): Promise<ValidatedInput>;
  listRuns(cursor?: number | null, limit?: number): Promise<RunPage>;
  queryRunItems(runId: number, query: ItemQuery): Promise<ItemPage>;
  listRunItemFacets(
    runId: number,
    facet: FacetKind,
    search?: string | null,
    cursor?: string | null,
    limit?: number,
  ): Promise<FacetPage>;
  startConversion(
    request: StartConversionRequest,
    onEvent: (event: ConversionEvent) => void,
  ): Promise<RunSummary>;
  rerunConversion(runId: number, onEvent: (event: ConversionEvent) => void): Promise<RunSummary>;
  cancelConversion(runId: number): Promise<boolean>;
  getPreferences(): Promise<Preferences>;
  setPreferences(preferences: Preferences): Promise<Preferences>;
  deleteRun(runId: number): Promise<boolean>;
  renameRun(runId: number, displayName: string): Promise<RunSummary | null>;
  updateRunItemMetadata(
    itemId: number,
    patch: MetadataPatch,
  ): Promise<import("./contracts").HistoryItem>;
  listArtworkCandidates(
    itemId: number,
    query?: string | null,
    limit?: number,
  ): Promise<ArtworkCandidate[]>;
  clearHistory(): Promise<void>;
  revealRunOutput(runId: number): Promise<boolean>;
  revealRunItem(itemId: number): Promise<boolean>;
  openRunItem(itemId: number): Promise<boolean>;
  getArtwork(artworkId: number): Promise<ArrayBuffer>;
  getArtworkMime(artworkId: number): Promise<{ id: number; mimeType: string }>;
};

export const desktopApi: DesktopApi = {
  bootstrap: () => invoke<BootstrapResponse>("bootstrap"),
  validateInput: (path) => invoke<ValidatedInput>("validate_input", { path }),
  listRuns: (cursor, limit) => invoke<RunPage>("list_runs", { cursor: cursor ?? null, limit }),
  queryRunItems: (runId, query) => invoke<ItemPage>("query_run_items", { runId, query }),
  listRunItemFacets: (runId, facet, search, cursor, limit) =>
    invoke<FacetPage>("list_run_item_facets", {
      runId,
      facet,
      search: search ?? null,
      cursor: cursor ?? null,
      limit,
    }),
  startConversion: (request, onEvent) => {
    const channel = new Channel<ConversionEvent>();
    channel.onmessage = onEvent;
    return invoke<RunSummary>("start_conversion", { request, onEvent: channel });
  },
  rerunConversion: (runId, onEvent) => {
    const channel = new Channel<ConversionEvent>();
    channel.onmessage = onEvent;
    return invoke<RunSummary>("rerun_conversion", { runId, onEvent: channel });
  },
  cancelConversion: (runId) => invoke<boolean>("cancel_conversion", { runId }),
  getPreferences: () => invoke<Preferences>("get_preferences"),
  setPreferences: (preferences) => invoke<Preferences>("set_preferences", { preferences }),
  deleteRun: (runId) => invoke<boolean>("delete_run", { runId }),
  renameRun: (runId, displayName) =>
    invoke<RunSummary | null>("rename_run", { runId, displayName }),
  updateRunItemMetadata: (itemId, patch) =>
    invoke<import("./contracts").HistoryItem>("update_run_item_metadata", { itemId, patch }),
  listArtworkCandidates: (itemId, query, limit) =>
    invoke<ArtworkCandidate[]>("list_artwork_candidates", { itemId, query: query ?? null, limit }),
  clearHistory: () => invoke<void>("clear_history"),
  revealRunOutput: (runId) => invoke<boolean>("reveal_run_output", { runId }),
  revealRunItem: (itemId) => invoke<boolean>("reveal_run_item", { itemId }),
  openRunItem: (itemId) => invoke<boolean>("open_run_item", { itemId }),
  getArtwork: (artworkId) => invoke<ArrayBuffer>("get_artwork", { artworkId }),
  getArtworkMime: (artworkId) =>
    invoke<{ id: number; mimeType: string }>("get_artwork_mime", { artworkId }),
};
