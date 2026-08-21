//! Platform-specific notify events are normalised before debounce and state
//! handling so every backend follows the same batch-equivalent pipeline.
use super::{Summary, runner, scan};
use crate::{
    config::RunConfig,
    error::{AppError, Result},
    io::atomic::is_owned_temp,
};
use notify::{
    Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher,
    event::{ModifyKind, RenameMode},
};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, SystemTime},
};
use tokio::{sync::mpsc, task::JoinSet, time::Instant};

const DEBOUNCE: Duration = Duration::from_millis(250);
const STABLE_CHECK: Duration = Duration::from_millis(100);
const STABLE_WINDOW: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, PartialEq, Eq)]
enum Action {
    Upsert(PathBuf),
    Remove(PathBuf),
}

fn normalize(event: &Event) -> Vec<Action> {
    let upserts = || event.paths.iter().cloned().map(Action::Upsert).collect();
    match &event.kind {
        EventKind::Create(_) => upserts(),
        EventKind::Modify(ModifyKind::Name(RenameMode::From)) => {
            event.paths.iter().cloned().map(Action::Remove).collect()
        }
        EventKind::Modify(ModifyKind::Name(RenameMode::To)) => upserts(),
        EventKind::Modify(ModifyKind::Name(RenameMode::Both))
        | EventKind::Modify(ModifyKind::Name(_))
            if event.paths.len() > 1 =>
        {
            let mut paths = event.paths.iter();
            let mut actions = Vec::new();
            if let Some(from) = paths.next() {
                actions.push(Action::Remove(from.clone()));
            }
            actions.extend(paths.cloned().map(Action::Upsert));
            actions
        }
        EventKind::Modify(_) => upserts(),
        EventKind::Remove(_) => event.paths.iter().cloned().map(Action::Remove).collect(),
        _ => Vec::new(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Queued,
    Running,
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct Fingerprint {
    len: u64,
    modified: Option<SystemTime>,
}
fn fingerprint(path: &Path) -> Option<Fingerprint> {
    std::fs::metadata(path)
        .ok()
        .filter(|m| m.is_file())
        .map(|m| Fingerprint {
            len: m.len(),
            modified: m.modified().ok(),
        })
}
struct PathState {
    phase: Phase,
    due: Instant,
    deadline: Instant,
    observed: Option<Fingerprint>,
    fingerprint: Option<Fingerprint>,
    dirty: bool,
    cancellation: runner::Cancellation,
}
fn ignored(config: &RunConfig, path: &Path) -> bool {
    is_owned_temp(path)
        || (config.output_is_proper_child() && path.starts_with(&config.output))
        || !path.starts_with(&config.input)
}
/// FSEvents can report `/var/...` while configuration canonicalization returns
/// `/private/var/...`. Normalize existing paths, or their existing parent for
/// a remove/rename-from, before applying containment checks.
fn normalize_event_path(path: PathBuf) -> PathBuf {
    if let Ok(path) = path.canonicalize() {
        return path;
    }
    if let (Some(parent), Some(name)) = (path.parent(), path.file_name())
        && let Ok(parent) = parent.canonicalize()
    {
        return parent.join(name);
    }
    path
}
fn queue(
    states: &mut HashMap<PathBuf, PathState>,
    path: PathBuf,
    fingerprint: Fingerprint,
    now: Instant,
) {
    match states.get_mut(&path) {
        Some(state) if state.phase == Phase::Running => {
            if state.fingerprint.as_ref() != Some(&fingerprint) {
                state.fingerprint = Some(fingerprint);
                state.dirty = true;
                state.cancellation.cancel();
            }
        }
        Some(state) => {
            if state.fingerprint.as_ref() == Some(&fingerprint) {
                return;
            }
            state.due = now + DEBOUNCE;
            state.deadline = now + STABLE_WINDOW;
            state.observed = None;
            state.fingerprint = Some(fingerprint);
        }
        None => {
            states.insert(
                path,
                PathState {
                    phase: Phase::Queued,
                    due: now + DEBOUNCE,
                    deadline: now + STABLE_WINDOW,
                    observed: None,
                    fingerprint: Some(fingerprint),
                    dirty: false,
                    cancellation: runner::Cancellation::default(),
                },
            );
        }
    }
}
fn remove(
    states: &mut HashMap<PathBuf, PathState>,
    known: &mut HashMap<PathBuf, Fingerprint>,
    path: &Path,
) {
    let victims = states
        .keys()
        .filter(|candidate| *candidate == path || candidate.starts_with(path))
        .cloned()
        .collect::<Vec<_>>();
    for victim in victims {
        if let Some(state) = states.remove(&victim) {
            state.cancellation.cancel();
        }
    }
    known.retain(|candidate, _| candidate != path && !candidate.starts_with(path));
}
fn enqueue_if_changed(
    states: &mut HashMap<PathBuf, PathState>,
    known: &HashMap<PathBuf, Fingerprint>,
    path: PathBuf,
    now: Instant,
) {
    let Some(current) = fingerprint(&path) else {
        return;
    };
    if states.contains_key(&path) || known.get(&path) != Some(&current) {
        queue(states, path, current, now);
    }
}
fn schedule(
    config: &RunConfig,
    states: &mut HashMap<PathBuf, PathState>,
    known: &mut HashMap<PathBuf, Fingerprint>,
    action: Action,
    now: Instant,
) -> Result<()> {
    match action {
        Action::Remove(path) => remove(states, known, &normalize_event_path(path)),
        Action::Upsert(path) if ignored(config, &normalize_event_path(path.clone())) => {}
        Action::Upsert(path) if normalize_event_path(path.clone()).is_dir() => {
            for child in scan::enumerate(config)? {
                enqueue_if_changed(states, known, child, now);
            }
        }
        Action::Upsert(path) => enqueue_if_changed(states, known, normalize_event_path(path), now),
    }
    Ok(())
}
fn requeue_after_dirty(state: &mut PathState) {
    let now = Instant::now();
    state.phase = Phase::Queued;
    state.dirty = false;
    state.observed = None;
    // The last fingerprint is retained until the follow-up reaches the
    // stability gate, preventing reconcile from creating a second follow-up.
    state.due = now + DEBOUNCE;
    state.deadline = now + STABLE_WINDOW;
    state.cancellation = runner::Cancellation::default();
}

async fn run_loop(
    config: Arc<RunConfig>,
    mut summary: Summary,
    cancellation: runner::Cancellation,
    ctrl_c: bool,
    ready: Option<tokio::sync::oneshot::Sender<()>>,
) -> Result<Summary> {
    // Snapshot before subscribing. Anything that appears in the setup gap
    // differs from this snapshot and is picked up by notify or reconciliation;
    // snapshotting after subscribing can lose a just-created source.
    let mut known = scan::enumerate(&config)?
        .into_iter()
        .filter_map(|path| fingerprint(&path).map(|value| (path, value)))
        .collect::<HashMap<_, _>>();
    let (tx, mut rx) = mpsc::unbounded_channel::<notify::Result<Event>>();
    let mut watcher = RecommendedWatcher::new(
        move |event| {
            let _ = tx.send(event);
        },
        Config::default(),
    )
    .map_err(|e| AppError::Watch(e.to_string()))?;
    watcher
        .watch(&config.input, RecursiveMode::Recursive)
        .map_err(|e| AppError::Watch(e.to_string()))?;
    if let Some(ready) = ready {
        let _ = ready.send(());
    }
    let resources = runner::RunnerResources::new(&config);
    let mut states: HashMap<PathBuf, PathState> = HashMap::new();
    let mut jobs: JoinSet<(PathBuf, Result<Summary>)> = JoinSet::new();
    let mut tick = tokio::time::interval(Duration::from_millis(25));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    // Notifications are normally sufficient, but a low-frequency reconciliation
    // closes backend loss/subscribe races without changing the per-file path.
    let mut reconcile = tokio::time::interval(Duration::from_secs(1));
    reconcile.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c(), if ctrl_c => cancellation.cancel(),
            _ = tick.tick() => {
                if cancellation.is_cancelled() {
                    for state in states.values() { state.cancellation.cancel(); }
                    while let Some(result) = jobs.join_next().await { if let Ok((_path, Ok(outcome))) = result { summary.merge(outcome); } }
                    return Ok(summary);
                }
                let now = Instant::now();
                let due = states.iter().filter_map(|(path, state)| (state.phase == Phase::Queued && state.due <= now).then_some(path.clone())).collect::<Vec<_>>();
                for path in due {
                    let Some(state) = states.get_mut(&path) else { continue };
                    let Some(current) = fingerprint(&path) else { remove(&mut states, &mut known, &path); continue };
                    if state.fingerprint.as_ref() != Some(&current) {
                        state.fingerprint = Some(current.clone());
                        state.observed = None;
                    }
                    if state.observed.as_ref() == Some(&current) {
                        state.phase = Phase::Running;
                        known.insert(path.clone(), current);
                        let c = config.clone(); let resources = resources.clone(); let child_cancel = state.cancellation.clone();
                        jobs.spawn(async move { let result = runner::run_paths_with_resources_and_cancel(c, vec![path.clone()], Arc::new(runner::RunnerMetrics::default()), child_cancel, resources).await; (path, result) });
                    } else if now >= state.deadline {
                        tracing::warn!(path=%path.display(), "watch stability window expired; ignoring event"); states.remove(&path);
                    } else { state.observed = Some(current); state.due = now + STABLE_CHECK; }
                }
            }
            _ = reconcile.tick() => {
                let now = Instant::now();
                let paths = scan::enumerate(&config)?;
                let visible = paths.iter().cloned().collect::<std::collections::HashSet<_>>();
                let removed = known.keys().filter(|path| !visible.contains(*path)).cloned().collect::<Vec<_>>();
                for path in removed { remove(&mut states, &mut known, &path); }
                for path in paths { enqueue_if_changed(&mut states, &known, path, now); }
            }
            event = rx.recv() => {
                let Some(event) = event else { return Err(AppError::Watch("watch channel closed".into())); };
                let event = event.map_err(|e| AppError::Watch(e.to_string()))?;
                for action in normalize(&event) { schedule(&config, &mut states, &mut known, action, Instant::now())?; }
            }
            result = jobs.join_next(), if !jobs.is_empty() => match result {
                Some(Ok((path, Ok(outcome)))) => { summary.merge(outcome); if let Some(state) = states.get_mut(&path) { if state.dirty { requeue_after_dirty(state); } else { states.remove(&path); } } }
                Some(Ok((path, Err(error)))) => { tracing::warn!(path=%path.display(), %error, "watch file processing failed"); states.remove(&path); summary.failed += 1; }
                Some(Err(error)) => { tracing::error!(%error, "watch worker panicked"); summary.failed += 1; }
                None => {}
            }
        }
    }
}
pub async fn run(config: Arc<RunConfig>, summary: Summary) -> Result<Summary> {
    run_loop(config, summary, runner::Cancellation::default(), true, None).await
}
#[doc(hidden)]
pub async fn run_until_cancelled(
    config: Arc<RunConfig>,
    summary: Summary,
    cancellation: runner::Cancellation,
) -> Result<Summary> {
    run_loop(config, summary, cancellation, false, None).await
}
#[doc(hidden)]
pub async fn run_until_cancelled_ready(
    config: Arc<RunConfig>,
    summary: Summary,
    cancellation: runner::Cancellation,
    ready: tokio::sync::oneshot::Sender<()>,
) -> Result<Summary> {
    run_loop(config, summary, cancellation, false, Some(ready)).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use notify::event::{CreateKind, DataChange, RemoveKind};
    fn event(kind: EventKind, paths: &[&str]) -> Event {
        let mut event = Event::new(kind);
        event.paths = paths.iter().map(PathBuf::from).collect();
        event
    }
    #[test]
    fn normalizes_create_modify_rename_and_remove() {
        assert_eq!(
            normalize(&event(EventKind::Create(CreateKind::File), &["new"])),
            vec![Action::Upsert("new".into())]
        );
        assert_eq!(
            normalize(&event(
                EventKind::Modify(ModifyKind::Data(DataChange::Any)),
                &["edit"]
            )),
            vec![Action::Upsert("edit".into())]
        );
        assert_eq!(
            normalize(&event(
                EventKind::Modify(ModifyKind::Name(RenameMode::From)),
                &["old"]
            )),
            vec![Action::Remove("old".into())]
        );
        assert_eq!(
            normalize(&event(
                EventKind::Modify(ModifyKind::Name(RenameMode::To)),
                &["new"]
            )),
            vec![Action::Upsert("new".into())]
        );
        assert_eq!(
            normalize(&event(
                EventKind::Modify(ModifyKind::Name(RenameMode::Both)),
                &["old", "new"]
            )),
            vec![Action::Remove("old".into()), Action::Upsert("new".into())]
        );
        assert_eq!(
            normalize(&event(EventKind::Remove(RemoveKind::Folder), &["dir"])),
            vec![Action::Remove("dir".into())]
        );
    }
    #[test]
    fn running_source_change_cancels_stale_job_and_requeue_resets_token() {
        let path = PathBuf::from("changed.ncm");
        let now = Instant::now();
        let mut states = HashMap::new();
        states.insert(
            path.clone(),
            PathState {
                phase: Phase::Running,
                due: now,
                deadline: now + STABLE_WINDOW,
                observed: None,
                fingerprint: Some(Fingerprint {
                    len: 10,
                    modified: None,
                }),
                dirty: false,
                cancellation: runner::Cancellation::default(),
            },
        );
        queue(
            &mut states,
            path.clone(),
            Fingerprint {
                len: 11,
                modified: None,
            },
            now,
        );
        let state = states.get_mut(&path).unwrap();
        assert!(state.dirty);
        assert!(state.cancellation.is_cancelled());
        requeue_after_dirty(state);
        assert_eq!(state.phase, Phase::Queued);
        assert!(!state.cancellation.is_cancelled());
    }
    #[test]
    fn remove_cancels_running_descendants() {
        let now = Instant::now();
        let cancelled = runner::Cancellation::default();
        let mut states = HashMap::new();
        states.insert(
            PathBuf::from("dir/song"),
            PathState {
                phase: Phase::Running,
                due: now,
                deadline: now,
                observed: None,
                fingerprint: None,
                dirty: false,
                cancellation: cancelled.clone(),
            },
        );
        let mut known = HashMap::new();
        remove(&mut states, &mut known, Path::new("dir"));
        assert!(states.is_empty());
        assert!(cancelled.is_cancelled());
    }
}
