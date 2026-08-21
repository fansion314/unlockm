import { Channel, invoke } from "@tauri-apps/api/core";
import type {
  BootstrapResponse,
  ConversionEvent,
  ItemPage,
  Preferences,
  RunPage,
  RunSummary,
  StartConversionRequest,
  ValidatedInput,
} from "./contracts";

export type DesktopApi = {
  bootstrap(): Promise<BootstrapResponse>;
  validateInput(path: string): Promise<ValidatedInput>;
  listRuns(cursor?: number | null, limit?: number): Promise<RunPage>;
  getRunItems(runId: number, cursor?: number | null, limit?: number): Promise<ItemPage>;
  startConversion(
    request: StartConversionRequest,
    onEvent: (event: ConversionEvent) => void,
  ): Promise<RunSummary>;
  cancelConversion(runId: number): Promise<boolean>;
  getPreferences(): Promise<Preferences>;
  setPreferences(preferences: Preferences): Promise<Preferences>;
  deleteRun(runId: number): Promise<boolean>;
  clearHistory(): Promise<void>;
  getArtwork(artworkId: number): Promise<ArrayBuffer>;
  getArtworkMime(artworkId: number): Promise<{ id: number; mimeType: string }>;
};

export const desktopApi: DesktopApi = {
  bootstrap: () => invoke<BootstrapResponse>("bootstrap"),
  validateInput: (path) => invoke<ValidatedInput>("validate_input", { path }),
  listRuns: (cursor, limit) => invoke<RunPage>("list_runs", { cursor: cursor ?? null, limit }),
  getRunItems: (runId, cursor, limit) =>
    invoke<ItemPage>("get_run_items", { runId, cursor: cursor ?? null, limit }),
  startConversion: (request, onEvent) => {
    const channel = new Channel<ConversionEvent>();
    channel.onmessage = onEvent;
    return invoke<RunSummary>("start_conversion", { request, onEvent: channel });
  },
  cancelConversion: (runId) => invoke<boolean>("cancel_conversion", { runId }),
  getPreferences: () => invoke<Preferences>("get_preferences"),
  setPreferences: (preferences) => invoke<Preferences>("set_preferences", { preferences }),
  deleteRun: (runId) => invoke<boolean>("delete_run", { runId }),
  clearHistory: () => invoke<void>("clear_history"),
  getArtwork: (artworkId) => invoke<ArrayBuffer>("get_artwork", { artworkId }),
  getArtworkMime: (artworkId) =>
    invoke<{ id: number; mimeType: string }>("get_artwork_mime", { artworkId }),
};
