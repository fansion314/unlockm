#[path = "support/qmc_fixture.rs"]
mod qmc_fixture;

use std::{
    fs,
    sync::{Arc, Mutex},
};

#[tokio::test]
async fn metadata_off_full_batch_never_requests_provider_or_tag_snapshots() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("input.qmc0");
    let output = temp.path().join("out");
    let mut flac = b"fLaC".to_vec();
    flac.resize(64, 0);
    qmc_fixture::transform(&mut flac, 0);
    fs::write(&input, flac).unwrap();
    let config = unlockm::config::RunConfig::new(
        input.clone(),
        Some(output),
        false,
        false,
        false,
        false,
        false,
        1,
        7,
        1,
        None,
        None,
        None,
        None,
        None,
    )
    .unwrap();
    let planned = unlockm::app::job::preflight(&config, &config.input)
        .unwrap()
        .unwrap();
    assert!(matches!(
        planned.action,
        unlockm::app::job::PlannedAction::Decrypt { .. }
    ));
    assert!(
        !config.output.exists(),
        "bounded preflight must not materialize a full stage or output root"
    );

    let events = Arc::new(Mutex::new(Vec::new()));
    let captured = events.clone();
    let summary = unlockm::app::run_batch_with_observer_and_cancel(
        config,
        unlockm::app::runner::Cancellation::default(),
        unlockm::app::events::BatchObserver::new(Arc::new(move |event| {
            captured.lock().unwrap().push(event);
        })),
    )
    .await
    .unwrap();
    assert_eq!(summary.converted, 1);
    let events = events.lock().unwrap();
    let reports: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            unlockm::app::events::BatchEvent::ItemTerminal { report } => Some(report),
            _ => None,
        })
        .collect();
    assert_eq!(reports.len(), 1);
    assert!(reports[0].warnings.is_empty());
    assert!(
        reports[0]
            .music
            .as_ref()
            .is_some_and(|music| music.artwork.is_none())
    );
}
