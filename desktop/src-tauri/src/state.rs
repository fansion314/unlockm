use crate::history::{CreateRun, HistoryError, HistoryStore, RunStatus, RunSummary};
use std::{
    sync::{Arc, Mutex, MutexGuard},
    time::SystemTime,
};
use thiserror::Error;
use unlockm::app::runner::Cancellation;

#[derive(Debug, Error)]
pub enum StateError {
    #[error("a conversion is already active (run {active_run_id})")]
    Busy { active_run_id: i64 },
    #[error("run {run_id} is not the active conversion")]
    NotActive { run_id: i64 },
    #[error(transparent)]
    History(#[from] HistoryError),
}

#[derive(Clone)]
pub struct AppState {
    history: Arc<HistoryStore>,
    active: Arc<Mutex<Option<ActiveRun>>>,
}

#[derive(Clone)]
pub struct ActiveRun {
    pub run_id: i64,
    pub cancellation: Cancellation,
    pub started_at: SystemTime,
}

/// Owns the idle active-run lock for the full duration of a history mutation.
///
/// Keeping this guard alive makes starting a conversion mutually exclusive
/// with mutations that must agree with both an output file and SQLite.
pub struct IdleHistoryMutationGuard<'a> {
    _active: MutexGuard<'a, Option<ActiveRun>>,
    history: &'a HistoryStore,
}

impl IdleHistoryMutationGuard<'_> {
    pub fn history(&self) -> &HistoryStore {
        self.history
    }
}

impl AppState {
    pub fn new(history: HistoryStore) -> Result<Self, HistoryError> {
        history.recover_interrupted()?;
        Ok(Self {
            history: Arc::new(history),
            active: Arc::new(Mutex::new(None)),
        })
    }

    pub fn history(&self) -> &Arc<HistoryStore> {
        &self.history
    }

    pub fn begin_run(&self, request: CreateRun) -> Result<ActiveRun, StateError> {
        let mut active = self.active.lock().expect("active run lock");
        if let Some(existing) = active.as_ref() {
            return Err(StateError::Busy {
                active_run_id: existing.run_id,
            });
        }
        let run = self.history.create_run(request)?;
        let active_run = ActiveRun {
            run_id: run.id,
            cancellation: Cancellation::default(),
            started_at: SystemTime::now(),
        };
        *active = Some(active_run.clone());
        Ok(active_run)
    }

    /// Atomically reset a terminal run for a same-ID replay while holding the
    /// one-active-run lock that also gates history mutations and new runs.
    pub fn begin_rerun(&self, run_id: i64) -> Result<ActiveRun, StateError> {
        let mut active = self.active.lock().expect("active run lock");
        if let Some(existing) = active.as_ref() {
            return Err(StateError::Busy {
                active_run_id: existing.run_id,
            });
        }
        self.history
            .reset_run_for_rerun(run_id)?
            .ok_or(StateError::NotActive { run_id })?;
        let active_run = ActiveRun {
            run_id,
            cancellation: Cancellation::default(),
            started_at: SystemTime::now(),
        };
        *active = Some(active_run.clone());
        Ok(active_run)
    }

    /// Starts a history mutation only while no conversion is active. The
    /// returned guard must cover every related file and database operation.
    pub fn begin_history_mutation(&self) -> Result<IdleHistoryMutationGuard<'_>, StateError> {
        let active = self.active.lock().expect("active run lock");
        if let Some(existing) = active.as_ref() {
            return Err(StateError::Busy {
                active_run_id: existing.run_id,
            });
        }
        Ok(IdleHistoryMutationGuard {
            _active: active,
            history: self.history.as_ref(),
        })
    }

    pub fn mark_run_running(&self, run_id: i64, total_items: usize) -> Result<(), StateError> {
        let active = self.active.lock().expect("active run lock");
        match active.as_ref() {
            Some(current) if current.run_id == run_id => {}
            _ => return Err(StateError::NotActive { run_id }),
        }
        self.history.set_total_items(run_id, total_items)?;
        self.history.mark_running(run_id)?;
        Ok(())
    }

    pub fn active_run(&self) -> Option<ActiveRun> {
        self.active.lock().expect("active run lock").clone()
    }

    /// Returns false for an inactive/stale id, making repeated cancellation
    /// requests safe for the webview and window-lifecycle races.
    pub fn cancel_run(&self, run_id: i64) -> bool {
        let active = self.active.lock().expect("active run lock");
        let Some(active) = active.as_ref().filter(|active| active.run_id == run_id) else {
            return false;
        };
        active.cancellation.cancel();
        true
    }

    pub fn finish_run(
        &self,
        run_id: i64,
        status: RunStatus,
        error_code: Option<&str>,
        error_detail: Option<&str>,
    ) -> Result<RunSummary, StateError> {
        let mut active = self.active.lock().expect("active run lock");
        match active.as_ref() {
            Some(current) if current.run_id == run_id => {}
            _ => return Err(StateError::NotActive { run_id }),
        }
        let write_result = self
            .history
            .finish_run(run_id, status, error_code, error_detail);
        // Never leave the process permanently busy if SQLite's terminal write
        // fails. The next startup recovery will retain the active row as
        // interrupted rather than pretending that it completed.
        *active = None;
        write_result?;
        self.history
            .run(run_id)?
            .ok_or(StateError::NotActive { run_id })
    }

    /// Clears a stale active slot after an application-level coordinator
    /// error. The terminal history row is written before the slot is released.
    pub fn fail_active_run(&self, run_id: i64, detail: &str) -> Result<RunSummary, StateError> {
        self.finish_run(
            run_id,
            RunStatus::Failed,
            Some("desktop_coordinator_failed"),
            Some(detail),
        )
    }

    pub fn delete_history_run(&self, run_id: i64) -> Result<bool, StateError> {
        let mutation = self.begin_history_mutation()?;
        wait_for_history_mutation_barrier();
        mutation.history().delete_run(run_id).map_err(Into::into)
    }

    pub fn clear_history(&self) -> Result<(), StateError> {
        let mutation = self.begin_history_mutation()?;
        wait_for_history_mutation_barrier();
        mutation.history().clear_history().map_err(Into::into)
    }
}

