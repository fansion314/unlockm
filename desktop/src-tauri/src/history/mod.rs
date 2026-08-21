//! Rust-owned local conversion history.
//!
//! The webview never receives a database path or SQL capability.  This store
//! keeps only user-visible conversion facts and bounded artwork bytes; it does
//! not open, delete, or otherwise mutate recorded source/output paths.

use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};
use thiserror::Error;

const SCHEMA_VERSION: i64 = 1;
const MAX_PAGE_SIZE: usize = 200;
const MAX_ARTWORK_BYTES: usize = 10 * 1024 * 1024;

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
    pub next_cursor: Option<u64>,
    pub companion_count: usize,
    pub music_total: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtworkRecord {
    pub id: i64,
    pub mime_type: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Preferences {
    pub locale: String,
    pub default_settings: RunSettings,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            locale: "en-US".into(),
            default_settings: RunSettings {
                update_metadata: false,
                copy_passthrough: true,
                overwrite: false,
            },
        }
    }
}

pub struct HistoryStore {
    connection: Mutex<Connection>,
}

impl HistoryStore {
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
                action, outcome, logical_bytes, preflight_ms, decrypt_ms,
                metadata_resolve_ms, tag_write_ms, fallback_ms, copy_ms, publish_ms,
                error_code, error_detail, warnings_json, title, artist, album, duration_ms, artwork_id
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
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
            "SELECT id, input_kind, input_path, output_path, settings_json, status,
                    created_at_ms, updated_at_ms, finished_at_ms, total_items, completed_items,
                    converted, copied, skipped, conflicted, failed, representative_artwork_id,
                    terminal_error_code, terminal_error_detail
             FROM runs
             WHERE (? IS NULL OR id < ?)
             ORDER BY id DESC LIMIT ?",
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

    pub fn list_music_items(
        &self,
        run_id: i64,
        cursor: Option<u64>,
        limit: usize,
    ) -> HistoryResult<ItemPage> {
        let limit = checked_limit(limit);
        let connection = self.connection.lock().expect("history connection lock");
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
        let mut statement = connection.prepare(
            "SELECT id, run_id, ordinal, item_kind, source_path, relative_path, destination_path,
                    action, outcome, logical_bytes, error_code, error_detail, warnings_json,
                    title, artist, album, duration_ms, artwork_id
             FROM items
             WHERE run_id = ? AND item_kind = 'music' AND (? IS NULL OR ordinal > ?)
             ORDER BY ordinal ASC LIMIT ?",
        )?;
        let mut rows = statement.query(params![
            run_id,
            cursor.map(|value| value as i64),
            cursor.map(|value| value as i64),
            (limit + 1) as i64
        ])?;
        let mut items = Vec::with_capacity(limit);
        let mut has_more = false;
        while let Some(row) = rows.next()? {
            if items.len() == limit {
                has_more = true;
                break;
            }
            items.push(item_from_row(row)?);
        }
        let next_cursor = if has_more {
            items.last().map(|item| item.ordinal)
        } else {
            None
        };
        Ok(ItemPage {
            items,
            next_cursor,
            companion_count: companion_count.max(0) as usize,
            music_total: music_total.max(0) as usize,
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
        value
            .map(|value| serde_json::from_str(&value).map_err(Into::into))
            .transpose()
            .map(|value| value.unwrap_or_default())
    }

    pub fn set_preferences(&self, preferences: &Preferences) -> HistoryResult<()> {
        let connection = self.connection.lock().expect("history connection lock");
        connection.execute(
            "INSERT INTO preferences (key, value_json, updated_at_ms)
             VALUES ('preferences', ?, ?)
             ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json,
                                           updated_at_ms = excluded.updated_at_ms",
            params![serde_json::to_string(preferences)?, now_ms()],
        )?;
        Ok(())
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
    let version: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if version >= SCHEMA_VERSION {
        return Ok(());
    }
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
    transaction.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    transaction.commit()?;
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
            "SELECT id, input_kind, input_path, output_path, settings_json, status,
                    created_at_ms, updated_at_ms, finished_at_ms, total_items, completed_items,
                    converted, copied, skipped, conflicted, failed, representative_artwork_id,
                    terminal_error_code, terminal_error_detail
             FROM runs WHERE id = ?",
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
        input_kind: if row.get::<_, String>(1)? == "directory" {
            InputKind::Directory
        } else {
            InputKind::File
        },
        input_path: PathBuf::from(row.get::<_, String>(2)?),
        output_path: PathBuf::from(row.get::<_, String>(3)?),
        settings,
        status: RunStatus::parse(&row.get::<_, String>(5)?),
        created_at_ms: row.get(6)?,
        updated_at_ms: row.get(7)?,
        finished_at_ms: row.get(8)?,
        total_items: row.get::<_, i64>(9)? as usize,
        completed_items: row.get::<_, i64>(10)? as usize,
        converted: row.get::<_, i64>(11)? as usize,
        copied: row.get::<_, i64>(12)? as usize,
        skipped: row.get::<_, i64>(13)? as usize,
        conflicted: row.get::<_, i64>(14)? as usize,
        failed: row.get::<_, i64>(15)? as usize,
        representative_artwork_id: row.get(16)?,
        terminal_error_code: row.get(17)?,
        terminal_error_detail: row.get(18)?,
    })
}

fn item_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<HistoryItem> {
    let warnings_json: String = row.get(12)?;
    let warnings = serde_json::from_str(&warnings_json).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(12, rusqlite::types::Type::Text, Box::new(error))
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
        error_code: row.get(10)?,
        error_detail: row.get(11)?,
        warnings,
        title: row.get(13)?,
        artist: row.get(14)?,
        album: row.get(15)?,
        duration_ms: row.get::<_, Option<i64>>(16)?.map(|value| value as u32),
        artwork_id: row.get(17)?,
    })
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
            default_settings: settings(),
        };
        store.set_preferences(&preferences).unwrap();
        drop(store);
        let reopened = HistoryStore::open(database).unwrap();
        assert_eq!(reopened.preferences().unwrap(), preferences);
        assert_eq!(reopened.list_runs(None, 50).unwrap().runs, Vec::new());
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
        let first = store.list_music_items(run.id, None, 2).unwrap();
        assert_eq!(first.items.len(), 2);
        assert_eq!(first.next_cursor, Some(1));
        assert_eq!(first.items[0].artwork_id, first.items[1].artwork_id);
        let artwork_id = first.items[0].artwork_id.unwrap();
        assert_eq!(
            store.artwork(artwork_id).unwrap().unwrap().bytes,
            vec![1, 2, 3]
        );
        let second = store
            .list_music_items(run.id, first.next_cursor, 2)
            .unwrap();
        assert_eq!(second.items.len(), 1);
        assert!(second.next_cursor.is_none());
        assert!(store.delete_run(run.id).unwrap());
        assert!(store.artwork(artwork_id).unwrap().is_none());
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
                .list_music_items(run.id, None, 10)
                .unwrap()
                .items
                .len(),
            1
        );
    }
}
