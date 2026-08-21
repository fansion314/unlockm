use super::{JobOutcome, JobReport, JobTimings, Summary, job};
use crate::{
    config::RunConfig,
    error::Result,
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
use tokio::{sync::Semaphore, task::JoinSet};

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
    planned: job::PlannedJob,
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
    fn record(&self, report: JobReport) {
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
                collector.reports.push(report);
            }
        }
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
    source: &std::path::Path,
    logical_bytes: u64,
    preflight: std::time::Duration,
) -> JobReport {
    JobReport {
        relative_path: config
            .relative_input(source)
            .unwrap_or_else(|_| source.to_path_buf()),
        logical_bytes,
        outcome: JobOutcome::Failed,
        timings: JobTimings {
            preflight,
            ..Default::default()
        },
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
    let resources = RunnerResources::new(&config);
    run_paths_with_resources_and_cancel(
        config,
        paths,
        Arc::new(RunnerMetrics::default()),
        Cancellation::default(),
        resources,
    )
    .await
}
pub async fn run_paths_with_metrics(
    config: Arc<RunConfig>,
    paths: Vec<PathBuf>,
    metrics: Arc<RunnerMetrics>,
) -> Result<Summary> {
    let resources = RunnerResources::new(&config);
    run_paths_with_resources_and_cancel(config, paths, metrics, Cancellation::default(), resources)
        .await
}
pub async fn run_paths_with_metrics_and_cancel(
    config: Arc<RunConfig>,
    paths: Vec<PathBuf>,
    metrics: Arc<RunnerMetrics>,
    cancellation: Cancellation,
) -> Result<Summary> {
    let resources = RunnerResources::new(&config);
    run_paths_with_resources_and_cancel(config, paths, metrics, cancellation, resources).await
}

pub async fn run_paths_with_resources_and_cancel(
    config: Arc<RunConfig>,
    paths: Vec<PathBuf>,
    metrics: Arc<RunnerMetrics>,
    cancellation: Cancellation,
    resources: RunnerResources,
) -> Result<Summary> {
    let mut set = JoinSet::new();
    // Resolve every encrypted input through a bounded decrypted-prefix probe
    // before claiming destinations. Suffix-only reservations would let a copy
    // and conversion race to the same flat output, while full stages here would
    // retain corpus-sized temporary data behind the collision barrier.
    let mut seen = HashSet::new();
    for path in paths {
        if cancellation.is_cancelled() {
            break;
        }
        if !seen.insert(path.clone()) {
            continue;
        }
        let relative = config
            .relative_input(&path)
            .unwrap_or_else(|_| path.clone());
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
        set.spawn_blocking(move || {
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
            (path, started.elapsed(), result)
        });
    }
    let mut summary = Summary::default();
    let mut grouped: HashMap<PathBuf, Vec<MeasuredPlannedJob>> = HashMap::new();
    while let Some(result) = set.join_next().await {
        match result {
            Ok((source, preflight, Ok(Some(planned)))) => {
                let logical_bytes = std::fs::metadata(&source)
                    .map(|metadata| metadata.len())
                    .unwrap_or(0);
                grouped
                    .entry(collision_key(&planned.destination))
                    .or_default()
                    .push(MeasuredPlannedJob {
                        planned,
                        logical_bytes,
                        preflight,
                    });
            }
            Ok((source, preflight, Ok(None))) => {
                summary.skipped += 1;
                let logical_bytes = std::fs::metadata(&source)
                    .map(|metadata| metadata.len())
                    .unwrap_or(0);
                let mut report = report_for(&config, &source, logical_bytes, preflight);
                report.outcome = JobOutcome::Skipped;
                metrics.record(report);
                config.presentation.finished();
            }
            Ok((source, preflight, Err(error))) => {
                tracing::warn!(path=%source.display(), stage="preflight", %error, "file processing failed");
                summary.failed += 1;
                let logical_bytes = std::fs::metadata(&source)
                    .map(|metadata| metadata.len())
                    .unwrap_or(0);
                metrics.record(report_for(&config, &source, logical_bytes, preflight));
                config.presentation.finished();
            }
            Err(e) => {
                tracing::error!(error=%e, "preflight worker panicked");
                summary.failed += 1;
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
                let mut report = report_for(
                    &config,
                    &measured.planned.source,
                    measured.logical_bytes,
                    measured.preflight,
                );
                report.outcome = JobOutcome::Conflict;
                metrics.record(report);
                config.presentation.finished();
            }
        }
    }
    if cancellation.is_cancelled() {
        for _ in &ready {
            config.presentation.finished();
        }
        emit_timing_reports(&metrics);
        return Ok(summary);
    }

    let mut publish = JoinSet::new();
    for measured in ready {
        if cancellation.is_cancelled() {
            config.presentation.finished();
            break;
        }
        let permit = resources
            .semaphore
            .clone()
            .acquire_owned()
            .await
            .map_err(|e| crate::error::AppError::Internal(e.to_string()))?;
        let source = measured.planned.source.clone();
        let relative = config
            .relative_input(&source)
            .unwrap_or_else(|_| source.clone());
        let phase = match &measured.planned.action {
            job::PlannedAction::Copy => "Copying",
            job::PlannedAction::Decrypt { .. } => "Decrypting",
        };
        config.presentation.started(&relative, phase);
        let logical_bytes = measured.logical_bytes;
        let preflight = measured.preflight;
        let c = config.clone();
        let r = resources.reservations.clone();
        let keys = resources.keys.clone();
        let metadata = resources.metadata.clone();
        let cancellation = cancellation.clone();
        let dispatch = tracing::dispatcher::get_default(Clone::clone);
        let metrics = metrics.clone();
        publish.spawn_blocking(move || {
            let _permit = permit;
            metrics.enter();
            let mut timings = JobTimings::default();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                tracing::dispatcher::with_default(&dispatch, || {
                    job::execute_cancellable_with_resources(
                        &c,
                        &keys,
                        &metadata,
                        &r,
                        measured.planned,
                        &cancellation,
                        &mut timings,
                    )
                })
            }))
            .unwrap_or_else(|_| Err("publication worker panicked".into()));
            metrics.leave();
            // A completed worker releases its UI lifecycle immediately. The
            // scheduler may still be admitting later files and therefore may
            // not drain this JoinSet result until much later.
            c.presentation.finished();
            (source, logical_bytes, preflight, timings, result)
        });
    }
    while let Some(result) = publish.join_next().await {
        match result {
            Ok((source, logical_bytes, preflight, mut timings, Ok(outcome))) => {
                summary.merge(outcome);
                let mut report = report_for(&config, &source, logical_bytes, preflight);
                timings.preflight = preflight;
                report.timings = timings;
                report.outcome = outcome_from_summary(outcome);
                metrics.record(report);
            }
            Ok((source, logical_bytes, preflight, mut timings, Err(error))) => {
                tracing::warn!(stage="publication", %error, "file processing failed");
                if error.starts_with("collision") {
                    summary.conflicted += 1;
                } else {
                    summary.failed += 1;
                }
                let mut report = report_for(&config, &source, logical_bytes, preflight);
                timings.preflight = preflight;
                report.timings = timings;
                report.outcome = if error.starts_with("collision") {
                    JobOutcome::Conflict
                } else {
                    JobOutcome::Failed
                };
                metrics.record(report);
            }
            Err(error) => {
                tracing::error!(%error, "publication worker panicked");
                summary.failed += 1;
            }
        }
    }
    emit_timing_reports(&metrics);
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
        let summary = run_paths_with_metrics_and_cancel(
            c,
            vec![file],
            Arc::new(RunnerMetrics::default()),
            cancel,
        )
        .await
        .unwrap();
        assert_eq!(summary.copied, 0);
        assert!(!output.exists());
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
                relative_path: std::path::PathBuf::from("bad.ncm"),
                logical_bytes: 7,
                outcome: JobOutcome::Failed,
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
        let summary = run_paths_with_metrics(config, paths, metrics.clone())
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
    }
}
