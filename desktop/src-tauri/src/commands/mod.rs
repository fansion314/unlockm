pub mod online_metadata;

use crate::{
    PRODUCT_NAME,
    history::{
        ArtworkInput, ArtworkRecord, CreateRun, FacetKind, FacetPage, InputKind, ItemPage,
        ItemQuery, PersistedItemInput, Preferences, RunPage, RunSettings, RunStatus,
        StoredItemKind, StoredWarning,
    },
    state::AppState,
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        mpsc::{Receiver, SyncSender, sync_channel},
    },
    thread::JoinHandle,
};
use tauri::{
    AppHandle, State,
    ipc::{Channel, Response},
};
use tauri_plugin_opener::OpenerExt;
use unlockm::{
    app::{
        self, JobOutcome, JobReport, Summary,
        events::{BatchEvent, BatchObserver, ItemAction, ItemKind},
    },
    config::{OutputLayout, RunOptions},
    metadata::tags::MetadataPatch,
};

pub type CommandResult<T> = Result<T, CommandError>;
const SUPPORTED_LOCALES: [&str; 3] = ["zh-CN", "en-US", "ja-JP"];

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandError {
    pub code: &'static str,
    pub detail: String,
}

impl CommandError {
    fn history(error: impl ToString) -> Self {
        Self {
            code: "history_error",
            detail: error.to_string(),
        }
    }

    fn invalid_input(detail: impl Into<String>) -> Self {
        Self {
            code: "invalid_input",
            detail: detail.into(),
        }
    }

    fn state(error: crate::state::StateError) -> Self {
        Self {
            code: if matches!(error, crate::state::StateError::Busy { .. }) {
                "active_run"
            } else {
                "state_error"
            },
            detail: error.to_string(),
        }
    }

    fn output_unavailable(detail: impl Into<String>) -> Self {
        Self {
            code: "output_unavailable",
            detail: detail.into(),
        }
    }
}

