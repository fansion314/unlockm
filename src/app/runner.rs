use super::{
    JobOutcome, JobReport, JobTimings, Summary,
    events::{BatchEvent, BatchObserver, ItemAction, ItemKind, MusicSnapshot, ReportError},
    job,
};
use crate::{
    config::RunConfig,
    error::Result,
    formats,
    keys::KeySources,
    metadata::provider::qq::MetadataClient,
    pathing::{Reservations, collision_key},
};
use std::sync::atomic::AtomicBool;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Instant,
};
use tokio::{
    sync::Semaphore,
    task::{Id, JoinSet},
};

#[derive(Clone, Default)]
pub struct Cancellation(Arc<AtomicBool>);
impl Cancellation {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst)
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}
#[derive(Default)]
pub struct RunnerMetrics {
    active_jobs: AtomicUsize,
    active_open_files: AtomicUsize,
    max_jobs: AtomicUsize,
    max_open_files: AtomicUsize,
    totals: Mutex<TimingTotals>,
    collector: Option<Mutex<ReportCollector>>,
}

#[derive(Default)]
struct TimingTotals {
    files: usize,
    logical_bytes: u64,
    preflight: std::time::Duration,
    decrypt: std::time::Duration,
    metadata_resolve: std::time::Duration,
    tag_write: std::time::Duration,
    fallback: std::time::Duration,
    copy: std::time::Duration,
    publish: std::time::Duration,
    outcomes: Summary,
}

struct ReportCollector {
    limit: usize,
    reports: Vec<JobReport>,
}

struct MeasuredPlannedJob {
    ordinal: u64,
    planned: job::PlannedJob,
    logical_bytes: u64,
    preflight: std::time::Duration,
}

struct InFlightJob {
    ordinal: u64,
    source: PathBuf,
    destination: PathBuf,
    item_kind: ItemKind,
    action: ItemAction,
    logical_bytes: u64,
    preflight: std::time::Duration,
}
/// Long-lived inputs shared by the batch and watch schedulers.  In particular,
/// the semaphore must be shared: creating one per watch event would let an
/// event storm exceed `--jobs` even though every individual invocation looked
/// bounded.
#[derive(Clone)]
pub struct RunnerResources {
    semaphore: Arc<Semaphore>,
    keys: Arc<KeySources>,
    metadata: Arc<MetadataClient>,
    reservations: Arc<Mutex<Reservations>>,
}
impl RunnerResources {
    pub fn new(config: &RunConfig) -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(config.jobs.get())),
            keys: Arc::new(KeySources::from_config(config)),
            metadata: Arc::new(MetadataClient::new(config.metadata_jobs.get())),
            reservations: Arc::new(Mutex::new(Reservations::default())),
        }
    }
}
impl RunnerMetrics {
    fn enter(&self) {
        let j = self.active_jobs.fetch_add(1, Ordering::SeqCst) + 1;
        self.max_jobs.fetch_max(j, Ordering::SeqCst);
        let f = self.active_open_files.fetch_add(1, Ordering::SeqCst) + 1;
        self.max_open_files.fetch_max(f, Ordering::SeqCst);
    }
    fn leave(&self) {
        self.active_jobs.fetch_sub(1, Ordering::SeqCst);
        self.active_open_files.fetch_sub(1, Ordering::SeqCst);
    }
    pub fn maxima(&self) -> (usize, usize) {
        (
            self.max_jobs.load(Ordering::SeqCst),
            self.max_open_files.load(Ordering::SeqCst),
        )
    }
    fn record(&self, report: &JobReport) {
        tracing::trace!(
            path = %report.relative_path.display(),
            logical_bytes = report.logical_bytes,
            outcome = ?report.outcome,
            preflight_ms = report.timings.preflight.as_millis(),
            decrypt_ms = report.timings.decrypt.as_millis(),
            metadata_resolve_ms = report.timings.metadata_resolve.as_millis(),
            tag_write_ms = report.timings.tag_write.as_millis(),
            fallback_ms = report.timings.fallback.as_millis(),
            copy_ms = report.timings.copy.as_millis(),
            publish_ms = report.timings.publish.as_millis(),
            "job timing report"
        );
        let mut totals = self.totals.lock().expect("timing totals lock");
        totals.files += 1;
        totals.logical_bytes += report.logical_bytes;
        totals.preflight += report.timings.preflight;
        totals.decrypt += report.timings.decrypt;
        totals.metadata_resolve += report.timings.metadata_resolve;
        totals.tag_write += report.timings.tag_write;
        totals.fallback += report.timings.fallback;
        totals.copy += report.timings.copy;
        totals.publish += report.timings.publish;
        totals.outcomes.merge(report.outcome.into());
        drop(totals);
        if let Some(collector) = &self.collector {
            let mut collector = collector.lock().expect("report collector lock");
            if collector.reports.len() < collector.limit {
                collector.reports.push(report.clone());
            }
        }
    }
    fn collects_reports(&self) -> bool {
        self.collector.is_some()
    }
    #[cfg(test)]
    fn collecting(limit: usize) -> Self {
        Self {
            collector: Some(Mutex::new(ReportCollector {
                limit,
                reports: Vec::with_capacity(limit),
            })),
            ..Default::default()
        }
    }
    #[cfg(test)]
    fn reports(&self) -> Vec<JobReport> {
        self.collector
            .as_ref()
            .expect("test report collection enabled")
            .lock()
            .expect("report collector lock")
            .reports
            .clone()
    }
}

