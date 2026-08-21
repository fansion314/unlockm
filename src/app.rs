pub mod events;
pub mod job;
pub mod runner;
pub mod scan;
pub mod watch;

use crate::{
    PARTIAL_FAILURE_EXIT,
    config::RunConfig,
    error::{AppError, Result},
};
use events::{BatchObserver, ItemAction, ItemKind, MusicSnapshot, ReportError, ReportWarning};
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::Arc, time::Duration};
use tracing::info;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub converted: usize,
    pub copied: usize,
    pub skipped: usize,
    pub conflicted: usize,
    pub failed: usize,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobSpec {
    pub source: std::path::PathBuf,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobOutcome {
    Converted,
    Copied,
    Skipped,
    Conflict,
    Failed,
    Cancelled,
}

/// Per-file timings kept for internal diagnostics. They intentionally do not
/// change `Summary`, which remains the stable application/CLI count contract.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobTimings {
    pub preflight: Duration,
    pub decrypt: Duration,
    pub metadata_resolve: Duration,
    pub tag_write: Duration,
    pub fallback: Duration,
    pub copy: Duration,
    pub publish: Duration,
}

/// The terminal report for one path accepted by a runner invocation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobReport {
    pub ordinal: u64,
    pub source: PathBuf,
    pub relative_path: PathBuf,
    pub destination: Option<PathBuf>,
    pub item_kind: ItemKind,
    pub action: Option<ItemAction>,
    pub logical_bytes: u64,
    pub outcome: JobOutcome,
    pub error: Option<ReportError>,
    pub warnings: Vec<ReportWarning>,
    pub music: Option<MusicSnapshot>,
    pub timings: JobTimings,
}
impl Summary {
    pub fn exit_code(self) -> i32 {
        if self.failed + self.conflicted == 0 {
            0
        } else {
            PARTIAL_FAILURE_EXIT
        }
    }
    pub fn merge(&mut self, other: Self) {
        self.converted += other.converted;
        self.copied += other.copied;
        self.skipped += other.skipped;
        self.conflicted += other.conflicted;
        self.failed += other.failed;
    }
}
impl From<JobOutcome> for Summary {
    fn from(value: JobOutcome) -> Self {
        match value {
            JobOutcome::Converted => Self {
                converted: 1,
                ..Self::default()
            },
            JobOutcome::Copied => Self {
                copied: 1,
                ..Self::default()
            },
            JobOutcome::Skipped => Self {
                skipped: 1,
                ..Self::default()
            },
            JobOutcome::Conflict => Self {
                conflicted: 1,
                ..Self::default()
            },
            JobOutcome::Failed => Self {
                failed: 1,
                ..Self::default()
            },
            JobOutcome::Cancelled => Self::default(),
        }
    }
}
pub async fn run(config: RunConfig) -> Result<Summary> {
    let config = Arc::new(config);
    let paths = scan::enumerate(&config)?;
    config.presentation.begin_batch(paths.len());
    // The CLI deliberately keeps the observer disabled; desktop hosts opt in
    // through `run_batch_with_observer_and_cancel` without changing this
    // scanner/runner pipeline or the subsequent watch scheduler.
    let initial =
        runner::run_paths_with_observer(config.clone(), paths, BatchObserver::default()).await?;
    info!(
        converted = initial.converted,
        copied = initial.copied,
        skipped = initial.skipped,
        failed = initial.failed,
        "batch summary"
    );
    if config.watch {
        config.presentation.begin_watch();
        watch::run(config, initial).await
    } else {
        Ok(initial)
    }
}

/// Run a single non-watch batch through the same scanner and runner used by
/// the CLI, forwarding only UI-neutral events to the supplied observer.
pub async fn run_batch_with_observer_and_cancel(
    config: RunConfig,
    cancellation: runner::Cancellation,
    observer: BatchObserver,
) -> Result<Summary> {
    if config.watch {
        return Err(AppError::Config(
            "batch observer does not support watch mode".into(),
        ));
    }
    let config = Arc::new(config);
    let paths = scan::enumerate(&config)?;
    config.presentation.begin_batch(paths.len());
    let summary =
        runner::run_paths_with_observer_and_cancel(config, paths, cancellation, observer).await?;
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn outcomes_increment_exactly_one_counter() {
        for o in [
            JobOutcome::Converted,
            JobOutcome::Copied,
            JobOutcome::Skipped,
            JobOutcome::Conflict,
            JobOutcome::Failed,
        ] {
            let s: Summary = o.into();
            assert_eq!(
                s.converted + s.copied + s.skipped + s.conflicted + s.failed,
                1
            )
        }
    }

    #[test]
    fn cancelled_outcome_does_not_change_summary_counters() {
        assert_eq!(Summary::from(JobOutcome::Cancelled), Summary::default());
    }
}
