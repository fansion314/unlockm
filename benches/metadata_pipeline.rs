//! Deterministic local benchmark for the NCM + `--update-metadata` path.
//! It intentionally exercises success, invalid-cover best effort, passthrough,
//! and existing-output skip paths and checks audio payload identity after every
//! successful conversion.

#[path = "../tests/support/ncm_fixture.rs"]
mod ncm_fixture;

use base64::{engine::general_purpose::STANDARD, Engine};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};
use unlockm::{app, config::RunConfig};

const BUFFER: usize = 1024 * 1024;
const DEFAULT_NCM_BYTES: usize = 32 * 1024 * 1024;

fn env_bytes(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn run(input: PathBuf, output: PathBuf, update_metadata: bool) -> (app::Summary, Duration) {
    let config = Arc::new(
        RunConfig::new(
            input.clone(),
            Some(output),
            false,
            false,
            false,
            false,
            update_metadata,
            1,
            BUFFER,
            1,
            None,
            None,
            None,
            None,
            None,
        )
        .expect("benchmark config"),
    );
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("benchmark runtime");
    let started = Instant::now();
    let summary = runtime
        .block_on(app::runner::run_paths(
            config.clone(),
            vec![config.input.clone()],
        ))
        .expect("benchmark pipeline");
    if summary.failed != 0 {
        let detail = match app::job::preflight(&config, &config.input) {
            Err(error) => error,
            Ok(None) => "runner reported failure but replay preflight skipped".into(),
            Ok(Some(planned)) => app::job::execute(
                &config,
                &Arc::new(std::sync::Mutex::new(Default::default())),
                planned,
            )
            .err()
            .unwrap_or_else(|| "runner reported failure but replay execute succeeded".into()),
        };
        panic!("benchmark pipeline failed: {detail}");
    }
    (summary, started.elapsed())
}

fn payload_hash(path: &Path) -> [u8; 32] {
    let bytes = fs::read(path).expect("read benchmark output");
    let mut hash = Sha256::new();
    hash.update(ncm_fixture::mpeg_payload(&bytes));
    hash.finalize().into()
}

fn report(
    name: &str,
    elapsed: Duration,
    summary: app::Summary,
    bytes: usize,
    metadata: bool,
    payload_hash: [u8; 32],
) {
    println!(
        "{name}: bytes={bytes} jobs=1 buffer={BUFFER} metadata={metadata} seconds={:.3} converted={} copied={} skipped={} failed={} payload_sha256={payload_hash:02x?}",
        elapsed.as_secs_f64(),
        summary.converted,
        summary.copied,
        summary.skipped,
        summary.failed
    );
}

fn main() {
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .with_target(false)
        .try_init();
    let directory = tempfile::tempdir().expect("benchmark directory");
    let bytes = env_bytes("UNLOCKM_NCM_BENCH_BYTES", DEFAULT_NCM_BYTES).max(64 * 1024);
    let source_payload = ncm_fixture::large_mp3(bytes);
    let expected_payload = {
        let mut hash = Sha256::new();
        hash.update(ncm_fixture::mpeg_payload(&source_payload));
        hash.finalize().into()
    };

    let success = directory.path().join("success.ncm");
    fs::write(
        &success,
        ncm_fixture::ncm_with_audio(
            &STANDARD.decode(ncm_fixture::VALID_PNG).unwrap(),
            &source_payload,
        ),
    )
    .unwrap();
    let success_out = directory.path().join("success-out");
    let (summary, elapsed) = run(success.clone(), success_out.clone(), true);
    assert_eq!(
        summary.converted, 1,
        "metadata success summary: {summary:?}"
    );
    assert_eq!(
        payload_hash(&success_out.join("success.mp3")),
        expected_payload
    );
    report(
        "ncm-metadata-success",
        elapsed,
        summary,
        source_payload.len(),
        true,
        expected_payload,
    );

    let invalid = directory.path().join("invalid-cover.ncm");
    fs::write(
        &invalid,
        ncm_fixture::ncm_with_audio(b"not-an-image", &source_payload),
    )
    .unwrap();
    let invalid_out = directory.path().join("invalid-out");
    let (summary, elapsed) = run(invalid.clone(), invalid_out.clone(), true);
    assert_eq!(summary.converted, 1, "invalid-cover summary: {summary:?}");
    assert_eq!(
        payload_hash(&invalid_out.join("invalid-cover.mp3")),
        expected_payload
    );
    report(
        "ncm-metadata-invalid-cover-best-effort",
        elapsed,
        summary,
        source_payload.len(),
        true,
        expected_payload,
    );

    let (summary, elapsed) = run(success, success_out.clone(), true);
    assert_eq!(summary.skipped, 1, "existing-output summary: {summary:?}");
    assert_eq!(
        payload_hash(&success_out.join("success.mp3")),
        expected_payload
    );
    report(
        "ncm-existing-output",
        elapsed,
        summary,
        source_payload.len(),
        true,
        expected_payload,
    );

    let passthrough = directory.path().join("passthrough.mp3");
    fs::write(&passthrough, &source_payload).unwrap();
    let passthrough_out = directory.path().join("passthrough-out");
    let (summary, elapsed) = run(passthrough, passthrough_out.clone(), false);
    assert_eq!(summary.copied, 1, "passthrough summary: {summary:?}");
    assert_eq!(
        fs::read(passthrough_out.join("passthrough.mp3")).unwrap(),
        source_payload
    );
    report(
        "passthrough-copy",
        elapsed,
        summary,
        source_payload.len(),
        false,
        expected_payload,
    );
}