fn report_for(
    config: &RunConfig,
    ordinal: u64,
    source: &std::path::Path,
    logical_bytes: u64,
    preflight: std::time::Duration,
) -> JobReport {
    JobReport {
        ordinal,
        source: source.to_path_buf(),
        relative_path: config
            .relative_input(source)
            .unwrap_or_else(|_| source.to_path_buf()),
        destination: None,
        item_kind: item_kind_for_source(source),
        action: None,
        logical_bytes,
        outcome: JobOutcome::Failed,
        error: None,
        warnings: Vec::new(),
        music: None,
        timings: JobTimings {
            preflight,
            ..Default::default()
        },
    }
}

fn item_kind_for_source(source: &std::path::Path) -> ItemKind {
    if formats::candidate(source).is_some() {
        ItemKind::Music
    } else {
        ItemKind::Companion
    }
}

fn planned_item_kind(planned: &job::PlannedJob) -> ItemKind {
    match &planned.action {
        job::PlannedAction::Copy => ItemKind::Companion,
        job::PlannedAction::Decrypt { .. } => ItemKind::Music,
    }
}

fn planned_action(planned: &job::PlannedJob) -> ItemAction {
    match &planned.action {
        job::PlannedAction::Copy => ItemAction::Copy,
        job::PlannedAction::Decrypt { .. } => ItemAction::Convert,
    }
}

fn report_for_planned(config: &RunConfig, measured: &MeasuredPlannedJob) -> JobReport {
    let mut report = report_for(
        config,
        measured.ordinal,
        &measured.planned.source,
        measured.logical_bytes,
        measured.preflight,
    );
    report.destination = Some(measured.planned.destination.clone());
    report.item_kind = planned_item_kind(&measured.planned);
    report.action = Some(planned_action(&measured.planned));
    report
}

fn cancelled_report(config: &RunConfig, measured: &MeasuredPlannedJob) -> JobReport {
    let mut report = report_for_planned(config, measured);
    report.outcome = JobOutcome::Cancelled;
    report.error = Some(ReportError {
        code: "cancelled".into(),
        detail: "conversion was cancelled before publication".into(),
    });
    report
}

fn record_report(metrics: &RunnerMetrics, observer: &BatchObserver, mut report: JobReport) {
    let needs_desktop_report = observer.is_enabled() || metrics.collects_reports();
    if needs_desktop_report && report.item_kind == ItemKind::Music && report.music.is_none() {
        report.music = Some(MusicSnapshot {
            title: report
                .source
                .file_stem()
                .map(|value| value.to_string_lossy().into_owned())
                .filter(|value| !value.trim().is_empty()),
            ..Default::default()
        });
    }
    metrics.record(&report);
    if observer.is_enabled() {
        observer.emit(BatchEvent::ItemTerminal {
            report: Box::new(report),
        });
    }
}

fn outcome_from_summary(summary: Summary) -> JobOutcome {
    if summary.converted == 1 {
        JobOutcome::Converted
    } else if summary.copied == 1 {
        JobOutcome::Copied
    } else if summary.skipped == 1 {
        JobOutcome::Skipped
    } else if summary.conflicted == 1 {
        JobOutcome::Conflict
    } else {
        JobOutcome::Failed
    }
}

fn emit_timing_reports(metrics: &RunnerMetrics) {
    let totals = metrics.totals.lock().expect("timing totals lock");
    tracing::debug!(
        files = totals.files,
        logical_bytes = totals.logical_bytes,
        converted = totals.outcomes.converted,
        copied = totals.outcomes.copied,
        skipped = totals.outcomes.skipped,
        conflicted = totals.outcomes.conflicted,
        failed = totals.outcomes.failed,
        preflight_ms = totals.preflight.as_millis(),
        decrypt_ms = totals.decrypt.as_millis(),
        metadata_resolve_ms = totals.metadata_resolve.as_millis(),
        tag_write_ms = totals.tag_write.as_millis(),
        fallback_ms = totals.fallback.as_millis(),
        copy_ms = totals.copy.as_millis(),
        publish_ms = totals.publish.as_millis(),
        "runner timing summary"
    );
}
pub async fn run_paths(config: Arc<RunConfig>, paths: Vec<PathBuf>) -> Result<Summary> {
    run_paths_with_observer(config, paths, BatchObserver::default()).await
}

pub async fn run_paths_with_observer(
    config: Arc<RunConfig>,
    paths: Vec<PathBuf>,
    observer: BatchObserver,
) -> Result<Summary> {
    let resources = RunnerResources::new(&config);
    run_paths_with_resources_and_cancel_with_observer(
        config,
        paths,
        Arc::new(RunnerMetrics::default()),
        Cancellation::default(),
        resources,
        observer,
    )
    .await
}

