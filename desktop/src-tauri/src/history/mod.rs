//! Rust-owned local conversion history.
//!
//! The webview never receives a database path or SQL capability.  This store
//! keeps only user-visible conversion facts and bounded artwork bytes; it does
//! not open, delete, or otherwise mutate recorded source/output paths.

use rusqlite::{Connection, OpenFlags, OptionalExtension, Transaction, backup::Backup, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};
use thiserror::Error;

const SCHEMA_VERSION: i64 = 3;
const MAX_PAGE_SIZE: usize = 200;
const MAX_ARTWORK_BYTES: usize = 10 * 1024 * 1024;
const LEGACY_REPAIR_BATCH: usize = 32;

pub type HistoryResult<T> = Result<T, HistoryError>;

#[derive(Debug, Error)]
pub enum HistoryError {
    #[error("history I/O at {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("history database error: {0}")]
    Sql(#[from] rusqlite::Error),
    #[error("history JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("artwork exceeds the 10 MiB history limit")]
    ArtworkTooLarge,
    #[error("display name must contain 1–120 characters")]
    InvalidDisplayName,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputKind {
    File,
    Directory,
}

impl InputKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Directory => "directory",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Planning,
    Running,
    Completed,
    Partial,
    Failed,
    Cancelled,
    Interrupted,
}

impl RunStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Planning => "planning",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Partial => "partial",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::Interrupted => "interrupted",
        }
    }

    fn parse(value: &str) -> Self {
        match value {
            "planning" => Self::Planning,
            "running" => Self::Running,
            "completed" => Self::Completed,
            "partial" => Self::Partial,
            "failed" => Self::Failed,
            "cancelled" => Self::Cancelled,
            "interrupted" => Self::Interrupted,
            _ => Self::Failed,
        }
    }

    fn is_terminal(self) -> bool {
        !matches!(self, Self::Planning | Self::Running)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunSettings {
    pub update_metadata: bool,
    pub copy_passthrough: bool,
    pub overwrite: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateRun {
    pub input_kind: InputKind,
    pub input_path: PathBuf,
    pub output_path: PathBuf,
    pub settings: RunSettings,
    pub total_items: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunSummary {
    pub id: i64,
    pub display_name: String,
    pub input_kind: InputKind,
    pub input_path: PathBuf,
    pub output_path: PathBuf,
    pub settings: RunSettings,
    pub status: RunStatus,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
    pub finished_at_ms: Option<i64>,
    pub total_items: usize,
    pub completed_items: usize,
    pub converted: usize,
    pub copied: usize,
    pub skipped: usize,
    pub conflicted: usize,
    pub failed: usize,
    pub music_count: usize,
    pub representative_artwork_id: Option<i64>,
    pub terminal_error_code: Option<String>,
    pub terminal_error_detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunPage {
    pub runs: Vec<RunSummary>,
    pub next_cursor: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StoredItemKind {
    Music,
    Companion,
}

impl StoredItemKind {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Music => "music",
            Self::Companion => "companion",
        }
    }

    fn parse(value: &str) -> Self {
        if value == "companion" {
            Self::Companion
        } else {
            Self::Music
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtworkInput {
    pub mime_type: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredWarning {
    pub code: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersistedItemInput {
    pub ordinal: u64,
    pub item_kind: StoredItemKind,
    pub source_path: PathBuf,
    pub relative_path: PathBuf,
    pub destination_path: Option<PathBuf>,
    pub action: Option<String>,
    pub outcome: String,
    pub logical_bytes: u64,
    pub output_bytes: Option<u64>,
    pub output_format: Option<String>,
    pub bitrate_kbps: Option<u32>,
    pub preflight_ms: u64,
    pub decrypt_ms: u64,
    pub metadata_resolve_ms: u64,
    pub tag_write_ms: u64,
    pub fallback_ms: u64,
    pub copy_ms: u64,
    pub publish_ms: u64,
    pub error_code: Option<String>,
    pub error_detail: Option<String>,
    pub warnings: Vec<StoredWarning>,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration_ms: Option<u32>,
    pub artwork: Option<ArtworkInput>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryItem {
    pub id: i64,
    pub run_id: i64,
    pub ordinal: u64,
    pub item_kind: StoredItemKind,
    pub source_path: PathBuf,
    pub relative_path: PathBuf,
    pub destination_path: Option<PathBuf>,
    pub action: Option<String>,
    pub outcome: String,
    pub logical_bytes: u64,
    pub output_bytes: Option<u64>,
    pub output_format: Option<String>,
    pub bitrate_kbps: Option<u32>,
    pub media_probe_version: i64,
    pub metadata_edit_pending: bool,
    pub error_code: Option<String>,
    pub error_detail: Option<String>,
    pub warnings: Vec<StoredWarning>,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration_ms: Option<u32>,
    pub artwork_id: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemPage {
    pub items: Vec<HistoryItem>,
    pub page: usize,
    pub page_size: usize,
    pub page_count: usize,
    pub filtered_total: usize,
    pub companion_count: usize,
    pub music_total: usize,
    pub repair_pending: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ItemSort {
    #[default]
    Ordinal,
    Title,
    Artist,
    Album,
    Size,
}

impl ItemSort {
    fn sql(self) -> &'static str {
        match self {
            Self::Ordinal => "ordinal",
            Self::Title => "COALESCE(title, relative_path) COLLATE NOCASE",
            Self::Artist => "COALESCE(artist, '') COLLATE NOCASE",
            Self::Album => "COALESCE(album, '') COLLATE NOCASE",
            Self::Size => "COALESCE(output_bytes, -1)",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SortDirection {
    #[default]
    Asc,
    Desc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutcomeFilter {
    Converted,
    Copied,
    Skipped,
    Conflict,
    Failed,
    Cancelled,
}

impl OutcomeFilter {
    fn as_str(self) -> &'static str {
        match self {
            Self::Converted => "converted",
            Self::Copied => "copied",
            Self::Skipped => "skipped",
            Self::Conflict => "conflict",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }
}

impl SortDirection {
    fn sql(self) -> &'static str {
        match self {
            Self::Asc => "ASC",
            Self::Desc => "DESC",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemQuery {
    pub page: usize,
    pub page_size: usize,
    pub search: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub outcome: Option<OutcomeFilter>,
    pub sort: ItemSort,
    pub direction: SortDirection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FacetKind {
    Artist,
    Album,
}

impl FacetKind {
    fn sql(self) -> &'static str {
        match self {
            Self::Artist => "artist",
            Self::Album => "album",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FacetPage {
    pub values: Vec<String>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtworkRecord {
    pub id: i64,
    pub mime_type: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct EditableItem {
    pub id: i64,
    pub destination_path: PathBuf,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtworkCandidate {
    pub artwork_id: i64,
    pub album: Option<String>,
    pub artist: Option<String>,
    pub title: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Preferences {
    pub locale: String,
    #[serde(default)]
    pub theme: Theme,
    #[serde(default)]
    pub palette: Palette,
    #[serde(default = "default_player_volume")]
    pub player_volume: f64,
    pub default_settings: RunSettings,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    Light,
    Dark,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Palette {
    #[default]
    Blue,
    Emerald,
    Violet,
    Amber,
}

fn default_player_volume() -> f64 {
    0.6
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            locale: "en-US".into(),
            theme: Theme::Light,
            palette: Palette::Blue,
            player_volume: default_player_volume(),
            default_settings: RunSettings {
                update_metadata: false,
                copy_passthrough: true,
                overwrite: false,
            },
        }
    }
}

/// Preferences originate in the webview and may also come from older local
/// databases. Keep an explicit zero (mute) but never let an out-of-range or
/// non-finite value reach the HTML audio element, whose native default is 1.0.
pub fn sanitize_preferences(mut preferences: Preferences) -> Preferences {
    if !preferences.player_volume.is_finite() || !(0.0..=1.0).contains(&preferences.player_volume) {
        preferences.player_volume = default_player_volume();
    }
    preferences
}

pub struct HistoryStore {
    connection: Mutex<Connection>,
}

impl HistoryStore {
    pub fn backup_legacy_database(
        source: impl AsRef<Path>,
        destination: impl AsRef<Path>,
    ) -> HistoryResult<bool> {
        let source = source.as_ref();
        let destination = destination.as_ref();
        if destination.exists() || !source.is_file() {
            return Ok(false);
        }
        let parent = destination.parent().unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent).map_err(|source| HistoryError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
        let temporary = parent.join(format!(
            ".history-migration-{}-{}.sqlite",
            std::process::id(),
            now_ms()
        ));
        let backup_result = (|| -> HistoryResult<()> {
            let source = Connection::open_with_flags(source, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
            let mut destination = Connection::open(&temporary)?;
            let backup = Backup::new(&source, &mut destination)?;
            backup.run_to_completion(64, std::time::Duration::from_millis(10), None)?;
            Ok(())
        })();
        if let Err(error) = backup_result {
            let _ = fs::remove_file(&temporary);
            return Err(error);
        }
        if destination.exists() {
            let _ = fs::remove_file(&temporary);
            return Ok(false);
        }
        if let Err(source) = fs::rename(&temporary, destination) {
            let _ = fs::remove_file(&temporary);
            return Err(HistoryError::Io {
                path: destination.to_path_buf(),
                source,
            });
        }
        Ok(true)
    }

    pub fn open(path: impl AsRef<Path>) -> HistoryResult<Self> {
        let path = path.as_ref();
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent).map_err(|source| HistoryError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
        let connection = Connection::open(path)?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        connection.busy_timeout(std::time::Duration::from_secs(5))?;
        migrate(&connection)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    pub fn create_run(&self, request: CreateRun) -> HistoryResult<RunSummary> {
        let now = now_ms();
        let settings_json = serde_json::to_string(&request.settings)?;
        let connection = self.connection.lock().expect("history connection lock");
        connection.execute(
            "INSERT INTO runs (
                input_kind, input_path, output_path, settings_json, status,
                created_at_ms, updated_at_ms, total_items, app_version
             ) VALUES (?, ?, ?, ?, 'planning', ?, ?, ?, ?)",
            params![
                request.input_kind.as_str(),
                request.input_path.to_string_lossy(),
                request.output_path.to_string_lossy(),
                settings_json,
                now,
                now,
                request.total_items as i64,
                env!("CARGO_PKG_VERSION"),
            ],
        )?;
        let id = connection.last_insert_rowid();
        run_by_id(&connection, id)?.ok_or(rusqlite::Error::QueryReturnedNoRows.into())
    }

    pub fn mark_running(&self, run_id: i64) -> HistoryResult<()> {
        self.update_status(run_id, RunStatus::Running, None, None, false)
    }

    pub fn set_total_items(&self, run_id: i64, total_items: usize) -> HistoryResult<()> {
        let connection = self.connection.lock().expect("history connection lock");
        connection.execute(
            "UPDATE runs SET total_items = ?, updated_at_ms = ? WHERE id = ?",
            params![total_items as i64, now_ms(), run_id],
        )?;
        Ok(())
    }

    pub fn finish_run(
        &self,
        run_id: i64,
        status: RunStatus,
        terminal_error_code: Option<&str>,
        terminal_error_detail: Option<&str>,
    ) -> HistoryResult<()> {
        self.update_status(
            run_id,
            status,
            terminal_error_code,
            terminal_error_detail,
            true,
        )
    }

    pub fn record_item(&self, run_id: i64, item: PersistedItemInput) -> HistoryResult<i64> {
        let mut connection = self.connection.lock().expect("history connection lock");
        let transaction = connection.transaction()?;
        let artwork_id = item
            .artwork
            .as_ref()
            .map(|artwork| insert_or_get_artwork(&transaction, artwork))
            .transpose()?;
        transaction.execute(
            "INSERT INTO items (
                run_id, ordinal, item_kind, source_path, relative_path, destination_path,
                action, outcome, logical_bytes, output_bytes, output_format, preflight_ms, decrypt_ms,
                metadata_resolve_ms, tag_write_ms, fallback_ms, copy_ms, publish_ms,
                error_code, error_detail, warnings_json, title, artist, album, duration_ms, artwork_id,
                bitrate_kbps, media_probe_version
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                run_id,
                item.ordinal as i64,
                item.item_kind.as_str(),
                item.source_path.to_string_lossy(),
                item.relative_path.to_string_lossy(),
                item.destination_path.as_ref().map(|path| path.to_string_lossy().into_owned()),
                item.action,
                item.outcome,
                item.logical_bytes as i64,
                item.output_bytes.map(|value| value as i64),
                item.output_format,
                item.preflight_ms as i64,
                item.decrypt_ms as i64,
                item.metadata_resolve_ms as i64,
                item.tag_write_ms as i64,
                item.fallback_ms as i64,
                item.copy_ms as i64,
                item.publish_ms as i64,
                item.error_code,
                item.error_detail,
                serde_json::to_string(&item.warnings)?,
                item.title,
                item.artist,
                item.album,
                item.duration_ms.map(i64::from),
                artwork_id,
                item.bitrate_kbps.map(i64::from),
                1_i64,
            ],
        )?;
        let item_id = transaction.last_insert_rowid();
        transaction.execute(
            "UPDATE runs
             SET completed_items = completed_items + 1,
                 converted = converted + CASE WHEN ? = 'converted' THEN 1 ELSE 0 END,
                 copied = copied + CASE WHEN ? = 'copied' THEN 1 ELSE 0 END,
                 skipped = skipped + CASE WHEN ? = 'skipped' THEN 1 ELSE 0 END,
                 conflicted = conflicted + CASE WHEN ? = 'conflict' THEN 1 ELSE 0 END,
                 failed = failed + CASE WHEN ? = 'failed' THEN 1 ELSE 0 END,
                 representative_artwork_id = COALESCE(representative_artwork_id, ?),
                 updated_at_ms = ?
             WHERE id = ?",
            params![
                item.outcome,
                item.outcome,
                item.outcome,
                item.outcome,
                item.outcome,
                artwork_id,
                now_ms(),
                run_id,
            ],
        )?;
        transaction.commit()?;
        Ok(item_id)
    }

    pub fn list_runs(&self, cursor: Option<i64>, limit: usize) -> HistoryResult<RunPage> {
        let limit = checked_limit(limit);
        let connection = self.connection.lock().expect("history connection lock");
        let mut statement = connection.prepare(
            "SELECT runs.id, input_kind, input_path, output_path, settings_json, status, display_name,
                    created_at_ms, updated_at_ms, finished_at_ms, total_items, completed_items,
                    converted, copied, skipped, conflicted, failed, representative_artwork_id,
                    terminal_error_code, terminal_error_detail,
                    (SELECT COUNT(*) FROM items
                     WHERE items.run_id = runs.id AND item_kind = 'music') AS music_count
             FROM runs
             WHERE (? IS NULL OR runs.id < ?)
             ORDER BY runs.id DESC LIMIT ?",
        )?;
        let mut rows = statement.query(params![cursor, cursor, (limit + 1) as i64])?;
        let mut runs = Vec::with_capacity(limit);
        let mut has_more = false;
        while let Some(row) = rows.next()? {
            if runs.len() == limit {
                has_more = true;
                break;
            }
            runs.push(run_from_row(row)?);
        }
        let next_cursor = if has_more {
            runs.last().map(|run| run.id)
        } else {
            None
        };
        Ok(RunPage { runs, next_cursor })
    }

    pub fn run(&self, run_id: i64) -> HistoryResult<Option<RunSummary>> {
        let connection = self.connection.lock().expect("history connection lock");
        run_by_id(&connection, run_id)
    }

    pub fn rename_run(&self, run_id: i64, display_name: &str) -> HistoryResult<Option<RunSummary>> {
        let display_name = display_name.trim();
        if display_name.is_empty() || display_name.chars().count() > 120 {
            return Err(HistoryError::InvalidDisplayName);
        }
        let connection = self.connection.lock().expect("history connection lock");
        let Some(run) = run_by_id(&connection, run_id)? else {
            return Ok(None);
        };
        if !run.status.is_terminal() {
            return Ok(None);
        }
        connection.execute(
            "UPDATE runs SET display_name = ?, updated_at_ms = ? WHERE id = ?",
            params![display_name, now_ms(), run_id],
        )?;
        run_by_id(&connection, run_id)
    }

    pub fn run_output_path(&self, run_id: i64) -> HistoryResult<Option<PathBuf>> {
        let connection = self.connection.lock().expect("history connection lock");
        connection
            .query_row(
                "SELECT output_path FROM runs WHERE id = ?",
                [run_id],
                |row| row.get::<_, String>(0).map(PathBuf::from),
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn music_output_path(&self, item_id: i64) -> HistoryResult<Option<PathBuf>> {
        let connection = self.connection.lock().expect("history connection lock");
        connection
            .query_row(
                "SELECT destination_path FROM items
                 WHERE id = ? AND item_kind = 'music'
                   AND outcome IN ('converted', 'copied', 'skipped')
                   AND destination_path IS NOT NULL",
                [item_id],
                |row| row.get::<_, String>(0).map(PathBuf::from),
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn editable_item(&self, item_id: i64) -> HistoryResult<Option<EditableItem>> {
        let connection = self.connection.lock().expect("history connection lock");
        connection
            .query_row(
                "SELECT id, destination_path, title, artist, album FROM items
             WHERE id = ? AND item_kind = 'music' AND outcome IN ('converted', 'copied', 'skipped')
               AND destination_path IS NOT NULL",
                [item_id],
                |row| {
                    Ok(EditableItem {
                        id: row.get(0)?,
                        destination_path: PathBuf::from(row.get::<_, String>(1)?),
                        title: row.get(2)?,
                        artist: row.get(3)?,
                        album: row.get(4)?,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn history_item_by_id(&self, item_id: i64) -> HistoryResult<Option<HistoryItem>> {
        let connection = self.connection.lock().expect("history connection lock");
        connection.query_row(
            "SELECT id, run_id, ordinal, item_kind, source_path, relative_path, destination_path,
                    action, outcome, logical_bytes, output_bytes, output_format,
                    error_code, error_detail, warnings_json, title, artist, album, duration_ms, artwork_id,
                    bitrate_kbps, media_probe_version, metadata_edit_pending FROM items WHERE id = ?",
            [item_id], item_from_row,
        ).optional().map_err(Into::into)
    }

    pub fn set_metadata_edit_pending(&self, item_id: i64, pending: bool) -> HistoryResult<()> {
        let connection = self.connection.lock().expect("history connection lock");
        connection.execute(
            "UPDATE items SET metadata_edit_pending = ? WHERE id = ?",
            params![i64::from(pending), item_id],
        )?;
        Ok(())
    }

    pub fn refresh_metadata_text(
        &self,
        item_id: i64,
        title: Option<String>,
        artist: Option<String>,
        album: Option<String>,
        artwork: Option<ArtworkInput>,
    ) -> HistoryResult<()> {
        let mut connection = self.connection.lock().expect("history connection lock");
        let transaction = connection.transaction()?;
        let artwork_id = artwork
            .as_ref()
            .map(|value| insert_or_get_artwork(&transaction, value))
            .transpose()?;
        transaction.execute("UPDATE items SET title = ?, artist = ?, album = ?, artwork_id = COALESCE(?, artwork_id), metadata_edit_pending = 0 WHERE id = ?", params![title, artist, album, artwork_id, item_id])?;
        if let Some(artwork_id) = artwork_id {
            transaction.execute("UPDATE runs SET representative_artwork_id = ? WHERE id = (SELECT run_id FROM items WHERE id = ?)", params![artwork_id, item_id])?;
        }
        cleanup_orphan_artwork(&transaction)?;
        transaction.commit()?;
        Ok(())
    }

    pub fn reconcile_item_from_output(
        &self,
        item_id: i64,
        path: &Path,
    ) -> HistoryResult<HistoryItem> {
        let snapshot = unlockm::metadata::tags::read_bounded_snapshot(path).map_err(|detail| {
            HistoryError::Io {
                path: path.into(),
                source: std::io::Error::other(detail),
            }
        })?;
        let (bytes, format, bitrate) =
            probe_output_facts(path).ok_or_else(|| HistoryError::Io {
                path: path.into(),
                source: std::io::Error::other("output facts unavailable"),
            })?;
        let title = snapshot.metadata.title.map(|v| v.text);
        let artist = snapshot.metadata.artists.map(|v| v.text);
        let album = snapshot.metadata.album.map(|v| v.text);
        let duration = snapshot.metadata.duration_ms;
        let artwork = snapshot
            .metadata
            .cover
            .and_then(|c| c.bytes)
            .and_then(|bytes| {
                artwork_mime(&bytes).map(|mime_type| ArtworkInput {
                    mime_type: mime_type.into(),
                    bytes,
                })
            });
        let mut connection = self.connection.lock().expect("history connection lock");
        let tx = connection.transaction()?;
        let artwork_id = artwork
            .as_ref()
            .map(|v| insert_or_get_artwork(&tx, v))
            .transpose()?;
        tx.execute("UPDATE items SET title=?,artist=?,album=?,duration_ms=?,artwork_id=?,output_bytes=?,output_format=?,bitrate_kbps=?,media_probe_version=1,metadata_edit_pending=0 WHERE id=?", params![title,artist,album,duration.map(i64::from),artwork_id,bytes as i64,format,bitrate.map(i64::from),item_id])?;
        tx.execute("UPDATE runs SET representative_artwork_id=(SELECT artwork_id FROM items WHERE run_id=(SELECT run_id FROM items WHERE id=?) AND artwork_id IS NOT NULL ORDER BY id DESC LIMIT 1) WHERE id=(SELECT run_id FROM items WHERE id=?)", params![item_id,item_id])?;
        cleanup_orphan_artwork(&tx)?;
        tx.commit()?;
        connection.query_row(
            "SELECT id, run_id, ordinal, item_kind, source_path, relative_path, destination_path,
                    action, outcome, logical_bytes, output_bytes, output_format,
                    error_code, error_detail, warnings_json, title, artist, album, duration_ms, artwork_id,
                    bitrate_kbps, media_probe_version, metadata_edit_pending FROM items WHERE id = ?",
            [item_id], item_from_row,
        ).map_err(Into::into)
    }

    pub fn artwork_candidates(
        &self,
        item_id: i64,
        query: Option<String>,
        limit: usize,
    ) -> HistoryResult<Vec<ArtworkCandidate>> {
        let connection = self.connection.lock().expect("history connection lock");
        let album: Option<String> = connection
            .query_row("SELECT album FROM items WHERE id = ?", [item_id], |row| {
                row.get::<_, Option<String>>(0)
            })
            .optional()?
            .flatten()
            .map(|value| value.trim().to_lowercase())
            .filter(|value| !value.is_empty());
        let needle = normalized_filter(query).map(|value| like_pattern(&value));
        let mut statement = connection.prepare(
            "SELECT artwork_id, album, artist, title FROM (
                SELECT artwork_id, album, artist, title, id,
                       CASE WHEN ?2 IS NOT NULL AND LOWER(TRIM(COALESCE(album, ''))) = ?2 THEN 0 ELSE 1 END AS album_rank,
                       ROW_NUMBER() OVER (
                           PARTITION BY artwork_id
                           ORDER BY CASE WHEN ?2 IS NOT NULL AND LOWER(TRIM(COALESCE(album, ''))) = ?2 THEN 0 ELSE 1 END, id DESC
                       ) AS artwork_rank
                FROM items
                WHERE artwork_id IS NOT NULL
                  AND (?1 IS NULL OR title LIKE ?1 ESCAPE '\\' COLLATE NOCASE
                       OR album LIKE ?1 ESCAPE '\\' COLLATE NOCASE)
             ) WHERE artwork_rank = 1
             ORDER BY album_rank, id DESC LIMIT ?3",
        )?;
        let rows =
            statement.query_map(params![needle, album, checked_limit(limit) as i64], |row| {
                Ok(ArtworkCandidate {
                    artwork_id: row.get(0)?,
                    album: row.get(1)?,
                    artist: row.get(2)?,
                    title: row.get(3)?,
                })
            })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn query_music_items(&self, run_id: i64, query: ItemQuery) -> HistoryResult<ItemPage> {
        let page = query.page.max(1);
        let page_size = if query.page_size == 0 {
            25
        } else {
            checked_limit(query.page_size)
        };
        let search = normalized_filter(query.search).map(|value| like_pattern(&value));
        let artist = normalized_filter(query.artist).map(|value| like_pattern(&value));
        let album = normalized_filter(query.album).map(|value| like_pattern(&value));
        let outcome = query.outcome.map(OutcomeFilter::as_str);
        let mut connection = self.connection.lock().expect("history connection lock");
        repair_legacy_batch(&connection, run_id);
        let repair_pending: bool = connection.query_row("SELECT EXISTS(SELECT 1 FROM items WHERE run_id = ? AND action = 'copy' AND media_probe_version < 1)", [run_id], |row| row.get(0))?;
        let companion_count: i64 = connection.query_row(
            "SELECT COUNT(*) FROM items WHERE run_id = ? AND item_kind = 'companion'",
            [run_id],
            |row| row.get(0),
        )?;
        let music_total: i64 = connection.query_row(
            "SELECT COUNT(*) FROM items WHERE run_id = ? AND item_kind = 'music'",
            [run_id],
            |row| row.get(0),
        )?;
        let filter_sql = "run_id = ?1 AND item_kind = 'music'
             AND (?2 IS NULL OR title LIKE ?2 ESCAPE '\\' COLLATE NOCASE
                  OR relative_path LIKE ?2 ESCAPE '\\' COLLATE NOCASE)
             AND (?3 IS NULL OR artist LIKE ?3 ESCAPE '\\' COLLATE NOCASE)
             AND (?4 IS NULL OR album LIKE ?4 ESCAPE '\\' COLLATE NOCASE)
             AND (?5 IS NULL OR outcome = ?5)";
        let filtered_total: i64 = connection.query_row(
            &format!("SELECT COUNT(*) FROM items WHERE {filter_sql}"),
            params![run_id, search, artist, album, outcome],
            |row| row.get(0),
        )?;
        let page_count = (filtered_total.max(0) as usize).div_ceil(page_size);
        let page = if page_count == 0 {
            1
        } else {
            page.min(page_count)
        };
        let offset = (page - 1).saturating_mul(page_size);
        let order_sql = format!(
            "{} {}, ordinal {}",
            query.sort.sql(),
            query.direction.sql(),
            query.direction.sql()
        );
        let sql = format!(
            "SELECT id, run_id, ordinal, item_kind, source_path, relative_path, destination_path,
                    action, outcome, logical_bytes, output_bytes, output_format,
                    error_code, error_detail, warnings_json, title, artist, album, duration_ms, artwork_id,
                    bitrate_kbps, media_probe_version, metadata_edit_pending
             FROM items WHERE {filter_sql}
             ORDER BY {order_sql} LIMIT ?6 OFFSET ?7"
        );
        let mut statement = connection.prepare(&sql)?;
        let mut rows = statement.query(params![
            run_id,
            search,
            artist,
            album,
            outcome,
            page_size as i64,
            offset as i64,
        ])?;
        let mut items = Vec::with_capacity(page_size);
        while let Some(row) = rows.next()? {
            items.push(item_from_row(row)?);
        }
        drop(rows);
        drop(statement);
        repair_page_facts(&mut connection, &mut items);
        Ok(ItemPage {
            items,
            page,
            page_size,
            page_count,
            filtered_total: filtered_total.max(0) as usize,
            companion_count: companion_count.max(0) as usize,
            music_total: music_total.max(0) as usize,
            repair_pending,
        })
    }

    pub fn list_music_facets(
        &self,
        run_id: i64,
        facet: FacetKind,
        search: Option<String>,
        cursor: Option<String>,
        limit: usize,
    ) -> HistoryResult<FacetPage> {
        let limit = checked_limit(limit);
        let search = normalized_filter(search).map(|value| like_pattern(&value));
        let cursor = normalized_filter(cursor);
        let column = facet.sql();
        let sql = format!(
            "SELECT DISTINCT {column} FROM items
             WHERE run_id = ?1 AND item_kind = 'music'
               AND {column} IS NOT NULL AND TRIM({column}) != ''
               AND (?2 IS NULL OR {column} LIKE ?2 ESCAPE '\\' COLLATE NOCASE)
               AND (?3 IS NULL OR {column} > ?3 COLLATE NOCASE)
             ORDER BY {column} COLLATE NOCASE ASC LIMIT ?4"
        );
        let connection = self.connection.lock().expect("history connection lock");
        let mut statement = connection.prepare(&sql)?;
        let mut rows = statement.query(params![run_id, search, cursor, (limit + 1) as i64])?;
        let mut values = Vec::with_capacity(limit);
        let mut has_more = false;
        while let Some(row) = rows.next()? {
            if values.len() == limit {
                has_more = true;
                break;
            }
            values.push(row.get(0)?);
        }
        let next_cursor = has_more.then(|| values.last().cloned()).flatten();
        Ok(FacetPage {
            values,
            next_cursor,
        })
    }

    pub fn artwork(&self, artwork_id: i64) -> HistoryResult<Option<ArtworkRecord>> {
        let connection = self.connection.lock().expect("history connection lock");
        connection
            .query_row(
                "SELECT id, mime_type, bytes FROM artwork WHERE id = ?",
                [artwork_id],
                |row| {
                    Ok(ArtworkRecord {
                        id: row.get(0)?,
                        mime_type: row.get(1)?,
                        bytes: row.get(2)?,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn artwork_mime(&self, artwork_id: i64) -> HistoryResult<Option<(i64, String)>> {
        let connection = self.connection.lock().expect("history connection lock");
        connection
            .query_row(
                "SELECT id, mime_type FROM artwork WHERE id = ?",
                [artwork_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn delete_run(&self, run_id: i64) -> HistoryResult<bool> {
        let mut connection = self.connection.lock().expect("history connection lock");
        let transaction = connection.transaction()?;
        let deleted = transaction.execute("DELETE FROM runs WHERE id = ?", [run_id])? > 0;
        cleanup_orphan_artwork(&transaction)?;
        transaction.commit()?;
        Ok(deleted)
    }

    pub fn clear_history(&self) -> HistoryResult<()> {
        let mut connection = self.connection.lock().expect("history connection lock");
        let transaction = connection.transaction()?;
        transaction.execute("DELETE FROM runs", [])?;
        cleanup_orphan_artwork(&transaction)?;
        transaction.commit()?;
        Ok(())
    }

    pub fn preferences(&self) -> HistoryResult<Preferences> {
        let connection = self.connection.lock().expect("history connection lock");
        let value: Option<String> = connection
            .query_row(
                "SELECT value_json FROM preferences WHERE key = 'preferences'",
                [],
                |row| row.get(0),
            )
            .optional()?;
        let preferences = value
            .map(|value| serde_json::from_str::<Preferences>(&value).map_err(HistoryError::from))
            .transpose()
            .map(|value| value.unwrap_or_default())?;
        Ok(sanitize_preferences(preferences))
    }

    pub fn set_preferences(&self, preferences: &Preferences) -> HistoryResult<Preferences> {
        let preferences = sanitize_preferences(preferences.clone());
        let connection = self.connection.lock().expect("history connection lock");
        connection.execute(
            "INSERT INTO preferences (key, value_json, updated_at_ms)
             VALUES ('preferences', ?, ?)
             ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json,
                                           updated_at_ms = excluded.updated_at_ms",
            params![serde_json::to_string(&preferences)?, now_ms()],
        )?;
        Ok(preferences)
    }

    pub fn recover_interrupted(&self) -> HistoryResult<usize> {
        let connection = self.connection.lock().expect("history connection lock");
        connection
            .execute(
                "UPDATE runs
                 SET status = 'interrupted', updated_at_ms = ?, finished_at_ms = ?,
                     terminal_error_code = 'interrupted',
                     terminal_error_detail = 'UnlockM stopped before this conversion finished'
                 WHERE status IN ('planning', 'running')",
                params![now_ms(), now_ms()],
            )
            .map_err(Into::into)
    }

    fn update_status(
        &self,
        run_id: i64,
        status: RunStatus,
        error_code: Option<&str>,
        error_detail: Option<&str>,
        terminal: bool,
    ) -> HistoryResult<()> {
        let connection = self.connection.lock().expect("history connection lock");
        connection.execute(
            "UPDATE runs
             SET status = ?, updated_at_ms = ?,
                 finished_at_ms = CASE WHEN ? THEN ? ELSE finished_at_ms END,
                 terminal_error_code = ?, terminal_error_detail = ?
             WHERE id = ?",
            params![
                status.as_str(),
                now_ms(),
                terminal,
                now_ms(),
                error_code,
                error_detail,
                run_id
            ],
        )?;
        Ok(())
    }

    #[cfg(test)]
    pub fn execute_test_sql(&self, statement: &str) -> HistoryResult<()> {
        self.connection
            .lock()
            .expect("history connection lock")
            .execute_batch(statement)?;
        Ok(())
    }
}

fn migrate(connection: &Connection) -> HistoryResult<()> {
    let mut version: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if version >= SCHEMA_VERSION {
        return Ok(());
    }
    if version < 1 {
        let transaction = connection.unchecked_transaction()?;
        transaction.execute_batch(
            "CREATE TABLE IF NOT EXISTS runs (
            id INTEGER PRIMARY KEY,
            input_kind TEXT NOT NULL,
            input_path TEXT NOT NULL,
            output_path TEXT NOT NULL,
            settings_json TEXT NOT NULL,
            status TEXT NOT NULL,
            created_at_ms INTEGER NOT NULL,
            updated_at_ms INTEGER NOT NULL,
            finished_at_ms INTEGER,
            total_items INTEGER NOT NULL DEFAULT 0,
            completed_items INTEGER NOT NULL DEFAULT 0,
            converted INTEGER NOT NULL DEFAULT 0,
            copied INTEGER NOT NULL DEFAULT 0,
            skipped INTEGER NOT NULL DEFAULT 0,
            conflicted INTEGER NOT NULL DEFAULT 0,
            failed INTEGER NOT NULL DEFAULT 0,
            representative_artwork_id INTEGER,
            terminal_error_code TEXT,
            terminal_error_detail TEXT,
            app_version TEXT NOT NULL
         );
         CREATE TABLE IF NOT EXISTS artwork (
            id INTEGER PRIMARY KEY,
            sha256 TEXT NOT NULL UNIQUE,
            mime_type TEXT NOT NULL,
            bytes BLOB NOT NULL,
            byte_len INTEGER NOT NULL,
            created_at_ms INTEGER NOT NULL
         );
         CREATE TABLE IF NOT EXISTS items (
            id INTEGER PRIMARY KEY,
            run_id INTEGER NOT NULL REFERENCES runs(id) ON DELETE CASCADE,
            ordinal INTEGER NOT NULL,
            item_kind TEXT NOT NULL,
            source_path TEXT NOT NULL,
            relative_path TEXT NOT NULL,
            destination_path TEXT,
            action TEXT,
            outcome TEXT NOT NULL,
            logical_bytes INTEGER NOT NULL,
            preflight_ms INTEGER NOT NULL,
            decrypt_ms INTEGER NOT NULL,
            metadata_resolve_ms INTEGER NOT NULL,
            tag_write_ms INTEGER NOT NULL,
            fallback_ms INTEGER NOT NULL,
            copy_ms INTEGER NOT NULL,
            publish_ms INTEGER NOT NULL,
            error_code TEXT,
            error_detail TEXT,
            warnings_json TEXT NOT NULL,
            title TEXT,
            artist TEXT,
            album TEXT,
            duration_ms INTEGER,
            artwork_id INTEGER REFERENCES artwork(id) ON DELETE SET NULL,
            UNIQUE(run_id, ordinal)
         );
         CREATE INDEX IF NOT EXISTS items_run_music_ordinal ON items(run_id, item_kind, ordinal);
         CREATE TABLE IF NOT EXISTS preferences (
            key TEXT PRIMARY KEY,
            value_json TEXT NOT NULL,
            updated_at_ms INTEGER NOT NULL
         );",
        )?;
        transaction.pragma_update(None, "user_version", 1)?;
        transaction.commit()?;
        version = 1;
    }
    if version < 2 {
        let transaction = connection.unchecked_transaction()?;
        transaction.execute_batch(
            "ALTER TABLE items ADD COLUMN output_bytes INTEGER;
             ALTER TABLE items ADD COLUMN output_format TEXT;
             CREATE INDEX IF NOT EXISTS items_run_music_artist
               ON items(run_id, item_kind, artist COLLATE NOCASE, ordinal);
             CREATE INDEX IF NOT EXISTS items_run_music_album
               ON items(run_id, item_kind, album COLLATE NOCASE, ordinal);",
        )?;
        transaction.pragma_update(None, "user_version", 2)?;
        transaction.commit()?;
        version = 2;
    }
    if version < 3 {
        let transaction = connection.unchecked_transaction()?;
        transaction.execute_batch(
            "ALTER TABLE runs ADD COLUMN display_name TEXT;
             ALTER TABLE items ADD COLUMN bitrate_kbps INTEGER;
             ALTER TABLE items ADD COLUMN media_probe_version INTEGER NOT NULL DEFAULT 0;
             ALTER TABLE items ADD COLUMN metadata_edit_pending INTEGER NOT NULL DEFAULT 0;
             CREATE INDEX IF NOT EXISTS items_run_probe_pending
               ON items(run_id, item_kind, media_probe_version, metadata_edit_pending, ordinal);",
        )?;
        transaction.pragma_update(None, "user_version", SCHEMA_VERSION)?;
        transaction.commit()?;
    }
    Ok(())
}

fn insert_or_get_artwork(
    transaction: &Transaction<'_>,
    artwork: &ArtworkInput,
) -> HistoryResult<i64> {
    if artwork.bytes.len() > MAX_ARTWORK_BYTES {
        return Err(HistoryError::ArtworkTooLarge);
    }
    let digest = hex::encode(Sha256::digest(&artwork.bytes));
    transaction.execute(
        "INSERT INTO artwork (sha256, mime_type, bytes, byte_len, created_at_ms)
         VALUES (?, ?, ?, ?, ?)
         ON CONFLICT(sha256) DO NOTHING",
        params![
            digest,
            artwork.mime_type,
            artwork.bytes,
            artwork.bytes.len() as i64,
            now_ms()
        ],
    )?;
    transaction
        .query_row("SELECT id FROM artwork WHERE sha256 = ?", [digest], |row| {
            row.get(0)
        })
        .map_err(Into::into)
}

fn cleanup_orphan_artwork(transaction: &Transaction<'_>) -> HistoryResult<()> {
    transaction.execute(
        "DELETE FROM artwork WHERE id NOT IN (
            SELECT artwork_id FROM items WHERE artwork_id IS NOT NULL
        )",
        [],
    )?;
    Ok(())
}

fn run_by_id(connection: &Connection, id: i64) -> HistoryResult<Option<RunSummary>> {
    connection
        .query_row(
            "SELECT runs.id, input_kind, input_path, output_path, settings_json, status, display_name,
                    created_at_ms, updated_at_ms, finished_at_ms, total_items, completed_items,
                    converted, copied, skipped, conflicted, failed, representative_artwork_id,
                    terminal_error_code, terminal_error_detail,
                    (SELECT COUNT(*) FROM items
                     WHERE items.run_id = runs.id AND item_kind = 'music') AS music_count
             FROM runs WHERE runs.id = ?",
            [id],
            run_from_row,
        )
        .optional()
        .map_err(Into::into)
}

fn run_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<RunSummary> {
    let settings_json: String = row.get(4)?;
    let settings = serde_json::from_str(&settings_json).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(4, rusqlite::types::Type::Text, Box::new(error))
    })?;
    Ok(RunSummary {
        id: row.get(0)?,
        display_name: row.get::<_, Option<String>>(6)?.unwrap_or_else(|| {
            Path::new(&row.get::<_, String>(2).expect("input path selected"))
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("UnlockM conversion")
                .to_owned()
        }),
        input_kind: if row.get::<_, String>(1)? == "directory" {
            InputKind::Directory
        } else {
            InputKind::File
        },
        input_path: PathBuf::from(row.get::<_, String>(2)?),
        output_path: PathBuf::from(row.get::<_, String>(3)?),
        settings,
        status: RunStatus::parse(&row.get::<_, String>(5)?),
        created_at_ms: row.get(7)?,
        updated_at_ms: row.get(8)?,
        finished_at_ms: row.get(9)?,
        total_items: row.get::<_, i64>(10)? as usize,
        completed_items: row.get::<_, i64>(11)? as usize,
        converted: row.get::<_, i64>(12)? as usize,
        copied: row.get::<_, i64>(13)? as usize,
        skipped: row.get::<_, i64>(14)? as usize,
        conflicted: row.get::<_, i64>(15)? as usize,
        failed: row.get::<_, i64>(16)? as usize,
        music_count: row.get::<_, i64>(20)? as usize,
        representative_artwork_id: row.get(17)?,
        terminal_error_code: row.get(18)?,
        terminal_error_detail: row.get(19)?,
    })
}

fn item_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<HistoryItem> {
    let warnings_json: String = row.get(14)?;
    let warnings = serde_json::from_str(&warnings_json).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(14, rusqlite::types::Type::Text, Box::new(error))
    })?;
    Ok(HistoryItem {
        id: row.get(0)?,
        run_id: row.get(1)?,
        ordinal: row.get::<_, i64>(2)? as u64,
        item_kind: StoredItemKind::parse(&row.get::<_, String>(3)?),
        source_path: PathBuf::from(row.get::<_, String>(4)?),
        relative_path: PathBuf::from(row.get::<_, String>(5)?),
        destination_path: row.get::<_, Option<String>>(6)?.map(PathBuf::from),
        action: row.get(7)?,
        outcome: row.get(8)?,
        logical_bytes: row.get::<_, i64>(9)? as u64,
        output_bytes: row.get::<_, Option<i64>>(10)?.map(|value| value as u64),
        output_format: row.get(11)?,
        bitrate_kbps: row.get::<_, Option<i64>>(20)?.map(|value| value as u32),
        media_probe_version: row.get(21)?,
        metadata_edit_pending: row.get::<_, i64>(22)? != 0,
        error_code: row.get(12)?,
        error_detail: row.get(13)?,
        warnings,
        title: row.get(15)?,
        artist: row.get(16)?,
        album: row.get(17)?,
        duration_ms: row.get::<_, Option<i64>>(18)?.map(|value| value as u32),
        artwork_id: row.get(19)?,
    })
}

fn normalized_filter(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn like_pattern(value: &str) -> String {
    let escaped = value
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    format!("%{escaped}%")
}

fn reconcile_pending_item(connection: &mut Connection, item: &mut HistoryItem) {
    let Some(destination) = item.destination_path.as_deref() else {
        return;
    };
    let Ok(snapshot) = unlockm::metadata::tags::read_bounded_snapshot(destination) else {
        return;
    };
    let Some((bytes, format, bitrate_kbps)) = probe_output_facts(destination) else {
        return;
    };
    let artwork = snapshot
        .metadata
        .cover
        .and_then(|cover| cover.bytes)
        .and_then(|bytes| {
            artwork_mime(&bytes).map(|mime_type| ArtworkInput {
                mime_type: mime_type.into(),
                bytes,
            })
        });
    let title = snapshot.metadata.title.map(|value| value.text);
    let artist = snapshot.metadata.artists.map(|value| value.text);
    let album = snapshot.metadata.album.map(|value| value.text);
    let duration_ms = snapshot.metadata.duration_ms;
    let Ok(transaction) = connection.transaction() else {
        return;
    };
    let artwork_id = artwork
        .as_ref()
        .map(|value| insert_or_get_artwork(&transaction, value))
        .transpose();
    let Ok(artwork_id) = artwork_id else { return };
    if transaction.execute(
        "UPDATE items SET title=?, artist=?, album=?, duration_ms=?, artwork_id=?, output_bytes=?, output_format=?, bitrate_kbps=?, media_probe_version=1, metadata_edit_pending=0 WHERE id=?",
        params![title.clone(), artist.clone(), album.clone(), duration_ms.map(i64::from), artwork_id, bytes as i64, format.clone(), bitrate_kbps.map(i64::from), item.id],
    ).is_err() { return; }
    let _ = transaction.execute("UPDATE runs SET representative_artwork_id=(SELECT artwork_id FROM items WHERE run_id=? AND artwork_id IS NOT NULL ORDER BY id DESC LIMIT 1) WHERE id=?", params![item.run_id, item.run_id]);
    let _ = cleanup_orphan_artwork(&transaction);
    if transaction.commit().is_err() {
        return;
    }
    item.title = title;
    item.artist = artist;
    item.album = album;
    item.duration_ms = duration_ms;
    item.artwork_id = artwork_id;
    item.output_bytes = Some(bytes);
    item.output_format = format;
    item.bitrate_kbps = bitrate_kbps;
    item.media_probe_version = 1;
    item.metadata_edit_pending = false;
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

fn repair_page_facts(connection: &mut Connection, items: &mut [HistoryItem]) {
    for item in items {
        if item.metadata_edit_pending {
            reconcile_pending_item(connection, item);
        }
        if item.output_bytes.is_none()
            && matches!(item.outcome.as_str(), "converted" | "skipped")
            && let Some(destination) = item.destination_path.as_deref()
            && let Some((bytes, format, bitrate_kbps)) = probe_output_facts(destination)
            && connection
                .execute(
                    "UPDATE items SET output_bytes = ?, output_format = ?, bitrate_kbps = ?, media_probe_version = 1 WHERE id = ?",
                    params![bytes as i64, format, bitrate_kbps.map(i64::from), item.id],
                )
                .is_ok()
        {
            item.output_bytes = Some(bytes);
            item.output_format = format;
            item.bitrate_kbps = bitrate_kbps;
            item.media_probe_version = 1;
        }

        if item.artist.is_none() || item.title.is_none() {
            let mut title = item.title.clone();
            let mut artist = item.artist.clone();
            let mut album = item.album.clone();
            let mut duration_ms = item.duration_ms;
            if item
                .source_path
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("ncm"))
                && let Ok(container) = unlockm::metadata::ncm::parse(&item.source_path)
            {
                title = container.title.map(|value| value.text).or(title);
                artist = container.artists.map(|value| value.text).or(artist);
                album = container.album.map(|value| value.text).or(album);
                duration_ms = container.duration_ms.or(duration_ms);
            }
            if (artist.is_none() || title.is_none())
                && let Some(stem) = item
                    .source_path
                    .file_stem()
                    .and_then(|value| value.to_str())
                && let Some((filename_artist, filename_title)) = stem.split_once(" - ")
            {
                let filename_artist = filename_artist.trim();
                let filename_title = filename_title.trim();
                if !filename_artist.is_empty() && !filename_title.is_empty() {
                    artist.get_or_insert_with(|| filename_artist.to_owned());
                    title.get_or_insert_with(|| filename_title.to_owned());
                }
            }
            if (title != item.title || artist != item.artist || album != item.album)
                && connection
                    .execute(
                        "UPDATE items SET title = ?, artist = ?, album = ?, duration_ms = ? WHERE id = ?",
                        params![title, artist, album, duration_ms.map(i64::from), item.id],
                    )
                    .is_ok()
            {
                item.title = title;
                item.artist = artist;
                item.album = album;
                item.duration_ms = duration_ms;
            }
        }
    }
}

/// Repairs a small, deterministic group on each detail read. This lets copied
/// standard music from pre-v3 history become playable without an unbounded
/// startup scan or trusting a filename extension.
fn repair_legacy_batch(connection: &Connection, run_id: i64) {
    let mut statement = match connection.prepare(
        "SELECT id, destination_path FROM items
         WHERE run_id = ? AND action = 'copy' AND media_probe_version < 1
           AND destination_path IS NOT NULL ORDER BY ordinal LIMIT ?",
    ) {
        Ok(statement) => statement,
        Err(_) => return,
    };
    let rows = match statement.query_map(params![run_id, LEGACY_REPAIR_BATCH as i64], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            PathBuf::from(row.get::<_, String>(1)?),
        ))
    }) {
        Ok(rows) => rows,
        Err(_) => return,
    };
    for row in rows.flatten() {
        let (id, destination) = row;
        match probe_output_facts(&destination) {
            Some((bytes, format @ Some(_), bitrate)) => {
                let _ = connection.execute("UPDATE items SET item_kind = 'music', output_bytes = ?, output_format = ?, bitrate_kbps = ?, media_probe_version = 1 WHERE id = ?", params![bytes as i64, format, bitrate.map(i64::from), id]);
            }
            _ => {
                let _ = connection.execute(
                    "UPDATE items SET media_probe_version = 1 WHERE id = ?",
                    [id],
                );
            }
        }
    }
}

fn probe_output_facts(path: &Path) -> Option<(u64, Option<String>, Option<u32>)> {
    let metadata = fs::symlink_metadata(path).ok()?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return None;
    }
    let format = unlockm::formats::audio::probe_path(path).ok()?;
    let bitrate_kbps = unlockm::metadata::tags::read_bounded_snapshot(path)
        .ok()
        .and_then(|snapshot| snapshot.bitrate_kbps);
    Some((metadata.len(), format, bitrate_kbps))
}

fn checked_limit(limit: usize) -> usize {
    limit.clamp(1, MAX_PAGE_SIZE)
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before Unix epoch")
        .as_millis()
        .try_into()
        .unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings() -> RunSettings {
        RunSettings {
            update_metadata: false,
            copy_passthrough: true,
            overwrite: false,
        }
    }

    fn item(ordinal: u64, artwork: Option<ArtworkInput>) -> PersistedItemInput {
        PersistedItemInput {
            ordinal,
            item_kind: StoredItemKind::Music,
            source_path: format!("/input/{ordinal}.ncm").into(),
            relative_path: format!("{ordinal}.ncm").into(),
            destination_path: Some(format!("/output/{ordinal}.flac").into()),
            action: Some("convert".into()),
            outcome: "converted".into(),
            logical_bytes: 10,
            output_bytes: Some(20),
            output_format: Some("flac".into()),
            bitrate_kbps: None,
            preflight_ms: 1,
            decrypt_ms: 2,
            metadata_resolve_ms: 0,
            tag_write_ms: 0,
            fallback_ms: 0,
            copy_ms: 0,
            publish_ms: 1,
            error_code: None,
            error_detail: None,
            warnings: Vec::new(),
            title: Some(format!("Track {ordinal}")),
            artist: None,
            album: None,
            duration_ms: Some(1_000),
            artwork,
        }
    }

    fn store() -> (tempfile::TempDir, HistoryStore) {
        let directory = tempfile::tempdir().unwrap();
        let store = HistoryStore::open(directory.path().join("history.sqlite")).unwrap();
        (directory, store)
    }

    #[test]
    fn migration_reopen_and_preferences_are_idempotent() {
        let directory = tempfile::tempdir().unwrap();
        let database = directory.path().join("history.sqlite");
        let store = HistoryStore::open(&database).unwrap();
        let preferences = Preferences {
            locale: "zh-CN".into(),
            theme: Theme::Dark,
            palette: Palette::Blue,
            player_volume: 0.6,
            default_settings: settings(),
        };
        store.set_preferences(&preferences).unwrap();
        drop(store);
        let reopened = HistoryStore::open(database).unwrap();
        assert_eq!(reopened.preferences().unwrap(), preferences);
        assert_eq!(reopened.list_runs(None, 50).unwrap().runs, Vec::new());
    }

    #[test]
    fn legacy_preferences_without_theme_default_to_light() {
        let (_directory, store) = store();
        store
            .execute_test_sql(
                "INSERT INTO preferences (key, value_json, updated_at_ms)
                 VALUES ('preferences',
                   '{\"locale\":\"zh-CN\",\"defaultSettings\":{\"updateMetadata\":false,\"copyPassthrough\":true,\"overwrite\":false}}',
                   1)",
            )
            .unwrap();

        let preferences = store.preferences().unwrap();
        assert_eq!(preferences.locale, "zh-CN");
        assert_eq!(preferences.theme, Theme::Light);
        assert_eq!(preferences.palette, Palette::Blue);
        assert_eq!(preferences.player_volume, 0.6);
    }

    #[test]
    fn invalid_player_volume_falls_back_but_explicit_mute_is_preserved() {
        let (_directory, store) = store();
        let preferences = Preferences {
            player_volume: -0.1,
            ..Preferences::default()
        };
        assert_eq!(
            store.set_preferences(&preferences).unwrap().player_volume,
            0.6
        );

        let muted = Preferences {
            player_volume: 0.0,
            ..preferences
        };
        store.set_preferences(&muted).unwrap();
        assert_eq!(store.preferences().unwrap().player_volume, 0.0);
    }

    #[test]
    fn terminal_runs_can_be_renamed_but_active_runs_cannot() {
        let (_directory, store) = store();
        let run = store
            .create_run(CreateRun {
                input_kind: InputKind::File,
                input_path: "/input/album.ncm".into(),
                output_path: "/output".into(),
                settings: settings(),
                total_items: 0,
            })
            .unwrap();
        assert!(store.rename_run(run.id, "New name").unwrap().is_none());
        store
            .finish_run(run.id, RunStatus::Completed, None, None)
            .unwrap();
        assert_eq!(
            store
                .rename_run(run.id, " New name ")
                .unwrap()
                .unwrap()
                .display_name,
            "New name"
        );
        assert!(matches!(
            store.rename_run(run.id, ""),
            Err(HistoryError::InvalidDisplayName)
        ));
    }

    #[test]
    fn legacy_copy_repair_promotes_only_content_verified_music() {
        use base64::Engine;
        let (directory, store) = store();
        let output = directory.path().join("copied-without-extension");
        let audio = base64::engine::general_purpose::STANDARD
            .decode(include_str!("../../../../tests/fixtures/lofty-silent.mp3.b64").trim())
            .unwrap();
        fs::write(&output, audio).unwrap();
        let run = store
            .create_run(CreateRun {
                input_kind: InputKind::File,
                input_path: directory.path().join("source.mp3"),
                output_path: directory.path().to_path_buf(),
                settings: settings(),
                total_items: 1,
            })
            .unwrap();
        let mut copied = item(1, None);
        copied.item_kind = StoredItemKind::Companion;
        copied.action = Some("copy".into());
        copied.outcome = "copied".into();
        copied.destination_path = Some(output);
        let item_id = store.record_item(run.id, copied).unwrap();
        store
            .execute_test_sql(&format!(
                "UPDATE items
                 SET media_probe_version = 0, output_bytes = NULL,
                     output_format = NULL, bitrate_kbps = NULL
                 WHERE id = {item_id};"
            ))
            .unwrap();
        let page = store
            .query_music_items(run.id, ItemQuery::default())
            .unwrap();
        assert_eq!(page.music_total, 1);
        assert_eq!(page.items[0].output_format.as_deref(), Some("mp3"));
    }

    #[test]
    fn pending_metadata_recovers_real_disk_tags_but_not_missing_outputs() {
        use base64::Engine;
        let (directory, store) = store();
        let output = directory.path().join("pending.mp3");
        let audio = base64::engine::general_purpose::STANDARD
            .decode(include_str!("../../../../tests/fixtures/lofty-silent.mp3.b64").trim())
            .unwrap();
        fs::write(&output, audio).unwrap();
        let run = store
            .create_run(CreateRun {
                input_kind: InputKind::File,
                input_path: directory.path().join("source.mp3"),
                output_path: directory.path().to_path_buf(),
                settings: settings(),
                total_items: 2,
            })
            .unwrap();
        let mut persisted = item(1, None);
        persisted.destination_path = Some(output.clone());
        let item_id = store.record_item(run.id, persisted).unwrap();
        let metadata = unlockm::metadata::Metadata {
            title: Some(unlockm::metadata::MetadataValue::new(
                "Recovered title",
                unlockm::metadata::Provenance::ExistingTag,
            )),
            ..Default::default()
        };
        unlockm::metadata::tags::write(&output, &metadata, None).unwrap();
        store.set_metadata_edit_pending(item_id, true).unwrap();
        let page = store
            .query_music_items(run.id, ItemQuery::default())
            .unwrap();
        assert_eq!(page.items[0].title.as_deref(), Some("Recovered title"));
        assert!(!page.items[0].metadata_edit_pending);

        let mut missing = item(2, None);
        missing.destination_path = Some(directory.path().join("missing.mp3"));
        let missing_id = store.record_item(run.id, missing).unwrap();
        store.set_metadata_edit_pending(missing_id, true).unwrap();
        let page = store
            .query_music_items(run.id, ItemQuery::default())
            .unwrap();
        assert!(
            page.items
                .iter()
                .any(|value| value.id == missing_id && value.metadata_edit_pending)
        );
    }

    #[test]
    fn pending_recovery_reconciles_flac_artwork_and_output_facts() {
        use base64::Engine;
        let (directory, store) = store();
        let output = directory.path().join("pending.flac");
        let audio = base64::engine::general_purpose::STANDARD
            .decode(include_str!("../../../../tests/fixtures/lofty-full_test.flac.b64").trim())
            .unwrap();
        fs::write(&output, audio).unwrap();
        let run = store
            .create_run(CreateRun {
                input_kind: InputKind::File,
                input_path: directory.path().join("source.flac"),
                output_path: directory.path().to_path_buf(),
                settings: settings(),
                total_items: 1,
            })
            .unwrap();
        let mut persisted = item(1, None);
        persisted.destination_path = Some(output.clone());
        let item_id = store.record_item(run.id, persisted).unwrap();
        let cover = base64::engine::general_purpose::STANDARD.decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVQIHWP4z8DwHwAFgAI/ScLq6QAAAABJRU5ErkJggg==").unwrap();
        let metadata = unlockm::metadata::Metadata {
            title: Some(unlockm::metadata::MetadataValue::new(
                "Recovered FLAC",
                unlockm::metadata::Provenance::ExistingTag,
            )),
            ..Default::default()
        };
        unlockm::metadata::tags::write(&output, &metadata, Some(&cover)).unwrap();
        store.set_metadata_edit_pending(item_id, true).unwrap();
        let page = store
            .query_music_items(run.id, ItemQuery::default())
            .unwrap();
        let recovered = &page.items[0];
        assert_eq!(recovered.title.as_deref(), Some("Recovered FLAC"));
        assert!(!recovered.metadata_edit_pending);
        assert_eq!(recovered.output_format.as_deref(), Some("flac"));
        assert!(recovered.output_bytes.is_some() && recovered.bitrate_kbps.is_some());
        assert!(recovered.artwork_id.is_some());
        assert_eq!(
            store
                .run(run.id)
                .unwrap()
                .unwrap()
                .representative_artwork_id,
            recovered.artwork_id
        );
    }

    #[test]
    fn artwork_candidates_dedupe_sha_across_runs_and_prioritize_album() {
        let (_directory, store) = store();
        let first = store
            .create_run(CreateRun {
                input_kind: InputKind::File,
                input_path: "/input/one.ncm".into(),
                output_path: "/output".into(),
                settings: settings(),
                total_items: 1,
            })
            .unwrap();
        let second = store
            .create_run(CreateRun {
                input_kind: InputKind::File,
                input_path: "/input/two.ncm".into(),
                output_path: "/output".into(),
                settings: settings(),
                total_items: 1,
            })
            .unwrap();
        let cover = ArtworkInput {
            mime_type: "image/png".into(),
            bytes: b"same-cover".to_vec(),
        };
        let mut first_item = item(1, Some(cover.clone()));
        first_item.album = Some("Other album".into());
        let first_id = store.record_item(first.id, first_item).unwrap();
        let mut second_item = item(1, Some(cover));
        second_item.album = Some("Target album".into());
        let second_id = store.record_item(second.id, second_item).unwrap();
        let candidates = store.artwork_candidates(second_id, None, 12).unwrap();
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].album.as_deref(), Some("Target album"));
        let first_page = store
            .query_music_items(first.id, ItemQuery::default())
            .unwrap();
        let second_page = store
            .query_music_items(second.id, ItemQuery::default())
            .unwrap();
        assert_eq!(
            first_page.items[0].artwork_id,
            second_page.items[0].artwork_id
        );
        assert_ne!(first_id, second_id);
    }

    #[test]
    fn metadata_refresh_syncs_representative_artwork_and_deletes_orphans() {
        let (_directory, store) = store();
        let run = store
            .create_run(CreateRun {
                input_kind: InputKind::File,
                input_path: "/input/cover.ncm".into(),
                output_path: "/output".into(),
                settings: settings(),
                total_items: 1,
            })
            .unwrap();
        let item_id = store.record_item(run.id, item(1, None)).unwrap();
        let artwork = ArtworkInput {
            mime_type: "image/png".into(),
            bytes: vec![1, 2, 3],
        };
        store
            .refresh_metadata_text(item_id, Some("Title".into()), None, None, Some(artwork))
            .unwrap();
        let item_page = store
            .query_music_items(run.id, ItemQuery::default())
            .unwrap();
        let artwork_id = item_page.items[0].artwork_id.expect("item artwork sync");
        assert_eq!(
            store
                .run(run.id)
                .unwrap()
                .unwrap()
                .representative_artwork_id,
            Some(artwork_id)
        );
        store.delete_run(run.id).unwrap();
        assert!(store.artwork(artwork_id).unwrap().is_none());
    }

    #[test]
    fn history_paginates_deduplicates_artwork_and_deletes_records_only() {
        let (_directory, store) = store();
        let run = store
            .create_run(CreateRun {
                input_kind: InputKind::Directory,
                input_path: "/input".into(),
                output_path: "/output".into(),
                settings: settings(),
                total_items: 3,
            })
            .unwrap();
        store.mark_running(run.id).unwrap();
        let artwork = ArtworkInput {
            mime_type: "image/png".into(),
            bytes: vec![1, 2, 3],
        };
        store
            .record_item(run.id, item(0, Some(artwork.clone())))
            .unwrap();
        store.record_item(run.id, item(1, Some(artwork))).unwrap();
        store.record_item(run.id, item(2, None)).unwrap();
        let first = store
            .query_music_items(
                run.id,
                ItemQuery {
                    page: 1,
                    page_size: 2,
                    ..ItemQuery::default()
                },
            )
            .unwrap();
        assert_eq!(first.items.len(), 2);
        assert_eq!(first.page_count, 2);
        assert_eq!(first.items[0].artwork_id, first.items[1].artwork_id);
        let artwork_id = first.items[0].artwork_id.unwrap();
        assert_eq!(
            store.artwork(artwork_id).unwrap().unwrap().bytes,
            vec![1, 2, 3]
        );
        let second = store
            .query_music_items(
                run.id,
                ItemQuery {
                    page: 2,
                    page_size: 2,
                    ..ItemQuery::default()
                },
            )
            .unwrap();
        assert_eq!(second.items.len(), 1);
        assert_eq!(second.page, 2);
        assert!(store.delete_run(run.id).unwrap());
        assert!(store.artwork(artwork_id).unwrap().is_none());
    }

    #[test]
    fn detail_queries_filter_sort_and_page_the_complete_run() {
        let (_directory, store) = store();
        let run = store
            .create_run(CreateRun {
                input_kind: InputKind::Directory,
                input_path: "/input".into(),
                output_path: "/output".into(),
                settings: settings(),
                total_items: 6,
            })
            .unwrap();
        for ordinal in 0..6 {
            let mut value = item(ordinal, None);
            value.title = Some(if ordinal == 5 {
                "Late Target".into()
            } else {
                format!("Track {ordinal}")
            });
            value.artist = Some(if ordinal % 2 == 0 {
                "Artist A".into()
            } else {
                "Artist B".into()
            });
            value.album = Some(if ordinal < 3 {
                "First Album".into()
            } else {
                "Second Album".into()
            });
            value.output_bytes = Some(100 + ordinal);
            store.record_item(run.id, value).unwrap();
        }

        let search = store
            .query_music_items(
                run.id,
                ItemQuery {
                    page: 1,
                    page_size: 2,
                    search: Some(" target ".into()),
                    ..ItemQuery::default()
                },
            )
            .unwrap();
        assert_eq!(search.filtered_total, 1);
        assert_eq!(search.items[0].ordinal, 5);

        let later_artist_page = store
            .query_music_items(
                run.id,
                ItemQuery {
                    page: 2,
                    page_size: 2,
                    artist: Some("artist b".into()),
                    ..ItemQuery::default()
                },
            )
            .unwrap();
        assert_eq!(later_artist_page.filtered_total, 3);
        assert_eq!(later_artist_page.page_count, 2);
        assert_eq!(later_artist_page.items.len(), 1);

        let by_size = store
            .query_music_items(
                run.id,
                ItemQuery {
                    page: 1,
                    page_size: 25,
                    sort: ItemSort::Size,
                    direction: SortDirection::Desc,
                    ..ItemQuery::default()
                },
            )
            .unwrap();
        assert_eq!(by_size.items[0].ordinal, 5);
        assert_eq!(store.run(run.id).unwrap().unwrap().music_count, 6);

        let first_facets = store
            .list_music_facets(run.id, FacetKind::Artist, None, None, 1)
            .unwrap();
        assert_eq!(first_facets.values, ["Artist A"]);
        let second_facets = store
            .list_music_facets(run.id, FacetKind::Artist, None, first_facets.next_cursor, 1)
            .unwrap();
        assert_eq!(second_facets.values, ["Artist B"]);
    }

    #[test]
    fn schema_one_database_migrates_v2_and_v3_columns_idempotently() {
        let directory = tempfile::tempdir().unwrap();
        let database = directory.path().join("history.sqlite");
        let store = HistoryStore::open(&database).unwrap();
        store
            .execute_test_sql(
                "DROP INDEX IF EXISTS items_run_probe_pending;
                 ALTER TABLE runs DROP COLUMN display_name;
                 ALTER TABLE items DROP COLUMN metadata_edit_pending;
                 ALTER TABLE items DROP COLUMN media_probe_version;
                 ALTER TABLE items DROP COLUMN bitrate_kbps;
                 ALTER TABLE items DROP COLUMN output_bytes;
                 ALTER TABLE items DROP COLUMN output_format;
                 PRAGMA user_version = 1;",
            )
            .unwrap();
        drop(store);

        let migrated = HistoryStore::open(&database).unwrap();
        let connection = migrated.connection.lock().unwrap();
        let version: i64 = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        let columns: Vec<String> = connection
            .prepare("SELECT name FROM pragma_table_info('items')")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert_eq!(version, SCHEMA_VERSION);
        assert!(columns.iter().any(|column| column == "output_bytes"));
        assert!(columns.iter().any(|column| column == "output_format"));
        assert!(columns.iter().any(|column| column == "bitrate_kbps"));
        assert!(columns.iter().any(|column| column == "media_probe_version"));
        assert!(
            columns
                .iter()
                .any(|column| column == "metadata_edit_pending")
        );
        let run_columns: Vec<String> = connection
            .prepare("SELECT name FROM pragma_table_info('runs')")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert!(run_columns.iter().any(|column| column == "display_name"));
    }

    #[test]
    fn schema_two_database_migrates_v3_columns_idempotently() {
        let directory = tempfile::tempdir().unwrap();
        let database = directory.path().join("history.sqlite");
        let store = HistoryStore::open(&database).unwrap();
        store
            .execute_test_sql(
                "DROP INDEX IF EXISTS items_run_probe_pending;
             ALTER TABLE runs DROP COLUMN display_name;
             ALTER TABLE items DROP COLUMN metadata_edit_pending;
             ALTER TABLE items DROP COLUMN media_probe_version;
             ALTER TABLE items DROP COLUMN bitrate_kbps;
             PRAGMA user_version = 2;",
            )
            .unwrap();
        drop(store);
        let migrated = HistoryStore::open(&database).unwrap();
        let version: i64 = migrated
            .connection
            .lock()
            .unwrap()
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, SCHEMA_VERSION);
        let columns: Vec<String> = migrated
            .connection
            .lock()
            .unwrap()
            .prepare("SELECT name FROM pragma_table_info('items')")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert!(
            columns.iter().any(|column| column == "bitrate_kbps")
                && columns.iter().any(|column| column == "media_probe_version")
                && columns
                    .iter()
                    .any(|column| column == "metadata_edit_pending")
        );
        drop(migrated);
        let reopened = HistoryStore::open(&database).unwrap();
        assert_eq!(
            reopened
                .connection
                .lock()
                .unwrap()
                .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
                .unwrap(),
            SCHEMA_VERSION
        );
    }

    #[test]
    fn bounded_page_read_repairs_only_existing_legacy_outputs() {
        use base64::{Engine, engine::general_purpose::STANDARD};

        let (directory, store) = store();
        let run = store
            .create_run(CreateRun {
                input_kind: InputKind::Directory,
                input_path: directory.path().join("input"),
                output_path: directory.path().join("output"),
                settings: settings(),
                total_items: 2,
            })
            .unwrap();
        let output = directory.path().join("existing-output");
        let bytes = STANDARD
            .decode(include_str!("../../../../tests/fixtures/lofty-silent.mp3.b64").trim())
            .unwrap();
        fs::write(&output, &bytes).unwrap();
        let mut existing = item(0, None);
        existing.destination_path = Some(output);
        existing.output_bytes = None;
        existing.output_format = None;
        store.record_item(run.id, existing).unwrap();
        let mut missing = item(1, None);
        missing.destination_path = Some(directory.path().join("missing-output"));
        missing.output_bytes = None;
        missing.output_format = None;
        store.record_item(run.id, missing).unwrap();

        let page = store
            .query_music_items(run.id, ItemQuery::default())
            .unwrap();
        assert_eq!(page.items[0].output_bytes, Some(bytes.len() as u64));
        assert_eq!(page.items[0].output_format.as_deref(), Some("mp3"));
        assert_eq!(page.items[1].output_bytes, None);
    }

    #[test]
    fn bounded_page_read_repairs_missing_artist_from_local_ncm_container() {
        use base64::{Engine, engine::general_purpose::STANDARD};

        let (directory, store) = store();
        let source = directory.path().join("portable.ncm");
        let audio = STANDARD
            .decode(include_str!("../../../../tests/fixtures/lofty-silent.mp3.b64").trim())
            .unwrap();
        fs::write(
            &source,
            crate::desktop_ncm_fixture::ncm_with_audio(&[], &audio),
        )
        .unwrap();
        let expected = unlockm::metadata::ncm::parse(&source).unwrap();
        let expected_artist = expected.artists.as_ref().unwrap().text.as_str();
        let run = store
            .create_run(CreateRun {
                input_kind: InputKind::File,
                input_path: source.clone(),
                output_path: directory.path().to_path_buf(),
                settings: settings(),
                total_items: 1,
            })
            .unwrap();
        let mut legacy = item(0, None);
        legacy.source_path = source;
        legacy.title = None;
        legacy.artist = None;
        legacy.album = None;
        legacy.duration_ms = None;
        store.record_item(run.id, legacy).unwrap();

        let page = store
            .query_music_items(run.id, ItemQuery::default())
            .unwrap();
        assert_eq!(page.items[0].artist.as_deref(), Some(expected_artist));
        assert_eq!(
            page.items[0].title.as_deref(),
            expected
                .title
                .as_ref()
                .map(|value| value.text.as_str())
                .or(Some("portable"))
        );
    }

    #[test]
    fn thousand_item_run_remains_page_bounded_and_searches_late_matches() {
        let (_directory, store) = store();
        let run = store
            .create_run(CreateRun {
                input_kind: InputKind::Directory,
                input_path: "/large-input".into(),
                output_path: "/large-output".into(),
                settings: settings(),
                total_items: 1_001,
            })
            .unwrap();
        for ordinal in 0..1_001 {
            let mut value = item(ordinal, None);
            value.title = Some(if ordinal == 1_000 {
                "Only On The Last Page".into()
            } else {
                format!("Track {ordinal:04}")
            });
            value.artist = Some(format!("Artist {}", ordinal % 17));
            value.album = Some(format!("Album {}", ordinal % 31));
            store.record_item(run.id, value).unwrap();
        }

        let first = store
            .query_music_items(run.id, ItemQuery::default())
            .unwrap();
        assert_eq!(first.items.len(), 25);
        assert_eq!(first.music_total, 1_001);
        assert_eq!(first.page_count, 41);

        let late = store
            .query_music_items(
                run.id,
                ItemQuery {
                    search: Some("last page".into()),
                    ..ItemQuery::default()
                },
            )
            .unwrap();
        assert_eq!(late.filtered_total, 1);
        assert_eq!(late.items[0].ordinal, 1_000);
    }

    #[test]
    fn startup_recovery_preserves_items_and_marks_active_runs_interrupted() {
        let directory = tempfile::tempdir().unwrap();
        let database = directory.path().join("history.sqlite");
        let store = HistoryStore::open(&database).unwrap();
        let run = store
            .create_run(CreateRun {
                input_kind: InputKind::File,
                input_path: "/input/file.ncm".into(),
                output_path: "/output".into(),
                settings: settings(),
                total_items: 1,
            })
            .unwrap();
        store.mark_running(run.id).unwrap();
        store.record_item(run.id, item(0, None)).unwrap();
        drop(store);
        let state = crate::state::AppState::new(HistoryStore::open(database).unwrap()).unwrap();
        let recovered = state.history().run(run.id).unwrap().unwrap();
        assert_eq!(recovered.status, RunStatus::Interrupted);
        assert_eq!(
            state
                .history()
                .query_music_items(run.id, ItemQuery::default())
                .unwrap()
                .items
                .len(),
            1
        );
    }
}
