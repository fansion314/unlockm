pub mod job;
pub mod runner;
pub mod scan;
pub mod watch;

use crate::{config::RunConfig, error::Result, PARTIAL_FAILURE_EXIT};
use std::sync::Arc;
use tracing::info;

#[derive(Debug, Default, Clone, Copy)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobOutcome {
    Converted,
    Copied,
    Skipped,
    Conflict,
    Failed,
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
        }
    }
}
pub async fn run(config: RunConfig) -> Result<Summary> {
    let config = Arc::new(config);
    let initial = runner::run_paths(config.clone(), scan::enumerate(&config)?).await?;
    info!(
        converted = initial.converted,
        copied = initial.copied,
        skipped = initial.skipped,
        failed = initial.failed,
        "batch summary"
    );
    if config.watch {
        watch::run(config, initial).await
    } else {
        Ok(initial)
    }
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
}