pub async fn run_paths_with_observer_and_cancel(
    config: Arc<RunConfig>,
    paths: Vec<PathBuf>,
    cancellation: Cancellation,
    observer: BatchObserver,
) -> Result<Summary> {
    let resources = RunnerResources::new(&config);
    run_paths_with_resources_and_cancel_with_observer(
        config,
        paths,
        Arc::new(RunnerMetrics::default()),
        cancellation,
        resources,
        observer,
    )
    .await
}
pub async fn run_paths_with_metrics(
    config: Arc<RunConfig>,
    paths: Vec<PathBuf>,
    metrics: Arc<RunnerMetrics>,
) -> Result<Summary> {
    let resources = RunnerResources::new(&config);
    run_paths_with_resources_and_cancel_with_observer(
        config,
        paths,
        metrics,
        Cancellation::default(),
        resources,
        BatchObserver::default(),
    )
    .await
}
pub async fn run_paths_with_metrics_and_cancel(
    config: Arc<RunConfig>,
    paths: Vec<PathBuf>,
    metrics: Arc<RunnerMetrics>,
    cancellation: Cancellation,
) -> Result<Summary> {
    run_paths_with_metrics_and_cancel_and_observer(
        config,
        paths,
        metrics,
        cancellation,
        BatchObserver::default(),
    )
    .await
}

pub async fn run_paths_with_metrics_and_cancel_and_observer(
    config: Arc<RunConfig>,
    paths: Vec<PathBuf>,
    metrics: Arc<RunnerMetrics>,
    cancellation: Cancellation,
    observer: BatchObserver,
) -> Result<Summary> {
    let resources = RunnerResources::new(&config);
    run_paths_with_resources_and_cancel_with_observer(
        config,
        paths,
        metrics,
        cancellation,
        resources,
        observer,
    )
    .await
}

pub async fn run_paths_with_resources_and_cancel(
    config: Arc<RunConfig>,
    paths: Vec<PathBuf>,
    metrics: Arc<RunnerMetrics>,
    cancellation: Cancellation,
    resources: RunnerResources,
) -> Result<Summary> {
    run_paths_with_resources_and_cancel_with_observer(
        config,
        paths,
        metrics,
        cancellation,
        resources,
        BatchObserver::default(),
    )
    .await
}