#[cfg(test)]
static HISTORY_MUTATION_BARRIER: std::sync::OnceLock<Mutex<Option<Arc<std::sync::Barrier>>>> =
    std::sync::OnceLock::new();

#[cfg(test)]
fn set_history_mutation_barrier(barrier: Option<Arc<std::sync::Barrier>>) {
    *HISTORY_MUTATION_BARRIER
        .get_or_init(|| Mutex::new(None))
        .lock()
        .expect("history mutation barrier lock") = barrier;
}

#[cfg(test)]
fn wait_for_history_mutation_barrier() {
    let barrier = HISTORY_MUTATION_BARRIER
        .get_or_init(|| Mutex::new(None))
        .lock()
        .expect("history mutation barrier lock")
        .clone();
    if let Some(barrier) = barrier {
        barrier.wait();
        barrier.wait();
    }
}

#[cfg(not(test))]
fn wait_for_history_mutation_barrier() {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::{InputKind, RunSettings};

    fn request() -> CreateRun {
        CreateRun {
            input_kind: InputKind::File,
            input_path: "/input/track.ncm".into(),
            output_path: "/output".into(),
            settings: RunSettings {
                update_metadata: false,
                copy_passthrough: true,
                overwrite: false,
            },
            total_items: 1,
        }
    }

    #[test]
    fn state_enforces_one_active_run_and_idempotent_cancellation() {
        let directory = tempfile::tempdir().unwrap();
        let state =
            AppState::new(HistoryStore::open(directory.path().join("history.sqlite")).unwrap())
                .unwrap();
        let active = state.begin_run(request()).unwrap();
        assert!(matches!(
            state.begin_run(request()),
            Err(StateError::Busy { active_run_id }) if active_run_id == active.run_id
        ));
        assert!(!state.cancel_run(active.run_id + 1));
        assert!(state.cancel_run(active.run_id));
        assert!(state.cancel_run(active.run_id));
        assert!(active.cancellation.is_cancelled());
        let terminal = state
            .finish_run(active.run_id, RunStatus::Cancelled, None, None)
            .unwrap();
        assert_eq!(terminal.status, RunStatus::Cancelled);
        assert!(state.begin_run(request()).is_ok());
    }

    #[test]
    fn terminal_write_failure_still_releases_the_active_slot() {
        let directory = tempfile::tempdir().unwrap();
        let state =
            AppState::new(HistoryStore::open(directory.path().join("history.sqlite")).unwrap())
                .unwrap();
        let active = state.begin_run(request()).unwrap();
        state
            .history()
            .execute_test_sql(
                "CREATE TRIGGER fail_terminal_update BEFORE UPDATE ON runs
                 BEGIN SELECT RAISE(ABORT, 'injected terminal failure'); END;",
            )
            .unwrap();
        assert!(
            state
                .finish_run(active.run_id, RunStatus::Failed, None, None)
                .is_err()
        );
        assert!(state.active_run().is_none());
        state
            .history()
            .execute_test_sql("DROP TRIGGER fail_terminal_update")
            .unwrap();
        assert!(state.begin_run(request()).is_ok());
    }

    #[test]
    fn active_history_rows_cannot_be_deleted_or_cleared() {
        let directory = tempfile::tempdir().unwrap();
        let state =
            AppState::new(HistoryStore::open(directory.path().join("history.sqlite")).unwrap())
                .unwrap();
        let active = state.begin_run(request()).unwrap();
        assert!(matches!(
            state.delete_history_run(active.run_id),
            Err(StateError::Busy { .. })
        ));
        assert!(matches!(
            state.clear_history(),
            Err(StateError::Busy { .. })
        ));
        assert!(state.history().run(active.run_id).unwrap().is_some());
        assert!(state.cancel_run(active.run_id));
    }

    #[test]
    fn history_mutation_guard_rejects_active_runs_and_blocks_conversion_start() {
        let directory = tempfile::tempdir().unwrap();
        let state =
            AppState::new(HistoryStore::open(directory.path().join("history.sqlite")).unwrap())
                .unwrap();

        let mutation = state.begin_history_mutation().unwrap();
        let (sender, receiver) = std::sync::mpsc::channel();
        let begin_state = state.clone();
        let begin = std::thread::spawn(move || {
            sender.send(begin_state.begin_run(request())).unwrap();
        });
        assert!(
            receiver
                .recv_timeout(std::time::Duration::from_millis(30))
                .is_err()
        );
        drop(mutation);
        let active = receiver
            .recv_timeout(std::time::Duration::from_secs(1))
            .unwrap()
            .unwrap();
        begin.join().unwrap();
        assert!(matches!(
            state.begin_history_mutation(),
            Err(StateError::Busy { active_run_id }) if active_run_id == active.run_id
        ));
        state
            .finish_run(active.run_id, RunStatus::Completed, None, None)
            .unwrap();
    }

    #[test]
    fn history_mutation_guards_serialize_idle_writers() {
        let directory = tempfile::tempdir().unwrap();
        let state =
            AppState::new(HistoryStore::open(directory.path().join("history.sqlite")).unwrap())
                .unwrap();
        let first = state.begin_history_mutation().unwrap();
        let (sender, receiver) = std::sync::mpsc::channel();
        let second_state = state.clone();
        let second = std::thread::spawn(move || {
            let guard = second_state.begin_history_mutation();
            sender.send(guard.is_ok()).unwrap();
        });
        assert!(
            receiver
                .recv_timeout(std::time::Duration::from_millis(30))
                .is_err()
        );
        drop(first);
        assert!(
            receiver
                .recv_timeout(std::time::Duration::from_secs(1))
                .unwrap()
        );
        second.join().unwrap();
    }

    #[test]
    fn rerun_resets_a_terminal_run_in_place_and_obeys_the_history_mutation_gate() {
        let directory = tempfile::tempdir().unwrap();
        let state =
            AppState::new(HistoryStore::open(directory.path().join("history.sqlite")).unwrap())
                .unwrap();
        let original = state.begin_run(request()).unwrap();
        state
            .finish_run(original.run_id, RunStatus::Completed, None, None)
            .unwrap();
        state
            .history()
            .rename_run(original.run_id, "Library")
            .unwrap();
        let run_id = original.run_id;

        let mutation = state.begin_history_mutation().unwrap();
        let (sender, receiver) = std::sync::mpsc::channel();
        let rerun_state = state.clone();
        let rerun = std::thread::spawn(move || {
            sender.send(rerun_state.begin_rerun(run_id)).unwrap();
        });
        assert!(
            receiver
                .recv_timeout(std::time::Duration::from_millis(30))
                .is_err()
        );
        drop(mutation);
        let active = receiver
            .recv_timeout(std::time::Duration::from_secs(1))
            .unwrap()
            .unwrap();
        rerun.join().unwrap();
        assert_eq!(active.run_id, original.run_id);
        let reset = state.history().run(original.run_id).unwrap().unwrap();
        assert_eq!(reset.display_name, "Library");
        assert_eq!(reset.status, RunStatus::Planning);
        assert_eq!(reset.completed_items, 0);
        assert!(matches!(
            state.begin_rerun(original.run_id),
            Err(StateError::Busy { active_run_id }) if active_run_id == original.run_id
        ));
        state
            .finish_run(original.run_id, RunStatus::Cancelled, None, None)
            .unwrap();
        assert!(matches!(
            state.begin_rerun(original.run_id + 1),
            Err(StateError::NotActive { .. })
        ));
    }

    #[test]
    fn history_mutation_gate_prevents_begin_run_from_interleaving_delete_or_clear() {
        for delete_only in [true, false] {
            let directory = tempfile::tempdir().unwrap();
            let state =
                AppState::new(HistoryStore::open(directory.path().join("history.sqlite")).unwrap())
                    .unwrap();
            let old = state.begin_run(request()).unwrap();
            state
                .finish_run(old.run_id, RunStatus::Completed, None, None)
                .unwrap();

            let barrier = Arc::new(std::sync::Barrier::new(2));
            set_history_mutation_barrier(Some(barrier.clone()));
            let mutation_state = state.clone();
            let mutation = std::thread::spawn(move || {
                if delete_only {
                    mutation_state.delete_history_run(old.run_id).map(|_| ())
                } else {
                    mutation_state.clear_history()
                }
            });
            // The mutator now owns the active lock and is paused immediately
            // before the database mutation.
            barrier.wait();
            let (sender, receiver) = std::sync::mpsc::channel();
            let begin_state = state.clone();
            let begin = std::thread::spawn(move || {
                sender.send(begin_state.begin_run(request())).unwrap();
            });
            assert!(
                receiver
                    .recv_timeout(std::time::Duration::from_millis(30))
                    .is_err()
            );
            barrier.wait();
            mutation.join().unwrap().unwrap();
            let fresh = receiver
                .recv_timeout(std::time::Duration::from_secs(1))
                .unwrap()
                .unwrap();
            begin.join().unwrap();
            set_history_mutation_barrier(None);
            assert!(state.history().run(fresh.run_id).unwrap().is_some());
        }
    }
}