async fn blocking<T: Send + 'static>(
    task: impl FnOnce() -> CommandResult<T> + Send + 'static,
) -> CommandResult<T> {
    tauri::async_runtime::spawn_blocking(task)
        .await
        .map_err(|error| CommandError {
            code: "desktop_blocking_task_failed",
            detail: error.to_string(),
        })?
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapResponse {
    pub product_name: &'static str,
    pub supported_locales: [&'static str; 3],
    pub preferences: Preferences,
    pub history: RunPage,
    pub active_run: Option<ActiveRunResponse>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveRunResponse {
    pub run_id: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidatedInput {
    pub path: PathBuf,
    pub kind: String,
    pub default_output_path: PathBuf,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtworkMimeResponse {
    pub id: i64,
    pub mime_type: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartConversionRequest {
    pub input_path: PathBuf,
    pub output_path: Option<PathBuf>,
    pub settings: RunSettings,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetadataPatchRequest {
    pub title: Option<Option<String>>,
    pub artist: Option<Option<String>>,
    pub album: Option<Option<String>>,
    pub cover: Option<Vec<u8>>,
    pub existing_artwork_id: Option<i64>,
}

#[derive(Debug, Serialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ConversionEventDto {
    PlanningStarted {
        run_id: i64,
        total: usize,
    },
    ItemPlanning {
        run_id: i64,
        ordinal: u64,
        source: PathBuf,
        relative_path: PathBuf,
        item_kind: ItemKind,
    },
    ItemProcessing {
        run_id: i64,
        ordinal: u64,
        source: PathBuf,
        relative_path: PathBuf,
        destination: PathBuf,
        item_kind: ItemKind,
        action: ItemAction,
    },
    ItemTerminal {
        run_id: i64,
        report: Box<JobReport>,
    },
    Completed {
        run_id: i64,
        summary: Summary,
        cancelled: bool,
    },
}

#[tauri::command]
pub async fn bootstrap(state: State<'_, AppState>) -> CommandResult<BootstrapResponse> {
    let state = state.inner().clone();
    blocking(move || {
        let history = state.history();
        Ok(BootstrapResponse {
            product_name: PRODUCT_NAME,
            supported_locales: SUPPORTED_LOCALES,
            preferences: history.preferences().map_err(CommandError::history)?,
            history: history.list_runs(None, 30).map_err(CommandError::history)?,
            active_run: state
                .active_run()
                .map(|run| ActiveRunResponse { run_id: run.run_id }),
        })
    })
    .await
}

#[tauri::command]
pub async fn validate_input(path: PathBuf) -> CommandResult<ValidatedInput> {
    blocking(move || validate_input_path(path)).await
}

fn validate_input_path(path: PathBuf) -> CommandResult<ValidatedInput> {
    let metadata = fs::symlink_metadata(&path).map_err(|error| {
        CommandError::invalid_input(format!("read {}: {error}", path.display()))
    })?;
    if metadata.file_type().is_symlink() {
        return Err(CommandError::invalid_input(
            "symlink roots are not supported",
        ));
    }
    let path = path.canonicalize().map_err(|error| {
        CommandError::invalid_input(format!("canonicalize {}: {error}", path.display()))
    })?;
    if metadata.is_dir() {
        return Ok(ValidatedInput {
            default_output_path: path.clone(),
            path,
            kind: "directory".into(),
        });
    }
    if metadata.is_file() {
        let output = path
            .parent()
            .map(PathBuf::from)
            .ok_or_else(|| CommandError::invalid_input("file input has no parent directory"))?;
        return Ok(ValidatedInput {
            path,
            default_output_path: output,
            kind: "file".into(),
        });
    }
    Err(CommandError::invalid_input(
        "input must be a regular file or directory",
    ))
}

fn validate_output_dir(path: Option<PathBuf>) -> CommandResult<Option<PathBuf>> {
    let Some(path) = path else {
        return Ok(None);
    };
    match fs::symlink_metadata(&path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() {
                return Err(CommandError::invalid_input(
                    "output directory cannot be a symlink",
                ));
            }
            if !metadata.is_dir() {
                return Err(CommandError::invalid_input(
                    "output path must be a directory",
                ));
            }
            path.canonicalize().map(Some).map_err(|error| {
                CommandError::invalid_input(format!(
                    "canonicalize output {}: {error}",
                    path.display()
                ))
            })
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let parent = path.parent().unwrap_or_else(|| Path::new("."));
            let parent_metadata = fs::symlink_metadata(parent).map_err(|parent_error| {
                CommandError::invalid_input(format!(
                    "read output parent {}: {parent_error}",
                    parent.display()
                ))
            })?;
            if parent_metadata.file_type().is_symlink() || !parent_metadata.is_dir() {
                return Err(CommandError::invalid_input(
                    "output parent must be a non-symlink directory",
                ));
            }
            let parent = parent.canonicalize().map_err(|parent_error| {
                CommandError::invalid_input(format!(
                    "canonicalize output parent {}: {parent_error}",
                    parent.display()
                ))
            })?;
            let name = path
                .file_name()
                .ok_or_else(|| CommandError::invalid_input("output has no directory name"))?;
            Ok(Some(parent.join(name)))
        }
        Err(error) => Err(CommandError::invalid_input(format!(
            "read output {}: {error}",
            path.display()
        ))),
    }
}

#[tauri::command]
pub async fn list_runs(
    state: State<'_, AppState>,
    cursor: Option<i64>,
    limit: Option<usize>,
) -> CommandResult<RunPage> {
    let history = state.history().clone();
    blocking(move || {
        history
            .list_runs(cursor, limit.unwrap_or(30))
            .map_err(CommandError::history)
    })
    .await
}

#[tauri::command]
pub async fn query_run_items(
    state: State<'_, AppState>,
    run_id: i64,
    query: ItemQuery,
) -> CommandResult<ItemPage> {
    let history = state.history().clone();
    blocking(move || {
        history
            .query_music_items(run_id, query)
            .map_err(CommandError::history)
    })
    .await
}

#[tauri::command]
pub async fn list_run_item_facets(
    state: State<'_, AppState>,
    run_id: i64,
    facet: FacetKind,
    search: Option<String>,
    cursor: Option<String>,
    limit: Option<usize>,
) -> CommandResult<FacetPage> {
    let history = state.history().clone();
    blocking(move || {
        history
            .list_music_facets(run_id, facet, search, cursor, limit.unwrap_or(50))
            .map_err(CommandError::history)
    })
    .await
}

#[tauri::command]
pub async fn get_artwork(state: State<'_, AppState>, artwork_id: i64) -> CommandResult<Response> {
    let history = state.history().clone();
    blocking(move || {
        let ArtworkRecord { bytes, .. } = history
            .artwork(artwork_id)
            .map_err(CommandError::history)?
            .ok_or_else(|| CommandError {
                code: "artwork_not_found",
                detail: format!("artwork {artwork_id} does not exist"),
            })?;
        Ok(Response::new(bytes))
    })
    .await
}

/// Raw IPC bodies cannot carry a MIME header. The frontend asks this narrow
/// metadata command once per lazy artwork request and creates its Blob with
/// the validated stored MIME type.
#[tauri::command]
pub async fn get_artwork_mime(
    state: State<'_, AppState>,
    artwork_id: i64,
) -> CommandResult<ArtworkMimeResponse> {
    let history = state.history().clone();
    blocking(move || {
        let artwork = history
            .artwork_mime(artwork_id)
            .map_err(CommandError::history)?
            .ok_or_else(|| CommandError {
                code: "artwork_not_found",
                detail: format!("artwork {artwork_id} does not exist"),
            })?;
        Ok(ArtworkMimeResponse {
            id: artwork.0,
            mime_type: artwork.1,
        })
    })
    .await
}

#[tauri::command]
pub async fn get_preferences(state: State<'_, AppState>) -> CommandResult<Preferences> {
    let history = state.history().clone();
    blocking(move || history.preferences().map_err(CommandError::history)).await
}

#[tauri::command]
pub async fn update_run_item_metadata(
    state: State<'_, AppState>,
    item_id: i64,
    patch: MetadataPatchRequest,
) -> CommandResult<crate::history::HistoryItem> {
    let state = state.inner().clone();
    blocking(move || update_run_item_metadata_blocking(&state, item_id, patch)).await
}

fn update_run_item_metadata_blocking(
    state: &AppState,
    item_id: i64,
    patch: MetadataPatchRequest,
) -> CommandResult<crate::history::HistoryItem> {
    // This lock intentionally spans every disk and SQLite step. A conversion
    // cannot start, and another metadata/delete/clear mutation cannot observe
    // a candidate-publish/reconcile sequence halfway through.
    let mutation = state
        .begin_history_mutation()
        .map_err(CommandError::state)?;
    wait_for_metadata_mutation_barrier();
    let history = mutation.history();
    let item = history
        .editable_item(item_id)
        .map_err(CommandError::history)?
        .ok_or_else(|| CommandError::output_unavailable("recorded music output is unavailable"))?;
    let output = validated_recorded_file(item.destination_path)?;
    let patch = normalize_metadata_patch(patch)?;
    let existing_cover = match patch.existing_artwork_id {
        Some(artwork_id) => Some(
            history
                .artwork(artwork_id)
                .map_err(CommandError::history)?
                .ok_or_else(|| CommandError::invalid_input("selected artwork no longer exists"))?
                .bytes,
        ),
        None => None,
    };
    let patch = MetadataPatch {
        title: patch.title,
        artist: patch.artist,
        album: patch.album,
        cover: patch.cover.or(existing_cover),
    };
    history
        .set_metadata_edit_pending(item.id, true)
        .map_err(CommandError::history)?;
    let candidate = match unlockm::metadata::tags::write_patch(&output, &patch) {
        Ok(candidate) => candidate,
        Err(error) => {
            let _ = history.set_metadata_edit_pending(item.id, false);
            return Err(CommandError::output_unavailable(error));
        }
    };
    if let Err(error) =
        unlockm::io::atomic::AtomicOutput::publish_existing_temp(candidate, &output, true)
    {
        let _ = history.set_metadata_edit_pending(item.id, false);
        return Err(CommandError::output_unavailable(error.to_string()));
    }
    history
        .reconcile_item_from_output(item.id, &output)
        .map_err(CommandError::history)
}

#[cfg(test)]
static METADATA_MUTATION_BARRIER: std::sync::OnceLock<Mutex<Option<Arc<std::sync::Barrier>>>> =
    std::sync::OnceLock::new();

#[cfg(test)]
fn set_metadata_mutation_barrier(barrier: Option<Arc<std::sync::Barrier>>) {
    *METADATA_MUTATION_BARRIER
        .get_or_init(|| Mutex::new(None))
        .lock()
        .expect("metadata mutation barrier lock") = barrier;
}

#[cfg(test)]
fn wait_for_metadata_mutation_barrier() {
    let barrier = METADATA_MUTATION_BARRIER
        .get_or_init(|| Mutex::new(None))
        .lock()
        .expect("metadata mutation barrier lock")
        .clone();
    if let Some(barrier) = barrier {
        barrier.wait();
        barrier.wait();
    }
}

#[cfg(not(test))]
fn wait_for_metadata_mutation_barrier() {}

fn validate_metadata_patch(patch: &MetadataPatchRequest) -> CommandResult<()> {
    for value in [&patch.title, &patch.artist, &patch.album] {
        if value
            .as_ref()
            .and_then(|value| value.as_ref())
            .is_some_and(|value| value.trim().chars().count() > 512)
        {
            return Err(CommandError::invalid_input(
                "metadata fields are limited to 512 characters",
            ));
        }
    }
    if patch
        .cover
        .as_ref()
        .is_some_and(|bytes| bytes.len() > unlockm::metadata::tags::MAX_COVER_BYTES)
    {
        return Err(CommandError::invalid_input("cover exceeds 10 MiB"));
    }
    if patch
        .cover
        .as_deref()
        .is_some_and(|bytes| artwork_mime(bytes).is_none())
    {
        return Err(CommandError::invalid_input(
            "cover is not a supported image type",
        ));
    }
    Ok(())
}

fn normalize_metadata_patch(
    mut patch: MetadataPatchRequest,
) -> CommandResult<MetadataPatchRequest> {
    for value in [&mut patch.title, &mut patch.artist, &mut patch.album] {
        if let Some(Some(text)) = value {
            let normalized = text.trim().to_owned();
            *value = Some((!normalized.is_empty()).then_some(normalized));
        }
    }
    validate_metadata_patch(&patch)?;
    Ok(patch)
}

fn artwork_mime(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if bytes.starts_with(b"\xff\xd8\xff") {
        Some("image/jpeg")
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        Some("image/webp")
    } else {
        None
    }
}

#[tauri::command]
pub async fn list_artwork_candidates(
    state: State<'_, AppState>,
    item_id: i64,
    query: Option<String>,
    limit: usize,
) -> CommandResult<Vec<crate::history::ArtworkCandidate>> {
    let history = state.history().clone();
    blocking(move || {
        history
            .artwork_candidates(item_id, query, limit)
            .map_err(CommandError::history)
    })
    .await
}

#[tauri::command]
pub async fn set_preferences(
    state: State<'_, AppState>,
    preferences: Preferences,
) -> CommandResult<Preferences> {
    let history = state.history().clone();
    blocking(move || {
        history
            .set_preferences(&preferences)
            .map_err(CommandError::history)
    })
    .await
}

fn validated_recorded_directory(path: PathBuf) -> CommandResult<PathBuf> {
    let metadata = fs::symlink_metadata(&path).map_err(|error| {
        CommandError::output_unavailable(format!("read {}: {error}", path.display()))
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(CommandError::output_unavailable(
            "recorded output is not a regular directory",
        ));
    }
    path.canonicalize().map_err(|error| {
        CommandError::output_unavailable(format!("canonicalize {}: {error}", path.display()))
    })
}

fn validated_recorded_file(path: PathBuf) -> CommandResult<PathBuf> {
    let metadata = fs::symlink_metadata(&path).map_err(|error| {
        CommandError::output_unavailable(format!("read {}: {error}", path.display()))
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(CommandError::output_unavailable(
            "recorded output is not a regular file",
        ));
    }
    path.canonicalize().map_err(|error| {
        CommandError::output_unavailable(format!("canonicalize {}: {error}", path.display()))
    })
}

fn invoke_native_action(
    path: &Path,
    action: impl FnOnce(&Path) -> Result<(), String>,
) -> CommandResult<bool> {
    action(path).map_err(CommandError::output_unavailable)?;
    Ok(true)
}

#[tauri::command]
pub async fn reveal_run_output(
    app: AppHandle,
    state: State<'_, AppState>,
    run_id: i64,
) -> CommandResult<bool> {
    let history = state.history().clone();
    let path = blocking(move || {
        let path = history
            .run_output_path(run_id)
            .map_err(CommandError::history)?
            .ok_or_else(|| CommandError::output_unavailable("run output does not exist"))?;
        validated_recorded_directory(path)
    })
    .await?;
    invoke_native_action(&path, |path| {
        app.opener()
            .open_path(path.to_string_lossy(), None::<String>)
            .map_err(|error| error.to_string())
    })
}

#[tauri::command]
pub async fn reveal_run_item(
    app: AppHandle,
    state: State<'_, AppState>,
    item_id: i64,
) -> CommandResult<bool> {
    let history = state.history().clone();
    let path = blocking(move || {
        let path = history
            .music_output_path(item_id)
            .map_err(CommandError::history)?
            .ok_or_else(|| CommandError::output_unavailable("music output does not exist"))?;
        validated_recorded_file(path)
    })
    .await?;
    invoke_native_action(&path, |path| {
        app.opener()
            .reveal_item_in_dir(path)
            .map_err(|error| error.to_string())
    })
}

#[tauri::command]
pub async fn open_run_item(
    app: AppHandle,
    state: State<'_, AppState>,
    item_id: i64,
) -> CommandResult<bool> {
    let history = state.history().clone();
    let path = blocking(move || {
        let path = history
            .music_output_path(item_id)
            .map_err(CommandError::history)?
            .ok_or_else(|| CommandError::output_unavailable("music output does not exist"))?;
        validated_recorded_file(path)
    })
    .await?;
    invoke_native_action(&path, |path| {
        app.opener()
            .open_path(path.to_string_lossy(), None::<String>)
            .map_err(|error| error.to_string())
    })
}

#[tauri::command]
pub async fn delete_run(state: State<'_, AppState>, run_id: i64) -> CommandResult<bool> {
    let state = state.inner().clone();
    blocking(move || {
        state
            .delete_history_run(run_id)
            .map_err(CommandError::state)
    })
    .await
}

#[tauri::command]
pub async fn rename_run(
    state: State<'_, AppState>,
    run_id: i64,
    display_name: String,
) -> CommandResult<Option<crate::history::RunSummary>> {
    let state = state.inner().clone();
    blocking(move || {
        state
            .history()
            .rename_run(run_id, &display_name)
            .map_err(CommandError::history)
    })
    .await
}

#[tauri::command]
pub async fn clear_history(state: State<'_, AppState>) -> CommandResult<()> {
    let state = state.inner().clone();
    blocking(move || state.clear_history().map_err(CommandError::state)).await
}

enum CoordinatorMessage {
    Terminal {
        report: Box<JobReport>,
        reply: SyncSender<Result<Box<JobReport>, String>>,
    },
    Stop,
}

struct PersistenceCoordinator {
    sender: SyncSender<CoordinatorMessage>,
    worker: Option<JoinHandle<()>>,
}

impl PersistenceCoordinator {
    fn start(history: Arc<crate::history::HistoryStore>, run_id: i64, capacity: usize) -> Self {
        let (sender, receiver) = sync_channel(capacity.max(1));
        let worker = std::thread::spawn(move || coordinator_loop(history, run_id, receiver));
        Self {
            sender,
            worker: Some(worker),
        }
    }

    fn shutdown(&mut self) {
        let _ = self.sender.send(CoordinatorMessage::Stop);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl Drop for PersistenceCoordinator {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn persist_with_sender(
    sender: &SyncSender<CoordinatorMessage>,
    report: Box<JobReport>,
) -> Result<Box<JobReport>, String> {
    let (reply_sender, reply_receiver) = sync_channel(0);
    sender
        .send(CoordinatorMessage::Terminal {
            report,
            reply: reply_sender,
        })
        .map_err(|_| "history coordinator stopped unexpectedly".to_string())?;
    reply_receiver
        .recv()
        .map_err(|_| "history coordinator dropped terminal acknowledgement".to_string())?
}

fn handle_terminal_persistence(
    sender: &SyncSender<CoordinatorMessage>,
    report: Box<JobReport>,
    cancellation: &unlockm::app::runner::Cancellation,
    send_committed_event: impl FnOnce(Box<JobReport>),
) -> Result<(), String> {
    match persist_with_sender(sender, report) {
        Ok(report) => {
            send_committed_event(report);
            Ok(())
        }
        Err(error) => {
            cancellation.cancel();
            Err(error)
        }
    }
}

fn coordinator_loop(
    history: Arc<crate::history::HistoryStore>,
    run_id: i64,
    receiver: Receiver<CoordinatorMessage>,
) {
    while let Ok(message) = receiver.recv() {
        match message {
            CoordinatorMessage::Terminal { mut report, reply } => {
                let result = history
                    .record_item(run_id, item_for_history(&report))
                    .map_err(|error| error.to_string())
                    .map(|_| {
                        if let Some(music) = report.music.as_mut() {
                            music.artwork = None;
                        }
                        report
                    });
                let _ = reply.send(result);
            }
            CoordinatorMessage::Stop => break,
        }
    }
}

#[tauri::command]
pub async fn start_conversion(
    state: State<'_, AppState>,
    request: StartConversionRequest,
    on_event: Channel<ConversionEventDto>,
) -> CommandResult<crate::history::RunSummary> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        start_conversion_blocking(state, request, on_event)
    })
    .await
    .map_err(|error| CommandError {
        code: "conversion_coordinator_join_failed",
        detail: error.to_string(),
    })?
}

/// Replays a terminal run's persisted request through the exact same bounded
/// conversion entrypoint. Paths and settings are never accepted from the
/// webview for this operation.
#[tauri::command]
pub async fn rerun_conversion(
    state: State<'_, AppState>,
    run_id: i64,
    on_event: Channel<ConversionEventDto>,
) -> CommandResult<crate::history::RunSummary> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let request = rerun_snapshot(state.history(), run_id)?;
        start_conversion_blocking_for(state, request, on_event, Some(run_id))
    })
    .await
    .map_err(|error| CommandError {
        code: "conversion_coordinator_join_failed",
        detail: error.to_string(),
    })?
}

fn rerun_snapshot(
    history: &crate::history::HistoryStore,
    run_id: i64,
) -> CommandResult<StartConversionRequest> {
    let previous = history
        .run(run_id)
        .map_err(CommandError::history)?
        .ok_or_else(|| CommandError::invalid_input("recorded conversion no longer exists"))?;
    if matches!(previous.status, RunStatus::Planning | RunStatus::Running) {
        return Err(CommandError::state(crate::state::StateError::NotActive {
            run_id,
        }));
    }
    Ok(StartConversionRequest {
        input_path: previous.input_path,
        output_path: Some(previous.output_path),
        settings: previous.settings,
    })
}

fn start_conversion_blocking(
    state: AppState,
    request: StartConversionRequest,
    on_event: Channel<ConversionEventDto>,
) -> CommandResult<crate::history::RunSummary> {
    start_conversion_blocking_for(state, request, on_event, None)
}

/// Shared conversion pipeline for both a new run and a same-ID replay. Only
/// the state entry action differs; scanning, observer persistence and terminal
/// finalization are intentionally identical.
fn start_conversion_blocking_for(
    state: AppState,
    request: StartConversionRequest,
    on_event: Channel<ConversionEventDto>,
    rerun_id: Option<i64>,
) -> CommandResult<crate::history::RunSummary> {
    let validated = validate_input_path(request.input_path)?;
    let output_path = validate_output_dir(request.output_path)?;
    let config = RunOptions {
        input: validated.path,
        output: output_path,
        layout: OutputLayout::Mirror,
        copy_passthrough: request.settings.copy_passthrough,
        overwrite: request.settings.overwrite,
        watch: false,
        update_metadata: request.settings.update_metadata,
        ..RunOptions::new(PathBuf::new())
    }
    .build()
    .map_err(|error| CommandError::invalid_input(error.to_string()))?;
    let config = Arc::new(config);
    let active = match rerun_id {
        Some(run_id) => state.begin_rerun(run_id),
        None => state.begin_run(CreateRun {
            input_kind: if config.input_is_dir {
                InputKind::Directory
            } else {
                InputKind::File
            },
            input_path: config.input.clone(),
            output_path: config.output.clone(),
            settings: request.settings,
            total_items: 0,
        }),
    }
    .map_err(|error| CommandError {
        code: if matches!(error, crate::state::StateError::Busy { .. }) {
            "busy"
        } else {
            "state_error"
        },
        detail: error.to_string(),
    })?;
    let paths = match scan_for_run(&config) {
        Ok(paths) => paths,
        Err(error) => {
            return Err(finalize_startup_failure(
                &state,
                &active,
                format!("scan failed: {error}"),
            ));
        }
    };
    if let Err(error) = state.mark_run_running(active.run_id, paths.len()) {
        return Err(finalize_startup_failure(
            &state,
            &active,
            format!("could not mark scanned run active: {error}"),
        ));
    }
    #[cfg(test)]
    if FORCE_CANCEL_AFTER_MARK_RUNNING.load(std::sync::atomic::Ordering::SeqCst) {
        state.cancel_run(active.run_id);
    }
    config.presentation.begin_batch(paths.len());

    let persistence_error = Arc::new(Mutex::new(None::<String>));
    let persistence_error_for_sink = persistence_error.clone();
    let mut coordinator =
        PersistenceCoordinator::start(state.history().clone(), active.run_id, config.jobs.get());
    let coordinator_sender = coordinator.sender.clone();
    let cancellation = active.cancellation.clone();
    let run_id = active.run_id;
    let progress_channel = on_event.clone();
    let observer = BatchObserver::new(Arc::new(move |event| match event {
        BatchEvent::PlanningStarted { total } => {
            let _ = progress_channel.send(ConversionEventDto::PlanningStarted { run_id, total });
        }
        BatchEvent::ItemPlanning {
            ordinal,
            source,
            relative_path,
            item_kind,
        } => {
            let _ = progress_channel.send(ConversionEventDto::ItemPlanning {
                run_id,
                ordinal,
                source,
                relative_path,
                item_kind,
            });
        }
        BatchEvent::ItemProcessing {
            ordinal,
            source,
            relative_path,
            destination,
            item_kind,
            action,
        } => {
            let _ = progress_channel.send(ConversionEventDto::ItemProcessing {
                run_id,
                ordinal,
                source,
                relative_path,
                destination,
                item_kind,
                action,
            });
        }
        BatchEvent::ItemTerminal { report } => {
            if persistence_error_for_sink
                .lock()
                .expect("persistence error lock")
                .is_some()
            {
                return;
            }
            if let Err(error) =
                handle_terminal_persistence(&coordinator_sender, report, &cancellation, |report| {
                    // The coordinator commits first, then and only then does
                    // this event become visible to the frontend.
                    let _ =
                        progress_channel.send(ConversionEventDto::ItemTerminal { run_id, report });
                })
            {
                *persistence_error_for_sink
                    .lock()
                    .expect("persistence error lock") = Some(error);
            }
        }
        // The coordinator commits the terminal run row before it sends this
        // event, so a reload cannot observe a phantom terminal state.
        BatchEvent::Completed { .. } => {}
    }))
    .with_music_snapshots();

    let runtime = match build_conversion_runtime() {
        Ok(runtime) => runtime,
        Err(detail) => return Err(finalize_startup_failure(&state, &active, detail)),
    };
    let result = runtime.block_on(app::runner::run_paths_with_observer_and_cancel(
        config,
        paths,
        active.cancellation.clone(),
        observer,
    ));
    coordinator.shutdown();
    if let Some(detail) = persistence_error
        .lock()
        .expect("persistence error lock")
        .clone()
    {
        if let Err(error) = state.fail_active_run(run_id, &detail) {
            return Err(CommandError {
                code: "history_terminal_update_failed",
                detail: format!("{detail}; additionally failed to close active run: {error}"),
            });
        }
        return Err(CommandError {
            code: "history_persistence_failed",
            detail,
        });
    }
    let summary = match result {
        Ok(summary) => summary,
        Err(error) => {
            let detail = error.to_string();
            if let Err(history_error) = state.fail_active_run(run_id, &detail) {
                return Err(CommandError {
                    code: "history_terminal_update_failed",
                    detail: format!(
                        "{detail}; additionally failed to close active run: {history_error}"
                    ),
                });
            }
            return Err(CommandError {
                code: "conversion_failed",
                detail,
            });
        }
    };
    let cancelled = active.cancellation.is_cancelled();
    let status = status_for_summary(summary, cancelled);
    let terminal = state
        .finish_run(run_id, status, None, None)
        .map_err(|error| CommandError {
            code: "history_terminal_update_failed",
            detail: error.to_string(),
        })?;
    let _ = on_event.send(ConversionEventDto::Completed {
        run_id,
        summary,
        cancelled,
    });
    Ok(terminal)
}

fn finalize_startup_failure(
    state: &AppState,
    active: &crate::state::ActiveRun,
    detail: String,
) -> CommandError {
    match state.fail_active_run(active.run_id, &detail) {
        Ok(_) => CommandError {
            code: "conversion_startup_failed",
            detail,
        },
        Err(error) => CommandError {
            code: "history_terminal_update_failed",
            detail: format!("{detail}; additionally failed to close active run: {error}"),
        },
    }
}

#[cfg(test)]
static FORCE_RUNTIME_FAILURE: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

#[cfg(test)]
static FORCE_SCAN_FAILURE: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

#[cfg(test)]
static FORCE_CANCEL_AFTER_MARK_RUNNING: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

fn scan_for_run(
    config: &Arc<unlockm::config::RunConfig>,
) -> Result<Vec<PathBuf>, unlockm::error::AppError> {
    #[cfg(test)]
    if FORCE_SCAN_FAILURE.load(std::sync::atomic::Ordering::SeqCst) {
        return Err(unlockm::error::AppError::Config(
            "injected scan failure".into(),
        ));
    }
    app::scan::enumerate(config)
}

fn build_conversion_runtime() -> Result<tokio::runtime::Runtime, String> {
    #[cfg(test)]
    if FORCE_RUNTIME_FAILURE.load(std::sync::atomic::Ordering::SeqCst) {
        return Err("injected runtime initialization failure".into());
    }
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn cancel_conversion(state: State<'_, AppState>, run_id: i64) -> bool {
    state.cancel_run(run_id)
}

fn status_for_summary(summary: Summary, cancelled: bool) -> RunStatus {
    if cancelled {
        RunStatus::Cancelled
    } else if summary.failed + summary.conflicted == 0 {
        RunStatus::Completed
    } else if summary.converted + summary.copied + summary.skipped > 0 {
        RunStatus::Partial
    } else {
        RunStatus::Failed
    }
}

fn item_for_history(report: &JobReport) -> PersistedItemInput {
    let music = report.music.as_ref();
    PersistedItemInput {
        ordinal: report.ordinal,
        item_kind: match report.item_kind {
            ItemKind::Music => StoredItemKind::Music,
            ItemKind::Companion => StoredItemKind::Companion,
        },
        source_path: report.source.clone(),
        relative_path: report.relative_path.clone(),
        destination_path: report.destination.clone(),
        action: report.action.map(|action| match action {
            ItemAction::Convert => "convert".into(),
            ItemAction::Copy => "copy".into(),
        }),
        outcome: outcome_name(report.outcome).into(),
        logical_bytes: report.logical_bytes,
        output_bytes: music.and_then(|music| music.output_bytes),
        output_format: music.and_then(|music| music.output_format.clone()),
        bitrate_kbps: music.and_then(|music| music.bitrate_kbps),
        preflight_ms: report.timings.preflight.as_millis() as u64,
        decrypt_ms: report.timings.decrypt.as_millis() as u64,
        metadata_resolve_ms: report.timings.metadata_resolve.as_millis() as u64,
        tag_write_ms: report.timings.tag_write.as_millis() as u64,
        fallback_ms: report.timings.fallback.as_millis() as u64,
        copy_ms: report.timings.copy.as_millis() as u64,
        publish_ms: report.timings.publish.as_millis() as u64,
        error_code: report.error.as_ref().map(|error| error.code.clone()),
        error_detail: report.error.as_ref().map(|error| error.detail.clone()),
        warnings: report
            .warnings
            .iter()
            .map(|warning| StoredWarning {
                code: warning.code.clone(),
                detail: warning.detail.clone(),
            })
            .collect(),
        title: music.and_then(|music| music.title.clone()),
        artist: music.and_then(|music| music.artist.clone()),
        album: music.and_then(|music| music.album.clone()),
        duration_ms: music.and_then(|music| music.duration_ms),
        artwork: music
            .and_then(|music| music.artwork.as_ref())
            .map(|artwork| ArtworkInput {
                mime_type: artwork.mime_type.clone(),
                bytes: artwork.bytes.clone(),
            }),
    }
}

fn outcome_name(outcome: JobOutcome) -> &'static str {
    match outcome {
        JobOutcome::Converted => "converted",
        JobOutcome::Copied => "copied",
        JobOutcome::Skipped => "skipped",
        JobOutcome::Conflict => "conflict",
        JobOutcome::Failed => "failed",
        JobOutcome::Cancelled => "cancelled",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::{HistoryStore, InputKind};
    use base64::{Engine, engine::general_purpose::STANDARD};
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn start_test_guard() -> std::sync::MutexGuard<'static, ()> {
        static GUARD: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
        GUARD
            .get_or_init(|| std::sync::Mutex::new(()))
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
    }

    fn conversion_settings() -> RunSettings {
        RunSettings {
            update_metadata: false,
            copy_passthrough: true,
            overwrite: false,
        }
    }

    #[test]
    fn rerun_snapshot_copies_terminal_request_and_rejects_nonterminal_or_missing() {
        let directory = tempfile::tempdir().unwrap();
        let history = HistoryStore::open(directory.path().join("history.sqlite")).unwrap();
        let settings = conversion_settings();
        let run = history
            .create_run(CreateRun {
                input_kind: InputKind::Directory,
                input_path: directory.path().join("input"),
                output_path: directory.path().join("output"),
                settings: settings.clone(),
                total_items: 0,
            })
            .unwrap();
        assert_eq!(
            rerun_snapshot(&history, run.id).unwrap_err().code,
            "state_error"
        );
        history
            .finish_run(run.id, RunStatus::Completed, None, None)
            .unwrap();
        history.rename_run(run.id, "Album rerun").unwrap();
        let request = rerun_snapshot(&history, run.id).unwrap();
        assert_eq!(request.settings, settings);
        assert_eq!(request.input_path, directory.path().join("input"));
        assert_eq!(request.output_path, Some(directory.path().join("output")));
        assert_eq!(
            rerun_snapshot(&history, run.id + 1).unwrap_err().code,
            "invalid_input"
        );
    }

    #[test]
    fn metadata_patch_validation_limits_trimmed_fields_and_cover_bytes() {
        let mut patch = MetadataPatchRequest {
            title: Some(Some("x".repeat(513))),
            artist: None,
            album: None,
            cover: None,
            existing_artwork_id: None,
        };
        assert_eq!(
            validate_metadata_patch(&patch).unwrap_err().code,
            "invalid_input"
        );
        patch.title = Some(Some(" title ".into()));
        patch.cover = Some(vec![0; unlockm::metadata::tags::MAX_COVER_BYTES + 1]);
        assert_eq!(
            validate_metadata_patch(&patch).unwrap_err().code,
            "invalid_input"
        );
        patch.cover = None;
        assert!(validate_metadata_patch(&patch).is_ok());
    }

    #[test]
    fn metadata_patch_normalizes_whitespace_to_clear_or_trimmed_value() {
        let patch = normalize_metadata_patch(MetadataPatchRequest {
            title: Some(Some("   ".into())),
            artist: Some(Some(" A ".into())),
            album: Some(Some("\t".into())),
            cover: None,
            existing_artwork_id: None,
        })
        .unwrap();
        assert_eq!(patch.title, Some(None));
        assert_eq!(patch.artist, Some(Some("A".into())));
        assert_eq!(patch.album, Some(None));
    }

    #[test]
    fn metadata_edit_reconciles_the_published_output_facts() {
        let _guard = start_test_guard();
        let directory = tempfile::tempdir().unwrap();
        let input = directory.path().join("track.ncm");
        let output = directory.path().join("output");
        fs::write(&input, silent_ncm()).unwrap();
        let state =
            AppState::new(HistoryStore::open(directory.path().join("history.sqlite")).unwrap())
                .unwrap();
        let run = start_conversion_blocking(
            state.clone(),
            StartConversionRequest {
                input_path: input,
                output_path: Some(output.clone()),
                settings: conversion_settings(),
            },
            channel(),
        )
        .unwrap();
        let before = state
            .history()
            .query_music_items(run.id, ItemQuery::default())
            .unwrap()
            .items
            .pop()
            .unwrap();
        let cover = STANDARD
            .decode(
                "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVQIHWP4z8DwHwAFgAI/ScLq6QAAAABJRU5ErkJggg==",
            )
            .unwrap();
        let updated = update_run_item_metadata_blocking(
            &state,
            before.id,
            MetadataPatchRequest {
                title: Some(Some("Cover-sized output".into())),
                artist: None,
                album: None,
                cover: Some(cover),
                existing_artwork_id: None,
            },
        )
        .unwrap();
        let output = output.join("track.mp3");
        let published_bytes = fs::metadata(output).unwrap().len();
        assert_ne!(updated.output_bytes, before.output_bytes);
        assert_eq!(updated.output_bytes, Some(published_bytes));
        assert_eq!(updated.output_format.as_deref(), Some("mp3"));
        assert_eq!(
            state.history().history_item_by_id(updated.id).unwrap(),
            Some(updated)
        );
    }

    #[test]
    fn metadata_mutations_serialize_the_full_publish_and_reconcile_sequence() {
        let _guard = start_test_guard();
        let directory = tempfile::tempdir().unwrap();
        let input = directory.path().join("track.ncm");
        let output = directory.path().join("output");
        fs::write(&input, silent_ncm()).unwrap();
        let state =
            AppState::new(HistoryStore::open(directory.path().join("history.sqlite")).unwrap())
                .unwrap();
        let run = start_conversion_blocking(
            state.clone(),
            StartConversionRequest {
                input_path: input,
                output_path: Some(output),
                settings: conversion_settings(),
            },
            channel(),
        )
        .unwrap();
        let item_id = state
            .history()
            .query_music_items(run.id, ItemQuery::default())
            .unwrap()
            .items[0]
            .id;
        let barrier = Arc::new(std::sync::Barrier::new(2));
        set_metadata_mutation_barrier(Some(barrier.clone()));
        let first_state = state.clone();
        let (first_sender, first_receiver) = std::sync::mpsc::channel();
        let first = std::thread::spawn(move || {
            first_sender
                .send(update_run_item_metadata_blocking(
                    &first_state,
                    item_id,
                    MetadataPatchRequest {
                        title: Some(Some("first".into())),
                        artist: None,
                        album: None,
                        cover: None,
                        existing_artwork_id: None,
                    },
                ))
                .unwrap();
        });
        // The first command owns the state guard before it reaches any file or
        // SQLite work. A second mutation must remain blocked on that guard.
        barrier.wait();
        let second_state = state.clone();
        let (second_sender, second_receiver) = std::sync::mpsc::channel();
        let second = std::thread::spawn(move || {
            second_sender
                .send(update_run_item_metadata_blocking(
                    &second_state,
                    item_id,
                    MetadataPatchRequest {
                        title: Some(Some("second".into())),
                        artist: None,
                        album: None,
                        cover: None,
                        existing_artwork_id: None,
                    },
                ))
                .unwrap();
        });
        assert!(
            second_receiver
                .recv_timeout(std::time::Duration::from_millis(30))
                .is_err()
        );
        set_metadata_mutation_barrier(None);
        barrier.wait();
        first_receiver
            .recv_timeout(std::time::Duration::from_secs(1))
            .unwrap()
            .unwrap();
        second_receiver
            .recv_timeout(std::time::Duration::from_secs(1))
            .unwrap()
            .unwrap();
        first.join().unwrap();
        second.join().unwrap();
        assert_eq!(
            state
                .history()
                .history_item_by_id(item_id)
                .unwrap()
                .unwrap()
                .title
                .as_deref(),
            Some("second")
        );
    }

    fn silent_ncm() -> Vec<u8> {
        let audio = STANDARD
            .decode(include_str!("../../../../tests/fixtures/lofty-silent.mp3.b64").trim())
            .unwrap();
        crate::desktop_ncm_fixture::ncm_with_audio(&[], &audio)
    }

    fn channel() -> Channel<ConversionEventDto> {
        Channel::new(|_| Ok(()))
    }

    #[test]
    fn validation_rejects_missing_paths() {
        let missing = std::env::temp_dir().join("unlockm-missing-input");
        assert_eq!(
            validate_input_path(missing).unwrap_err().code,
            "invalid_input"
        );
    }

    #[test]
    fn output_validation_rejects_files_and_normalizes_new_directory_roots() {
        let directory = tempfile::tempdir().unwrap();
        let file = directory.path().join("not-a-directory");
        fs::write(&file, b"x").unwrap();
        assert_eq!(
            validate_output_dir(Some(file)).unwrap_err().code,
            "invalid_input"
        );
        let output = validate_output_dir(Some(directory.path().join("new-output")))
            .unwrap()
            .unwrap();
        assert_eq!(
            output,
            directory.path().canonicalize().unwrap().join("new-output")
        );
    }

    #[cfg(unix)]
    #[test]
    fn output_validation_rejects_symlink_roots() {
        let directory = tempfile::tempdir().unwrap();
        let target = directory.path().join("target");
        let link = directory.path().join("link");
        fs::create_dir(&target).unwrap();
        std::os::unix::fs::symlink(&target, &link).unwrap();
        assert_eq!(
            validate_output_dir(Some(link)).unwrap_err().code,
            "invalid_input"
        );
    }

    #[test]
    fn recorded_output_validation_and_native_adapter_use_only_the_validated_target() {
        let directory = tempfile::tempdir().unwrap();
        let file = directory.path().join("track.mp3");
        fs::write(&file, b"audio").unwrap();
        let validated_file = validated_recorded_file(file.clone()).unwrap();
        let validated_directory = validated_recorded_directory(directory.path().into()).unwrap();
        assert_eq!(validated_file, file.canonicalize().unwrap());
        assert_eq!(
            validated_directory,
            directory.path().canonicalize().unwrap()
        );

        let seen = std::sync::Mutex::new(None);
        assert!(
            invoke_native_action(&validated_file, |path| {
                *seen.lock().unwrap() = Some(path.to_path_buf());
                Ok(())
            })
            .unwrap()
        );
        assert_eq!(*seen.lock().unwrap(), Some(validated_file));
        assert_eq!(
            invoke_native_action(&file, |_| Err("native open failed".into()))
                .unwrap_err()
                .code,
            "output_unavailable"
        );
    }

    #[cfg(unix)]
    #[test]
    fn recorded_file_validation_rejects_symlink_replacement() {
        let directory = tempfile::tempdir().unwrap();
        let target = directory.path().join("target.mp3");
        let link = directory.path().join("recorded.mp3");
        fs::write(&target, b"audio").unwrap();
        std::os::unix::fs::symlink(target, &link).unwrap();
        assert_eq!(
            validated_recorded_file(link).unwrap_err().code,
            "output_unavailable"
        );
    }

    fn report(ordinal: u64) -> Box<JobReport> {
        Box::new(JobReport {
            ordinal,
            source: format!("/input/{ordinal}.ncm").into(),
            relative_path: format!("{ordinal}.ncm").into(),
            destination: Some(format!("/output/{ordinal}.flac").into()),
            item_kind: ItemKind::Music,
            action: Some(ItemAction::Convert),
            logical_bytes: 1,
            outcome: JobOutcome::Converted,
            error: None,
            warnings: Vec::new(),
            music: None,
            timings: Default::default(),
        })
    }

    #[test]
    fn coordinator_acks_only_after_commit_and_surfaces_persistence_failure() {
        let directory = tempfile::tempdir().unwrap();
        let database = directory.path().join("history.sqlite");
        let history = Arc::new(HistoryStore::open(&database).unwrap());
        let run = history
            .create_run(CreateRun {
                input_kind: InputKind::File,
                input_path: "/input/track.ncm".into(),
                output_path: "/output".into(),
                settings: RunSettings {
                    update_metadata: false,
                    copy_passthrough: true,
                    overwrite: false,
                },
                total_items: 1,
            })
            .unwrap();
        let mut coordinator = PersistenceCoordinator::start(history.clone(), run.id, 1);
        assert!(persist_with_sender(&coordinator.sender, report(0)).is_ok());
        let independent_reader = HistoryStore::open(&database).unwrap();
        assert_eq!(
            independent_reader
                .query_music_items(run.id, ItemQuery::default())
                .unwrap()
                .items
                .len(),
            1
        );
        assert!(persist_with_sender(&coordinator.sender, report(0)).is_err());
        coordinator.shutdown();
        assert!(persist_with_sender(&coordinator.sender, report(1)).is_err());
    }

    #[test]
    fn coordinator_failure_returns_no_success_ack_and_allows_cancellation() {
        let directory = tempfile::tempdir().unwrap();
        let history =
            Arc::new(HistoryStore::open(directory.path().join("history.sqlite")).unwrap());
        let run = history
            .create_run(CreateRun {
                input_kind: InputKind::File,
                input_path: "/input/track.ncm".into(),
                output_path: "/output".into(),
                settings: RunSettings {
                    update_metadata: false,
                    copy_passthrough: true,
                    overwrite: false,
                },
                total_items: 1,
            })
            .unwrap();
        history
            .execute_test_sql(
                "CREATE TRIGGER reject_items BEFORE INSERT ON items
                 BEGIN SELECT RAISE(ABORT, 'injected item failure'); END;",
            )
            .unwrap();
        let mut coordinator = PersistenceCoordinator::start(history.clone(), run.id, 1);
        let cancellation = unlockm::app::runner::Cancellation::default();
        let emitted = AtomicUsize::new(0);
        assert!(
            handle_terminal_persistence(&coordinator.sender, report(0), &cancellation, |_| {
                emitted.fetch_add(1, Ordering::SeqCst);
            },)
            .is_err()
        );
        assert!(cancellation.is_cancelled());
        assert_eq!(emitted.load(Ordering::SeqCst), 0);
        // A later terminal can only receive another error acknowledgement; it
        // can never be mistaken for a committed terminal item.
        assert!(
            handle_terminal_persistence(&coordinator.sender, report(1), &cancellation, |_| {
                emitted.fetch_add(1, Ordering::SeqCst);
            },)
            .is_err()
        );
        assert_eq!(emitted.load(Ordering::SeqCst), 0);
        coordinator.shutdown();
        assert_eq!(
            history
                .query_music_items(run.id, ItemQuery::default())
                .unwrap()
                .items
                .len(),
            0
        );
    }

    #[test]
    fn typescript_bootstrap_fixture_matches_rust_camel_case_contract() {
        let actual = serde_json::to_value(BootstrapResponse {
            product_name: PRODUCT_NAME,
            supported_locales: SUPPORTED_LOCALES,
            preferences: Preferences::default(),
            history: RunPage {
                runs: Vec::new(),
                next_cursor: None,
            },
            active_run: None,
        })
        .unwrap();
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../src/contracts/bootstrap.fixture.json"
        ))
        .unwrap();
        assert_eq!(actual, fixture);
    }

    #[test]
    fn busy_start_never_scans_or_creates_output() {
        let _guard = start_test_guard();
        let directory = tempfile::tempdir().unwrap();
        let input = directory.path().join("input");
        let output = input.join("would-be-output");
        fs::create_dir(&input).unwrap();
        fs::write(input.join("track.lrc"), b"track").unwrap();
        let state =
            AppState::new(HistoryStore::open(directory.path().join("history.sqlite")).unwrap())
                .unwrap();
        let active = state
            .begin_run(CreateRun {
                input_kind: InputKind::Directory,
                input_path: input.clone(),
                output_path: output.clone(),
                settings: RunSettings {
                    update_metadata: false,
                    copy_passthrough: true,
                    overwrite: false,
                },
                total_items: 0,
            })
            .unwrap();
        let channel = Channel::<ConversionEventDto>::new(|_| Ok(()));
        let result = start_conversion_blocking(
            state.clone(),
            StartConversionRequest {
                input_path: input,
                output_path: Some(output.clone()),
                settings: RunSettings {
                    update_metadata: false,
                    copy_passthrough: true,
                    overwrite: false,
                },
            },
            channel,
        );
        assert_eq!(result.unwrap_err().code, "busy");
        assert!(!output.exists());
        assert!(state.cancel_run(active.run_id));
    }

    #[test]
    fn runtime_startup_failure_finalizes_the_planning_run() {
        let _guard = start_test_guard();
        let directory = tempfile::tempdir().unwrap();
        let input = directory.path().join("input");
        fs::create_dir(&input).unwrap();
        fs::write(input.join("track.lrc"), b"track").unwrap();
        let state =
            AppState::new(HistoryStore::open(directory.path().join("history.sqlite")).unwrap())
                .unwrap();
        FORCE_RUNTIME_FAILURE.store(true, Ordering::SeqCst);
        let result = start_conversion_blocking(
            state.clone(),
            StartConversionRequest {
                input_path: input,
                output_path: None,
                settings: RunSettings {
                    update_metadata: false,
                    copy_passthrough: true,
                    overwrite: false,
                },
            },
            Channel::<ConversionEventDto>::new(|_| Ok(())),
        );
        FORCE_RUNTIME_FAILURE.store(false, Ordering::SeqCst);
        assert_eq!(result.unwrap_err().code, "conversion_startup_failed");
        assert!(state.active_run().is_none());
        let runs = state.history().list_runs(None, 10).unwrap();
        assert_eq!(runs.runs.len(), 1);
        assert_eq!(runs.runs[0].status, RunStatus::Failed);
    }

    #[test]
    fn scan_failure_persists_failed_run_and_releases_active_slot() {
        let _guard = start_test_guard();
        let directory = tempfile::tempdir().unwrap();
        let input = directory.path().join("input");
        fs::create_dir(&input).unwrap();
        let state =
            AppState::new(HistoryStore::open(directory.path().join("history.sqlite")).unwrap())
                .unwrap();
        FORCE_SCAN_FAILURE.store(true, Ordering::SeqCst);
        let result = start_conversion_blocking(
            state.clone(),
            StartConversionRequest {
                input_path: input,
                output_path: None,
                settings: RunSettings {
                    update_metadata: false,
                    copy_passthrough: true,
                    overwrite: false,
                },
            },
            Channel::<ConversionEventDto>::new(|_| Ok(())),
        );
        FORCE_SCAN_FAILURE.store(false, Ordering::SeqCst);
        assert_eq!(result.unwrap_err().code, "conversion_startup_failed");
        assert!(state.active_run().is_none());
        let runs = state.history().list_runs(None, 10).unwrap();
        assert_eq!(runs.runs.len(), 1);
        assert_eq!(runs.runs[0].status, RunStatus::Failed);
        assert_eq!(runs.runs[0].total_items, 0);
    }

    #[test]
    fn desktop_coordinator_persists_single_file_input() {
        let _guard = start_test_guard();
        let directory = tempfile::tempdir().unwrap();
        let database = directory.path().join("history.sqlite");
        let input = directory.path().join("single.ncm");
        let output = directory.path().join("output");
        let single = STANDARD
            .decode(include_str!("../../../../tests/fixtures/lofty-silent.mp3.b64").trim())
            .unwrap();
        let mut audio = Vec::with_capacity(48 * 1024 * 1024);
        while audio.len() < 48 * 1024 * 1024 {
            audio.extend_from_slice(&single);
        }
        fs::write(
            &input,
            crate::desktop_ncm_fixture::ncm_with_audio(&[], &audio),
        )
        .unwrap();
        let state = AppState::new(HistoryStore::open(&database).unwrap()).unwrap();
        let run = start_conversion_blocking(
            state.clone(),
            StartConversionRequest {
                input_path: input,
                output_path: Some(output.clone()),
                settings: conversion_settings(),
            },
            channel(),
        )
        .unwrap();
        assert_eq!(run.status, RunStatus::Completed);
        assert!(output.join("single.mp3").is_file());
        let page = state
            .history()
            .query_music_items(run.id, ItemQuery::default())
            .unwrap();
        assert_eq!(page.items.len(), 1);
        let published = fs::metadata(output.join("single.mp3")).unwrap().len();
        assert_eq!(page.items[0].output_bytes, Some(published));
        assert_eq!(page.items[0].output_format.as_deref(), Some("mp3"));
    }

    #[test]
    fn desktop_coordinator_persists_nested_directory_and_restart_history() {
        let _guard = start_test_guard();
        let directory = tempfile::tempdir().unwrap();
        let database = directory.path().join("history.sqlite");
        let input = directory.path().join("input");
        let output = directory.path().join("output");
        fs::create_dir_all(input.join("nested")).unwrap();
        fs::write(input.join("top.ncm"), silent_ncm()).unwrap();
        fs::write(input.join("nested/track.ncm"), silent_ncm()).unwrap();
        let state = AppState::new(HistoryStore::open(&database).unwrap()).unwrap();
        let run = start_conversion_blocking(
            state.clone(),
            StartConversionRequest {
                input_path: input.clone(),
                output_path: Some(output.clone()),
                settings: conversion_settings(),
            },
            channel(),
        )
        .unwrap();
        assert_eq!(run.status, RunStatus::Completed);
        assert!(output.join("top.mp3").is_file());
        assert!(output.join("nested/track.mp3").is_file());
        let items = state
            .history()
            .query_music_items(run.id, ItemQuery::default())
            .unwrap();
        assert_eq!(items.items.len(), 2);
        drop(state);
        let reopened = AppState::new(HistoryStore::open(database).unwrap()).unwrap();
        assert_eq!(
            reopened.history().run(run.id).unwrap().unwrap().status,
            RunStatus::Completed
        );
        assert_eq!(
            reopened
                .history()
                .query_music_items(run.id, ItemQuery::default())
                .unwrap()
                .items
                .len(),
            2
        );
    }

    #[test]
    fn rerun_snapshot_preserves_named_history_request() {
        let directory = tempfile::tempdir().unwrap();
        let history = HistoryStore::open(directory.path().join("history.sqlite")).unwrap();
        let input = directory.path().join("input");
        let output = directory.path().join("output");
        std::fs::create_dir(&input).unwrap();
        let run = history
            .create_run(CreateRun {
                input_kind: InputKind::Directory,
                input_path: input.clone(),
                output_path: output.clone(),
                settings: conversion_settings(),
                total_items: 1,
            })
            .unwrap();
        history
            .finish_run(run.id, RunStatus::Completed, None, None)
            .unwrap();
        history.rename_run(run.id, "Library").unwrap();
        let request = rerun_snapshot(&history, run.id).unwrap();
        assert_eq!(request.input_path, input);
        assert_eq!(request.output_path, Some(output));
        assert_eq!(request.settings, conversion_settings());
    }

    #[test]
    fn rerun_replaces_terminal_run_in_place_and_keeps_its_request_name_and_id() {
        let _guard = start_test_guard();
        let directory = tempfile::tempdir().unwrap();
        let input = directory.path().join("input");
        let output = directory.path().join("output");
        fs::create_dir(&input).unwrap();
        fs::write(input.join("first.ncm"), silent_ncm()).unwrap();
        let state =
            AppState::new(HistoryStore::open(directory.path().join("history.sqlite")).unwrap())
                .unwrap();
        let original = start_conversion_blocking(
            state.clone(),
            StartConversionRequest {
                input_path: input.clone(),
                output_path: Some(output.clone()),
                settings: conversion_settings(),
            },
            channel(),
        )
        .unwrap();
        state
            .history()
            .rename_run(original.id, "Downloads")
            .unwrap();
        fs::write(input.join("later.ncm"), silent_ncm()).unwrap();

        let request = rerun_snapshot(state.history(), original.id).unwrap();
        let rerun =
            start_conversion_blocking_for(state.clone(), request, channel(), Some(original.id))
                .unwrap();

        assert_eq!(rerun.display_name, "Downloads");
        assert_eq!(rerun.id, original.id);
        assert_eq!(rerun.converted, 1);
        assert_eq!(rerun.skipped, 1);
        assert!(output.join("first.mp3").is_file());
        assert!(output.join("later.mp3").is_file());
        let after = state.history().run(original.id).unwrap().unwrap();
        assert_eq!(after.completed_items, 2);
        assert_eq!(after.converted, 1);
        assert_eq!(after.skipped, 1);
        assert_eq!(state.history().list_runs(None, 10).unwrap().runs.len(), 1);
    }

    #[test]
    fn rerun_scan_failure_replaces_old_facts_without_emitting_misleading_events() {
        let _guard = start_test_guard();
        let directory = tempfile::tempdir().unwrap();
        let input = directory.path().join("input");
        let output = directory.path().join("output");
        fs::create_dir(&input).unwrap();
        fs::write(input.join("track.ncm"), silent_ncm()).unwrap();
        let state =
            AppState::new(HistoryStore::open(directory.path().join("history.sqlite")).unwrap())
                .unwrap();
        let original = start_conversion_blocking(
            state.clone(),
            StartConversionRequest {
                input_path: input,
                output_path: Some(output),
                settings: conversion_settings(),
            },
            channel(),
        )
        .unwrap();
        let events = Arc::new(Mutex::new(Vec::<serde_json::Value>::new()));
        let received = events.clone();
        let event_channel = Channel::new(move |body| {
            received
                .lock()
                .unwrap()
                .push(body.deserialize::<serde_json::Value>().unwrap());
            Ok(())
        });
        FORCE_SCAN_FAILURE.store(true, Ordering::SeqCst);
        let request = rerun_snapshot(state.history(), original.id).unwrap();
        let result =
            start_conversion_blocking_for(state.clone(), request, event_channel, Some(original.id));
        FORCE_SCAN_FAILURE.store(false, Ordering::SeqCst);
        assert_eq!(result.unwrap_err().code, "conversion_startup_failed");
        let failed = state.history().run(original.id).unwrap().unwrap();
        assert_eq!(failed.id, original.id);
        assert_eq!(failed.status, RunStatus::Failed);
        assert_eq!(failed.completed_items, 0);
        assert_eq!(failed.music_count, 0);
        assert_eq!(state.history().list_runs(None, 10).unwrap().runs.len(), 1);
        assert!(state.active_run().is_none());
        assert!(events.lock().unwrap().is_empty());
    }

    #[test]
    fn rerun_cancellation_keeps_the_original_event_run_id() {
        let _guard = start_test_guard();
        let directory = tempfile::tempdir().unwrap();
        let input = directory.path().join("input");
        let output = directory.path().join("output");
        fs::create_dir(&input).unwrap();
        fs::write(input.join("track.ncm"), silent_ncm()).unwrap();
        let state =
            AppState::new(HistoryStore::open(directory.path().join("history.sqlite")).unwrap())
                .unwrap();
        let original = start_conversion_blocking(
            state.clone(),
            StartConversionRequest {
                input_path: input,
                output_path: Some(output),
                settings: conversion_settings(),
            },
            channel(),
        )
        .unwrap();
        let events = Arc::new(Mutex::new(Vec::<serde_json::Value>::new()));
        let received = events.clone();
        let event_channel = Channel::new(move |body| {
            received
                .lock()
                .unwrap()
                .push(body.deserialize::<serde_json::Value>().unwrap());
            Ok(())
        });
        FORCE_CANCEL_AFTER_MARK_RUNNING.store(true, Ordering::SeqCst);
        let request = rerun_snapshot(state.history(), original.id).unwrap();
        let rerun =
            start_conversion_blocking_for(state.clone(), request, event_channel, Some(original.id));
        FORCE_CANCEL_AFTER_MARK_RUNNING.store(false, Ordering::SeqCst);
        let rerun = rerun.unwrap();
        assert_eq!(rerun.id, original.id);
        assert_eq!(rerun.status, RunStatus::Cancelled);
        assert!(state.active_run().is_none());
        let events = events.lock().unwrap();
        assert!(!events.is_empty());
        assert!(
            events
                .iter()
                .all(|event| event.get("runId") == Some(&serde_json::json!(original.id))),
            "unexpected event run IDs: {events:?}"
        );
        assert!(events.iter().any(|event| {
            event.get("type") == Some(&serde_json::json!("completed"))
                && event.get("cancelled") == Some(&serde_json::json!(true))
        }));
    }

    #[test]
    fn desktop_cancellation_keeps_source_and_never_publishes_a_partial_file() {
        let _guard = start_test_guard();
        let directory = tempfile::tempdir().unwrap();
        let input = directory.path().join("large.ncm");
        let output = directory.path().join("output");
        let single = STANDARD
            .decode(include_str!("../../../../tests/fixtures/lofty-silent.mp3.b64").trim())
            .unwrap();
        let mut audio = Vec::with_capacity(64 * 1024 * 1024);
        while audio.len() < 64 * 1024 * 1024 {
            audio.extend_from_slice(&single);
        }
        fs::write(
            &input,
            crate::desktop_ncm_fixture::ncm_with_audio(&[], &audio),
        )
        .unwrap();
        let state =
            AppState::new(HistoryStore::open(directory.path().join("history.sqlite")).unwrap())
                .unwrap();
        let worker_state = state.clone();
        let input_for_worker = input.clone();
        let output_for_worker = output.clone();
        let worker = std::thread::spawn(move || {
            start_conversion_blocking(
                worker_state,
                StartConversionRequest {
                    input_path: input_for_worker,
                    output_path: Some(output_for_worker),
                    settings: conversion_settings(),
                },
                channel(),
            )
        });
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        let mut stage_seen = false;
        while std::time::Instant::now() < deadline {
            if output.is_dir()
                && fs::read_dir(&output)
                    .unwrap()
                    .filter_map(Result::ok)
                    .any(|entry| {
                        entry
                            .file_name()
                            .to_string_lossy()
                            .starts_with(".unlockm-stage-")
                    })
            {
                stage_seen = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        if !stage_seen {
            let result = worker.join().unwrap();
            panic!("conversion should stage private output before cancellation: {result:?}");
        }
        let active = state
            .active_run()
            .expect("conversion should remain active while staged");
        assert!(state.cancel_run(active.run_id));
        let run = worker.join().unwrap().unwrap();
        assert_eq!(run.status, RunStatus::Cancelled);
        assert!(state.active_run().is_none());
        assert!(input.is_file());
        assert!(!output.join("large.mp3").exists());
        assert!(
            fs::read_dir(&output)
                .unwrap()
                .filter_map(Result::ok)
                .all(|entry| {
                    !entry
                        .file_name()
                        .to_string_lossy()
                        .starts_with(".unlockm-stage-")
                })
        );
    }
}