pub async fn run_paths_with_resources_and_cancel_with_observer(
    config: Arc<RunConfig>,
    paths: Vec<PathBuf>,
    metrics: Arc<RunnerMetrics>,
    cancellation: Cancellation,
    resources: RunnerResources,
    observer: BatchObserver,
) -> Result<Summary> {
    let mut set = JoinSet::new();
    // Resolve every encrypted input through a bounded decrypted-prefix probe
    // before claiming destinations. Suffix-only reservations would let a copy
    // and conversion race to the same flat output, while full stages here would
    // retain corpus-sized temporary data behind the collision barrier.
    let mut seen = HashSet::new();
    let paths: Vec<_> = paths
        .into_iter()
        .filter(|path| seen.insert(path.clone()))
        .collect();
    observer.emit(BatchEvent::PlanningStarted { total: paths.len() });
    let mut preflight_jobs: HashMap<Id, (u64, PathBuf)> = HashMap::new();
    for (index, path) in paths.into_iter().enumerate() {
        let ordinal = index as u64;
        let relative = config
            .relative_input(&path)
            .unwrap_or_else(|_| path.clone());
        if cancellation.is_cancelled() {
            let mut report = report_for(&config, ordinal, &path, 0, std::time::Duration::ZERO);
            report.outcome = JobOutcome::Cancelled;
            report.error = Some(ReportError {
                code: "cancelled".into(),
                detail: "conversion was cancelled before planning".into(),
            });
            record_report(&metrics, &observer, report);
            continue;
        }
        if observer.is_enabled() {
            observer.emit(BatchEvent::ItemPlanning {
                ordinal,
                source: path.clone(),
                relative_path: relative.clone(),
                item_kind: item_kind_for_source(&path),
            });
        }
        config.presentation.activity(&relative, "Planning");
        let permit = resources
            .semaphore
            .clone()
            .acquire_owned()
            .await
            .map_err(|e| crate::error::AppError::Internal(e.to_string()))?;
        let c = config.clone();
        let keys = resources.keys.clone();
        let metadata = resources.metadata.clone();
        let dispatch = tracing::dispatcher::get_default(Clone::clone);
        let metrics = metrics.clone();
        let task_source = path.clone();
        let handle = set.spawn_blocking(move || {
            let _permit = permit;
            metrics.enter();
            let started = Instant::now();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                tracing::dispatcher::with_default(&dispatch, || {
                    job::preflight_with_resources(&c, &keys, &metadata, &path)
                })
            }))
            .unwrap_or_else(|_| Err("preflight worker panicked".into()));
            metrics.leave();
            (ordinal, path, started.elapsed(), result)
        });
        preflight_jobs.insert(handle.id(), (ordinal, task_source));
    }
    let mut summary = Summary::default();
    let mut grouped: HashMap<PathBuf, Vec<MeasuredPlannedJob>> = HashMap::new();
    while let Some(result) = set.join_next_with_id().await {
        match result {
            Ok((task_id, (ordinal, source, preflight, Ok(Some(planned))))) => {
                preflight_jobs.remove(&task_id);
                let logical_bytes = std::fs::metadata(&source)
                    .map(|metadata| metadata.len())
                    .unwrap_or(0);
                grouped
                    .entry(collision_key(&planned.destination))
                    .or_default()
                    .push(MeasuredPlannedJob {
                        ordinal,
                        planned,
                        logical_bytes,
                        preflight,
                    });
            }
            Ok((task_id, (ordinal, source, preflight, Ok(None)))) => {
                preflight_jobs.remove(&task_id);
                summary.skipped += 1;
                let logical_bytes = std::fs::metadata(&source)
                    .map(|metadata| metadata.len())
                    .unwrap_or(0);
                let mut report = report_for(&config, ordinal, &source, logical_bytes, preflight);
                report.outcome = JobOutcome::Skipped;
                record_report(&metrics, &observer, report);
                config.presentation.finished();
            }
            Ok((task_id, (ordinal, source, preflight, Err(error)))) => {
                preflight_jobs.remove(&task_id);
                tracing::warn!(path=%source.display(), stage="preflight", %error, "file processing failed");
                summary.failed += 1;
                let logical_bytes = std::fs::metadata(&source)
                    .map(|metadata| metadata.len())
                    .unwrap_or(0);
                let mut report = report_for(&config, ordinal, &source, logical_bytes, preflight);
                report.error = Some(ReportError {
                    code: "preflight_failed".into(),
                    detail: error,
                });
                record_report(&metrics, &observer, report);
                config.presentation.finished();
            }
            Err(e) => {
                tracing::error!(error=%e, "preflight worker panicked");
                summary.failed += 1;
                if let Some((ordinal, source)) = preflight_jobs.remove(&e.id()) {
                    let mut report = report_for(
                        &config,
                        ordinal,
                        &source,
                        std::fs::metadata(&source)
                            .map(|metadata| metadata.len())
                            .unwrap_or(0),
                        std::time::Duration::ZERO,
                    );
                    report.error = Some(ReportError {
                        code: "preflight_worker_join_failed".into(),
                        detail: e.to_string(),
                    });
                    record_report(&metrics, &observer, report);
                }
                config.presentation.finished();
            }
        }
    }

    let mut ready = Vec::new();
    for (key, jobs) in grouped {
        if jobs.len() == 1 {
            ready.extend(jobs);
        } else {
            // Dropping staged jobs removes their owned temporary files.  Every
            // claimant fails, including with --overwrite: overwrite only governs
            // a pre-existing destination, never chooses a same-run winner.
            tracing::warn!(destination=%key.display(), claims=jobs.len(), "flat output collision");
            summary.conflicted += jobs.len();
            for measured in jobs {
                let mut report = report_for_planned(&config, &measured);
                report.outcome = JobOutcome::Conflict;
                report.error = Some(ReportError {
                    code: "same_run_collision".into(),
                    detail: format!(
                        "multiple sources claim {}",
                        measured.planned.destination.display()
                    ),
                });
                record_report(&metrics, &observer, report);
                config.presentation.finished();
            }
        }
    }
    ready.sort_by_key(|measured| measured.ordinal);
    if cancellation.is_cancelled() {
        for measured in &ready {
            record_report(&metrics, &observer, cancelled_report(&config, measured));
            config.presentation.finished();
        }
        emit_timing_reports(&metrics);
        observer.emit(BatchEvent::Completed {
            summary,
            cancelled: true,
        });
        return Ok(summary);
    }

    let mut publish = JoinSet::new();
    let mut in_flight: HashMap<Id, InFlightJob> = HashMap::new();
    let mut pending = ready.into_iter();
    let mut admissions_closed = false;
    let max_in_flight = config.jobs.get();

    loop {
        if cancellation.is_cancelled() && !admissions_closed {
            for remaining in pending.by_ref() {
                record_report(&metrics, &observer, cancelled_report(&config, &remaining));
                config.presentation.finished();
            }
            admissions_closed = true;
        }

        while !admissions_closed && publish.len() < max_in_flight {
            let Some(measured) = pending.next() else {
                admissions_closed = true;
                break;
            };
            if cancellation.is_cancelled() {
                record_report(&metrics, &observer, cancelled_report(&config, &measured));
                config.presentation.finished();
                for remaining in pending.by_ref() {
                    record_report(&metrics, &observer, cancelled_report(&config, &remaining));
                    config.presentation.finished();
                }
                admissions_closed = true;
                break;
            }
            let permit = resources
                .semaphore
                .clone()
                .acquire_owned()
                .await
                .map_err(|e| crate::error::AppError::Internal(e.to_string()))?;
            let source = measured.planned.source.clone();
            let destination = measured.planned.destination.clone();
            let ordinal = measured.ordinal;
            let item_kind = planned_item_kind(&measured.planned);
            let action = planned_action(&measured.planned);
            let relative = config
                .relative_input(&source)
                .unwrap_or_else(|_| source.clone());
            let phase = match &measured.planned.action {
                job::PlannedAction::Copy => "Copying",
                job::PlannedAction::Decrypt { .. } => "Decrypting",
            };
            if observer.is_enabled() {
                observer.emit(BatchEvent::ItemProcessing {
                    ordinal,
                    source: source.clone(),
                    relative_path: relative.clone(),
                    destination: destination.clone(),
                    item_kind,
                    action,
                });
            }
            config.presentation.started(&relative, phase);
            let context = InFlightJob {
                ordinal,
                source: source.clone(),
                destination: destination.clone(),
                item_kind,
                action,
                logical_bytes: measured.logical_bytes,
                preflight: measured.preflight,
            };
            let c = config.clone();
            let r = resources.reservations.clone();
            let keys = resources.keys.clone();
            let metadata = resources.metadata.clone();
            let cancellation = cancellation.clone();
            let capture_music_snapshots = observer.captures_music_snapshots();
            let dispatch = tracing::dispatcher::get_default(Clone::clone);
            let metrics = metrics.clone();
            let handle = publish.spawn_blocking(move || {
                let _permit = permit;
                metrics.enter();
                let mut timings = JobTimings::default();
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    tracing::dispatcher::with_default(&dispatch, || {
                        job::execute_cancellable_with_resources_report(
                            &c,
                            &keys,
                            &metadata,
                            &r,
                            measured.planned,
                            &cancellation,
                            &mut timings,
                            capture_music_snapshots,
                        )
                    })
                }))
                .unwrap_or_else(|_| Err("publication worker panicked".into()));
                metrics.leave();
                c.presentation.finished();
                (timings, result)
            });
            in_flight.insert(handle.id(), context);
        }

        let Some(result) = publish.join_next_with_id().await else {
            break;
        };
        match result {
            Ok((task_id, (mut timings, Ok(execution)))) => {
                let context = in_flight
                    .remove(&task_id)
                    .expect("published worker context must exist");
                let outcome = execution.summary;
                summary.merge(outcome);
                let mut report = report_for(
                    &config,
                    context.ordinal,
                    &context.source,
                    context.logical_bytes,
                    context.preflight,
                );
                timings.preflight = context.preflight;
                report.timings = timings;
                report.destination = Some(context.destination);
                report.item_kind = context.item_kind;
                report.action = Some(context.action);
                report.warnings = execution.warnings;
                report.music = execution.music;
                report.outcome = if outcome == Summary::default() && cancellation.is_cancelled() {
                    JobOutcome::Cancelled
                } else {
                    outcome_from_summary(outcome)
                };
                if report.outcome == JobOutcome::Cancelled {
                    report.error = Some(ReportError {
                        code: "cancelled".into(),
                        detail: "conversion was cancelled before publication".into(),
                    });
                }
                record_report(&metrics, &observer, report);
            }
            Ok((task_id, (mut timings, Err(error)))) => {
                let context = in_flight
                    .remove(&task_id)
                    .expect("published worker context must exist");
                tracing::warn!(stage="publication", %error, "file processing failed");
                if error.starts_with("collision") {
                    summary.conflicted += 1;
                } else {
                    summary.failed += 1;
                }
                let mut report = report_for(
                    &config,
                    context.ordinal,
                    &context.source,
                    context.logical_bytes,
                    context.preflight,
                );
                timings.preflight = context.preflight;
                report.timings = timings;
                report.destination = Some(context.destination);
                report.item_kind = context.item_kind;
                report.action = Some(context.action);
                report.outcome = if error.starts_with("collision") {
                    JobOutcome::Conflict
                } else {
                    JobOutcome::Failed
                };
                report.error = Some(ReportError {
                    code: if report.outcome == JobOutcome::Conflict {
                        "destination_collision".into()
                    } else {
                        "publication_failed".into()
                    },
                    detail: error,
                });
                record_report(&metrics, &observer, report);
            }
            Err(error) => {
                tracing::error!(%error, "publication worker panicked");
                summary.failed += 1;
                if let Some(context) = in_flight.remove(&error.id()) {
                    let mut report = report_for(
                        &config,
                        context.ordinal,
                        &context.source,
                        context.logical_bytes,
                        context.preflight,
                    );
                    report.destination = Some(context.destination);
                    report.item_kind = context.item_kind;
                    report.action = Some(context.action);
                    report.error = Some(ReportError {
                        code: "publication_worker_join_failed".into(),
                        detail: error.to_string(),
                    });
                    record_report(&metrics, &observer, report);
                    config.presentation.finished();
                }
            }
        }
    }
    emit_timing_reports(&metrics);
    observer.emit(BatchEvent::Completed {
        summary,
        cancelled: cancellation.is_cancelled(),
    });
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn metrics_tracks_balanced_lifecycle() {
        let m = RunnerMetrics::default();
        assert!(m.collector.is_none());
        m.enter();
        m.enter();
        m.leave();
        m.leave();
        assert_eq!(m.maxima(), (2, 2));
        assert_eq!(m.active_jobs.load(Ordering::SeqCst), 0);
    }
    #[tokio::test]
    async fn cancelled_runner_does_not_intake_work() {
        let temp = tempfile::tempdir().unwrap();
        let input = temp.path().join("in");
        let output = temp.path().join("out");
        std::fs::create_dir(&input).unwrap();
        let file = input.join("x.lrc");
        std::fs::write(&file, b"x").unwrap();
        let c = Arc::new(
            crate::config::RunConfig::new(
                input,
                Some(output.clone()),
                false,
                false,
                false,
                false,
                false,
                2,
                64,
                1,
                None,
                None,
                None,
                None,
                None,
            )
            .unwrap(),
        );
        let cancel = Cancellation::default();
        cancel.cancel();
        let events = Arc::new(Mutex::new(Vec::new()));
        let captured = events.clone();
        let summary = run_paths_with_metrics_and_cancel_and_observer(
            c,
            vec![file],
            Arc::new(RunnerMetrics::default()),
            cancel,
            BatchObserver::new(Arc::new(move |event| {
                captured.lock().unwrap().push(event);
            })),
        )
        .await
        .unwrap();
        assert_eq!(summary.copied, 0);
        assert!(!output.exists());
        let events = events.lock().unwrap();
        assert!(matches!(
            events[0],
            BatchEvent::PlanningStarted { total: 1 }
        ));
        assert!(matches!(
            &events[1],
            BatchEvent::ItemTerminal { report }
                if report.ordinal == 0 && report.outcome == JobOutcome::Cancelled
        ));
        assert!(matches!(
            events[2],
            BatchEvent::Completed {
                cancelled: true,
                ..
            }
        ));
    }

    #[tokio::test]
    async fn cancellation_after_admission_emits_one_cancelled_terminal_report() {
        let temp = tempfile::tempdir().unwrap();
        let input = temp.path().join("in");
        let output = temp.path().join("out");
        std::fs::create_dir(&input).unwrap();
        let file = input.join("large.lrc");
        std::fs::write(&file, vec![b'x'; 16 * 1024 * 1024]).unwrap();
        let config = Arc::new(
            crate::config::RunConfig::new(
                input,
                Some(output.clone()),
                false,
                false,
                false,
                false,
                false,
                1,
                64 * 1024,
                1,
                None,
                None,
                None,
                None,
                None,
            )
            .unwrap(),
        );
        let cancellation = Cancellation::default();
        let metrics = Arc::new(RunnerMetrics::default());
        let events = Arc::new(Mutex::new(Vec::new()));
        let captured = events.clone();
        let task = tokio::spawn(run_paths_with_metrics_and_cancel_and_observer(
            config,
            vec![file.canonicalize().unwrap()],
            metrics.clone(),
            cancellation.clone(),
            BatchObserver::new(Arc::new(move |event| {
                captured.lock().unwrap().push(event);
            })),
        ));

        tokio::time::timeout(std::time::Duration::from_secs(3), async {
            loop {
                if metrics.active_jobs.load(Ordering::SeqCst) > 0
                    && events
                        .lock()
                        .unwrap()
                        .iter()
                        .any(|event| matches!(event, BatchEvent::ItemProcessing { .. }))
                {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("runner should admit a worker");
        cancellation.cancel();
        let summary = task.await.unwrap().unwrap();

        assert_eq!(summary, Summary::default());
        assert!(!output.join("large.lrc").exists());
        let events = events.lock().unwrap();
        let terminals: Vec<_> = events
            .iter()
            .filter_map(|event| match event {
                BatchEvent::ItemTerminal { report } => Some(report),
                _ => None,
            })
            .collect();
        assert_eq!(terminals.len(), 1);
        assert_eq!(terminals[0].outcome, JobOutcome::Cancelled);
    }
    #[tokio::test]
    async fn runner_observes_bounded_real_parallel_jobs() {
        let temp = tempfile::tempdir().unwrap();
        let input = temp.path().join("in");
        let output = temp.path().join("out");
        std::fs::create_dir(&input).unwrap();
        let mut paths = Vec::new();
        for n in 0..6 {
            let p = input.join(format!("{n}.lrc"));
            std::fs::write(&p, vec![b'x'; 4 * 1024 * 1024]).unwrap();
            paths.push(p.canonicalize().unwrap());
        }
        let c = Arc::new(
            crate::config::RunConfig::new(
                input,
                Some(output),
                false,
                false,
                false,
                false,
                false,
                2,
                64 * 1024,
                1,
                None,
                None,
                None,
                None,
                None,
            )
            .unwrap(),
        );
        let m = Arc::new(RunnerMetrics::collecting(6));
        let s = run_paths_with_metrics(c, paths, m.clone()).await.unwrap();
        let (jobs, files) = m.maxima();
        let reports = m.reports();
        assert_eq!(s.copied, 6);
        assert!(jobs >= 2, "jobs={jobs}");
        assert!(jobs <= 2 && files <= 2);
        assert_eq!(m.active_jobs.load(Ordering::SeqCst), 0);
        assert_eq!(reports.len(), 6);
        assert!(reports.iter().all(|report| {
            report.logical_bytes == 4 * 1024 * 1024
                && report.outcome == JobOutcome::Copied
                && report.timings.preflight > std::time::Duration::ZERO
                && report.timings.copy > std::time::Duration::ZERO
                && report.timings.publish > std::time::Duration::ZERO
        }));
    }
    #[tokio::test]
    async fn runner_records_failed_preflight_with_relative_path_bytes_and_timing() {
        let temp = tempfile::tempdir().unwrap();
        let input = temp.path().join("in");
        let output = temp.path().join("out");
        std::fs::create_dir(&input).unwrap();
        let bad = input.join("bad.ncm");
        std::fs::write(&bad, b"not ncm").unwrap();
        let config = Arc::new(
            crate::config::RunConfig::new(
                input,
                Some(output),
                false,
                false,
                false,
                false,
                false,
                1,
                64,
                1,
                None,
                None,
                None,
                None,
                None,
            )
            .unwrap(),
        );
        let metrics = Arc::new(RunnerMetrics::collecting(1));
        let summary =
            run_paths_with_metrics(config, vec![bad.canonicalize().unwrap()], metrics.clone())
                .await
                .unwrap();
        assert_eq!(summary.failed, 1);
        assert_eq!(
            summary.converted + summary.copied + summary.skipped + summary.conflicted,
            0
        );
        let reports = metrics.reports();
        assert_eq!(reports.len(), 1);
        let report = &reports[0];
        assert_eq!(
            report,
            &JobReport {
                ordinal: 0,
                source: bad.canonicalize().unwrap(),
                relative_path: std::path::PathBuf::from("bad.ncm"),
                destination: None,
                item_kind: ItemKind::Music,
                action: None,
                logical_bytes: 7,
                outcome: JobOutcome::Failed,
                error: report.error.clone(),
                warnings: Vec::new(),
                music: Some(MusicSnapshot {
                    title: Some("bad".into()),
                    ..Default::default()
                }),
                timings: JobTimings {
                    preflight: report.timings.preflight,
                    ..JobTimings::default()
                },
            }
        );
        assert!(report.timings.preflight > std::time::Duration::ZERO);
    }
    #[tokio::test]
    async fn runner_records_every_flat_collision_claimant() {
        let temp = tempfile::tempdir().unwrap();
        let input = temp.path().join("in");
        let output = temp.path().join("out");
        for directory in [input.join("a"), input.join("b")] {
            std::fs::create_dir_all(&directory).unwrap();
            std::fs::write(directory.join("same.lrc"), b"abc").unwrap();
        }
        let config = Arc::new(
            crate::config::RunConfig::new(
                input.clone(),
                Some(output.clone()),
                true,
                false,
                false,
                false,
                false,
                2,
                64,
                1,
                None,
                None,
                None,
                None,
                None,
            )
            .unwrap(),
        );
        let paths = [input.join("a/same.lrc"), input.join("b/same.lrc")]
            .into_iter()
            .map(|path| path.canonicalize().unwrap())
            .collect();
        let metrics = Arc::new(RunnerMetrics::collecting(2));
        let events = Arc::new(Mutex::new(Vec::new()));
        let captured = events.clone();
        let summary = run_paths_with_metrics_and_cancel_and_observer(
            config,
            paths,
            metrics.clone(),
            Cancellation::default(),
            BatchObserver::new(Arc::new(move |event| {
                captured.lock().unwrap().push(event);
            })),
        )
        .await
        .unwrap();
        assert_eq!(summary.conflicted, 2);
        assert!(!output.exists());
        let mut reports = metrics.reports();
        reports.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
        assert_eq!(reports.len(), 2);
        assert_eq!(
            reports
                .iter()
                .map(|report| report.relative_path.clone())
                .collect::<Vec<_>>(),
            vec![
                std::path::PathBuf::from("a/same.lrc"),
                std::path::PathBuf::from("b/same.lrc"),
            ]
        );
        assert!(reports.iter().all(|report| {
            report.logical_bytes == 3
                && report.outcome == JobOutcome::Conflict
                && report.timings.preflight > std::time::Duration::ZERO
                && report.timings.decrypt == std::time::Duration::ZERO
                && report.timings.copy == std::time::Duration::ZERO
                && report.timings.publish == std::time::Duration::ZERO
        }));
        let terminal_outcomes: Vec<_> = events
            .lock()
            .unwrap()
            .iter()
            .filter_map(crate::app::events::terminal_outcome)
            .collect();
        assert_eq!(
            terminal_outcomes,
            vec![JobOutcome::Conflict, JobOutcome::Conflict]
        );
    }

    #[tokio::test]
    async fn observer_emits_one_terminal_report_per_admitted_path() {
        let temp = tempfile::tempdir().unwrap();
        let input = temp.path().join("in");
        let output = temp.path().join("out");
        std::fs::create_dir(&input).unwrap();
        let companion = input.join("notes.lrc");
        let invalid = input.join("bad.ncm");
        std::fs::write(&companion, b"notes").unwrap();
        std::fs::write(&invalid, b"not ncm").unwrap();
        let config = Arc::new(
            crate::config::RunConfig::new(
                input,
                Some(output),
                false,
                false,
                false,
                false,
                false,
                2,
                64,
                1,
                None,
                None,
                None,
                None,
                None,
            )
            .unwrap(),
        );
        let events = Arc::new(Mutex::new(Vec::new()));
        let captured = events.clone();
        let observer = BatchObserver::new(Arc::new(move |event| {
            captured.lock().unwrap().push(event);
        }));
        let paths = vec![
            companion.canonicalize().unwrap(),
            invalid.canonicalize().unwrap(),
        ];

        let summary = run_paths_with_observer(config, paths, observer)
            .await
            .unwrap();
        assert_eq!(summary.copied, 1);
        assert_eq!(summary.failed, 1);

        let events = events.lock().unwrap();
        let terminals: Vec<_> = events
            .iter()
            .filter_map(|event| match event {
                BatchEvent::ItemTerminal { report } => Some(report.as_ref()),
                _ => None,
            })
            .collect();
        assert_eq!(terminals.len(), 2);
        let mut terminal_by_ordinal = terminals;
        terminal_by_ordinal.sort_by_key(|report| report.ordinal);
        assert_eq!(
            terminal_by_ordinal
                .iter()
                .map(|report| report.ordinal)
                .collect::<Vec<_>>(),
            vec![0, 1]
        );
        assert_eq!(terminal_by_ordinal[0].outcome, JobOutcome::Copied);
        assert_eq!(terminal_by_ordinal[1].outcome, JobOutcome::Failed);
        assert_eq!(
            terminal_by_ordinal[1]
                .error
                .as_ref()
                .map(|error| error.code.as_str()),
            Some("preflight_failed")
        );
        assert!(matches!(
            events.last(),
            Some(BatchEvent::Completed {
                summary: Summary {
                    copied: 1,
                    failed: 1,
                    ..
                },
                cancelled: false,
            })
        ));
    }

    #[tokio::test]
    async fn terminal_skip_event_agrees_with_summary() {
        let temp = tempfile::tempdir().unwrap();
        let input = temp.path().join("in");
        let output = temp.path().join("out");
        std::fs::create_dir(&input).unwrap();
        std::fs::create_dir(&output).unwrap();
        let source = input.join("skip.lrc");
        std::fs::write(&source, b"new source").unwrap();
        std::fs::write(output.join("skip.lrc"), b"existing output").unwrap();
        let config = Arc::new(
            crate::config::RunConfig::new(
                input,
                Some(output),
                false,
                false,
                false,
                false,
                false,
                1,
                64,
                1,
                None,
                None,
                None,
                None,
                None,
            )
            .unwrap(),
        );
        let events = Arc::new(Mutex::new(Vec::new()));
        let captured = events.clone();
        let summary = run_paths_with_observer(
            config,
            vec![source.canonicalize().unwrap()],
            BatchObserver::new(Arc::new(move |event| {
                captured.lock().unwrap().push(event);
            })),
        )
        .await
        .unwrap();

        assert_eq!(
            summary,
            Summary {
                skipped: 1,
                ..Default::default()
            }
        );
        assert_eq!(
            events
                .lock()
                .unwrap()
                .iter()
                .filter_map(crate::app::events::terminal_outcome)
                .collect::<Vec<_>>(),
            vec![JobOutcome::Skipped]
        );
    }

    #[tokio::test]
    async fn terminal_conversion_event_agrees_with_summary() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("track.qmc0");
        let output = temp.path().join("out");
        let mut encrypted = b"fLaC".to_vec();
        encrypted.resize(4096, 0);
        crate::crypto::qmc::v1::decrypt(&mut encrypted, 0);
        std::fs::write(&source, encrypted).unwrap();
        let config = Arc::new(
            crate::config::RunConfig::new(
                source.clone(),
                Some(output.clone()),
                false,
                false,
                false,
                false,
                false,
                1,
                64,
                1,
                None,
                None,
                None,
                None,
                None,
            )
            .unwrap(),
        );
        let events = Arc::new(Mutex::new(Vec::new()));
        let captured = events.clone();
        let summary = run_paths_with_observer(
            config,
            vec![source.canonicalize().unwrap()],
            BatchObserver::new(Arc::new(move |event| {
                captured.lock().unwrap().push(event);
            })),
        )
        .await
        .unwrap();

        assert_eq!(
            summary,
            Summary {
                converted: 1,
                ..Default::default()
            }
        );
        let events = events.lock().unwrap();
        let reports: Vec<_> = events
            .iter()
            .filter_map(|event| match event {
                BatchEvent::ItemTerminal { report } => Some(report),
                _ => None,
            })
            .collect();
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].outcome, JobOutcome::Converted);
        assert_eq!(reports[0].item_kind, ItemKind::Music);
        assert_eq!(
            reports[0]
                .music
                .as_ref()
                .and_then(|music| music.title.as_deref()),
            Some("track")
        );
        assert!(reports[0].warnings.is_empty());
        assert!(output.join("track.flac").is_file());
    }
}
