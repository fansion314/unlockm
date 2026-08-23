//! Stable, UI-independent batch observation types.
//!
//! The CLI uses the default no-op observer. Hosts such as the UnlockM desktop
//! application can subscribe without parsing tracing output or importing
//! runner internals.

use super::{JobOutcome, JobReport, Summary};
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::Arc};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemKind {
    Music,
    Companion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemAction {
    Convert,
    Copy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportWarning {
    pub code: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportError {
    pub code: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ArtworkSnapshot {
    pub mime_type: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct MusicSnapshot {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration_ms: Option<u32>,
    pub output_bytes: Option<u64>,
    pub output_format: Option<String>,
    pub bitrate_kbps: Option<u32>,
    pub artwork: Option<ArtworkSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BatchEvent {
    PlanningStarted {
        total: usize,
    },
    ItemPlanning {
        ordinal: u64,
        source: PathBuf,
        relative_path: PathBuf,
        item_kind: ItemKind,
    },
    ItemProcessing {
        ordinal: u64,
        source: PathBuf,
        relative_path: PathBuf,
        destination: PathBuf,
        item_kind: ItemKind,
        action: ItemAction,
    },
    ItemTerminal {
        report: Box<JobReport>,
    },
    Completed {
        summary: Summary,
        cancelled: bool,
    },
}

/// Synchronous emission deliberately leaves storage/backpressure policy to a
/// host-owned adapter. The desktop adapter feeds one bounded coordinator; the
/// no-op default leaves the CLI allocation-free at this boundary.
pub trait BatchEventSink: Send + Sync {
    fn emit(&self, event: BatchEvent);
}

impl<F> BatchEventSink for F
where
    F: Fn(BatchEvent) + Send + Sync,
{
    fn emit(&self, event: BatchEvent) {
        self(event);
    }
}

#[derive(Clone)]
pub struct BatchObserver {
    sink: Arc<dyn BatchEventSink>,
    enabled: bool,
    capture_music_snapshots: bool,
}

impl BatchObserver {
    pub fn new(sink: Arc<dyn BatchEventSink>) -> Self {
        Self {
            sink,
            enabled: true,
            capture_music_snapshots: false,
        }
    }

    /// Enables bounded local tag/artwork snapshots for a host that persists
    /// them. The CLI/watch default remains a cheap no-op reporting path.
    pub fn with_music_snapshots(mut self) -> Self {
        self.capture_music_snapshots = true;
        self
    }

    pub fn captures_music_snapshots(&self) -> bool {
        self.capture_music_snapshots
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn emit(&self, event: BatchEvent) {
        self.sink.emit(event);
    }
}

impl Default for BatchObserver {
    fn default() -> Self {
        Self {
            sink: Arc::new(|_: BatchEvent| {}),
            enabled: false,
            capture_music_snapshots: false,
        }
    }
}

pub fn terminal_outcome(event: &BatchEvent) -> Option<JobOutcome> {
    match event {
        BatchEvent::ItemTerminal { report } => Some(report.outcome),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{JobTimings, events::ItemAction};
    use std::sync::Mutex;

    #[test]
    fn batch_events_round_trip_and_preserve_terminal_details() {
        let event = BatchEvent::ItemTerminal {
            report: Box::new(JobReport {
                ordinal: 7,
                source: PathBuf::from("/music/source.ncm"),
                relative_path: PathBuf::from("source.ncm"),
                destination: Some(PathBuf::from("/output/source.flac")),
                item_kind: ItemKind::Music,
                action: Some(ItemAction::Convert),
                logical_bytes: 42,
                outcome: JobOutcome::Cancelled,
                error: Some(ReportError {
                    code: "cancelled".into(),
                    detail: "user requested cancellation".into(),
                }),
                warnings: vec![ReportWarning {
                    code: "metadata_update_skipped".into(),
                    detail: "offline".into(),
                }],
                music: Some(MusicSnapshot {
                    title: Some("Track".into()),
                    ..Default::default()
                }),
                timings: JobTimings::default(),
            }),
        };

        let json = serde_json::to_string(&event).unwrap();
        let decoded: BatchEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, event);
        assert_eq!(terminal_outcome(&decoded), Some(JobOutcome::Cancelled));
    }

    #[test]
    fn observer_preserves_emission_order() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let captured = seen.clone();
        let observer = BatchObserver::new(Arc::new(move |event| {
            captured.lock().unwrap().push(event);
        }));

        observer.emit(BatchEvent::PlanningStarted { total: 1 });
        observer.emit(BatchEvent::Completed {
            summary: Summary::default(),
            cancelled: false,
        });

        assert!(matches!(
            seen.lock().unwrap().as_slice(),
            [
                BatchEvent::PlanningStarted { total: 1 },
                BatchEvent::Completed {
                    cancelled: false,
                    ..
                }
            ]
        ));
    }

    #[test]
    fn snapshots_are_an_explicit_observer_capability() {
        assert!(!BatchObserver::default().captures_music_snapshots());
        assert!(!BatchObserver::default().is_enabled());
        assert!(
            BatchObserver::new(Arc::new(|_: BatchEvent| {}))
                .with_music_snapshots()
                .captures_music_snapshots()
        );
    }
}
