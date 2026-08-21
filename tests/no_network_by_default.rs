#[path = "support/qmc_fixture.rs"]
mod qmc_fixture;

use std::fs;

#[test]
fn encrypted_preflight_without_update_metadata_never_needs_a_provider() {
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
}
