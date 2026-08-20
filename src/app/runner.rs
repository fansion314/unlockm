use super::{job, Summary};
use crate::{
    config::RunConfig,
    error::Result,
    keys::KeySources,
    metadata::provider::qq::MetadataClient,
    pathing::{collision_key, Reservations},
};
use std::sync::atomic::AtomicBool;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::{Arc, Mutex},
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
    // Complete every encrypted input into an owned hidden staging file before
    // claiming destinations.  Native container detection can only happen after
    // decryption, so suffix-only reservations would let a copy and conversion
    // race to the same flat output.
    let mut seen = HashSet::new();
    for path in paths {
        if cancellation.is_cancelled() {
            break;
        }
        if !seen.insert(path.clone()) {
            continue;
        }
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
            let result = tracing::dispatcher::with_default(&dispatch, || {
                job::preflight_with_resources(&c, &keys, &metadata, &path)
            });
            metrics.leave();
            (path, result)
        });
    }
    let mut summary = Summary::default();
    let mut grouped: HashMap<PathBuf, Vec<job::PlannedJob>> = HashMap::new();
    while let Some(result) = set.join_next().await {
        match result {
            Ok((_source, Ok(Some(planned)))) => {
                grouped
                    .entry(collision_key(&planned.destination))
                    .or_default()
                    .push(planned);
            }
            Ok((_source, Ok(None))) => summary.skipped += 1,
            Ok((source, Err(error))) => {
                tracing::warn!(path=%source.display(), stage="preflight", %error, "file processing failed");
                summary.failed += 1;
            }
            Err(e) => {
                tracing::error!(error=%e, "preflight worker panicked");
                summary.failed += 1;
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
        }
    }
    if cancellation.is_cancelled() {
        return Ok(summary);
    }

    let mut publish = JoinSet::new();
    for planned in ready {
        if cancellation.is_cancelled() {
            break;
        }
        let permit = resources
            .semaphore
            .clone()
            .acquire_owned()
            .await
            .map_err(|e| crate::error::AppError::Internal(e.to_string()))?;
        let c = config.clone();
        let r = resources.reservations.clone();
        let cancellation = cancellation.clone();
        let dispatch = tracing::dispatcher::get_default(Clone::clone);
        let metrics = metrics.clone();
        publish.spawn_blocking(move || {
            let _permit = permit;
            metrics.enter();
            let result = tracing::dispatcher::with_default(&dispatch, || {
                job::execute_cancellable(&c, &r, planned, &cancellation)
            });
            metrics.leave();
            result
        });
    }
    while let Some(result) = publish.join_next().await {
        match result {
            Ok(Ok(outcome)) => summary.merge(outcome),
            Ok(Err(error)) => {
                tracing::warn!(stage="publication", %error, "file processing failed");
                if error.starts_with("collision") {
                    summary.conflicted += 1;
                } else {
                    summary.failed += 1;
                }
            }
            Err(error) => {
                tracing::error!(%error, "publication worker panicked");
                summary.failed += 1;
            }
        }
    }
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn metrics_tracks_balanced_lifecycle() {
        let m = RunnerMetrics::default();
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
        let m = Arc::new(RunnerMetrics::default());
        let s = run_paths_with_metrics(c, paths, m.clone()).await.unwrap();
        let (jobs, files) = m.maxima();
        assert_eq!(s.copied, 6);
        assert!(jobs >= 2, "jobs={jobs}");
        assert!(jobs <= 2 && files <= 2);
        assert_eq!(m.active_jobs.load(Ordering::SeqCst), 0);
    }
}
